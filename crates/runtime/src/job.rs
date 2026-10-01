//! Windows Job Object ownership for managed cores (plan §5, §13).
//!
//! net-host is the sole owner of the core process tree. A Job Object created
//! with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` guarantees the core dies with
//! net-host even if net-host is hard-killed: the OS closes the job handle as
//! part of process teardown, which terminates every assigned process.
//!
//! This job constrains only processes net-host explicitly assigns; it never
//! touches unrelated processes.

/// A Job Object holding the managed core(s).
#[cfg(windows)]
pub struct JobGuard {
    handle: windows::Win32::Foundation::HANDLE,
}

// The handle is owned exclusively by this value and only closed in `Drop`.
#[cfg(windows)]
unsafe impl Send for JobGuard {}
#[cfg(windows)]
unsafe impl Sync for JobGuard {}

#[cfg(windows)]
impl JobGuard {
    /// Create an anonymous job that kills its members when the handle closes.
    pub fn create_kill_on_close() -> std::io::Result<Self> {
        use std::io::Error;
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let handle = CreateJobObjectW(None, None).map_err(|e| Error::other(e.to_string()))?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ptr = &info as *const _ as *const core::ffi::c_void;
            let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            if let Err(e) =
                SetInformationJobObject(handle, JobObjectExtendedLimitInformation, ptr, size)
            {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                return Err(Error::other(e.to_string()));
            }
            Ok(Self { handle })
        }
    }

    /// Assign a process handle to this job.
    pub fn assign(&self, process: std::os::windows::io::RawHandle) -> std::io::Result<()> {
        use std::io::Error;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;

        let process = HANDLE(process);
        unsafe { AssignProcessToJobObject(self.handle, process) }
            .map_err(|e| Error::other(e.to_string()))
    }
}

#[cfg(windows)]
impl Drop for JobGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

/// Non-Windows placeholder so the crate builds on other platforms; T03 is
/// Windows-only, but the shape keeps the API portable for later tasks.
#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn kill_on_job_close_terminates_assigned_members() {
        let job = JobGuard::create_kill_on_close().expect("create job");
        // A long-lived, harmless child; killing it can never affect anything
        // the user runs.
        let mut child = Command::new("cmd")
            .args(["/c", "ping", "-n", "30", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn child");
        job.assign(child.as_raw_handle()).expect("assign child");
        drop(job);

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("job member survived KILL_ON_JOB_CLOSE");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(not(windows))]
#[derive(Default)]
pub struct JobGuard;

#[cfg(not(windows))]
impl JobGuard {
    pub fn create_kill_on_close() -> std::io::Result<Self> {
        Ok(Self)
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn assign(&self, _process: isize) -> std::io::Result<()> {
        Ok(())
    }
}
