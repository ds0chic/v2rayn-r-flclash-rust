//! Process identity: PID plus creation time (plan §13).
//!
//! A bare PID can be recycled by the OS, so every owned process is recorded
//! and later re-verified as `(pid, creation_time)`. Recovery only touches
//! processes whose identity still matches what net-host journaled.

use serde::{Deserialize, Serialize};

/// Stable identity of an OS process owned by this app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessIdentity {
    pub pid: u32,
    /// Process creation time as Unix milliseconds.
    pub created_at_ms: i64,
}

impl ProcessIdentity {
    pub fn new(pid: u32, created_at_ms: i64) -> Self {
        Self { pid, created_at_ms }
    }
}

/// Identity of the current process.
pub fn current_identity() -> ProcessIdentity {
    let pid = std::process::id();
    ProcessIdentity {
        pid,
        created_at_ms: process_creation_time_ms(pid).unwrap_or(0),
    }
}

/// Query the creation time of `pid`, if the process is alive and visible.
#[cfg(windows)]
pub fn process_creation_time_ms(pid: u32) -> Option<i64> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut creation = windows::Win32::Foundation::FILETIME::default();
        let mut exit = windows::Win32::Foundation::FILETIME::default();
        let mut kernel = windows::Win32::Foundation::FILETIME::default();
        let mut user = windows::Win32::Foundation::FILETIME::default();
        let result = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let _ = CloseHandle(handle);
        result.ok()?;
        Some(filetime_to_unix_ms(&creation))
    }
}

#[cfg(not(windows))]
pub fn process_creation_time_ms(_pid: u32) -> Option<i64> {
    None
}

/// Whether `pid` is still the same process that was recorded.
pub fn matches_identity(identity: &ProcessIdentity) -> bool {
    match process_creation_time_ms(identity.pid) {
        Some(created) => created == identity.created_at_ms,
        None => false,
    }
}

/// Terminate a process only when `(pid, creation_time)` still matches.
///
/// Returns `true` when a matching process was terminated. This is the only
/// kill primitive the app is allowed to use for managed processes: it refuses
/// to touch a recycled PID or an unrecorded process.
#[cfg(windows)]
pub fn terminate_identity(identity: &ProcessIdentity) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    if !matches_identity(identity) {
        return false;
    }
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, identity.pid) else {
            return false;
        };
        let result = TerminateProcess(handle, 1).is_ok();
        let _ = CloseHandle(handle);
        result
    }
}

#[cfg(not(windows))]
pub fn terminate_identity(_identity: &ProcessIdentity) -> bool {
    false
}

#[cfg(windows)]
fn filetime_to_unix_ms(ft: &windows::Win32::Foundation::FILETIME) -> i64 {
    const WINDOWS_TO_UNIX_EPOCH_100NS: i64 = 116_444_736_000_000_000;
    let raw = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    ((raw as i64) - WINDOWS_TO_UNIX_EPOCH_100NS) / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_has_an_identity() {
        let id = current_identity();
        assert_eq!(id.pid, std::process::id());
        #[cfg(windows)]
        assert!(matches_identity(&id));
    }

    #[cfg(windows)]
    #[test]
    fn a_bogus_identity_does_not_match() {
        let bogus = ProcessIdentity::new(0xFFFF_FFF0, 1);
        assert!(!matches_identity(&bogus));
    }
}
