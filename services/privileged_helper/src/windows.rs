//! Windows platform implementation for the privileged helper (compile-only).
//!
//! This module is compiled on Windows but is **never executed** in this
//! repository's test environment: tests use [`crate::backend::FakeBackend`].
//! Routing uses the IP Helper API; elevated core launch uses `CreateProcessW`
//! bound into a Job Object. The executable path and arguments are validated by
//! `ipc_contract` before they reach these functions.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_void;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ipc_contract::{
    parse_cidr, validate_elevated_core, validate_route_entries, validate_tun_address,
    ElevatedCoreSpec, HelperError, RouteEntry, TunAddressConfig,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::backend::{CoreExit, HelperBackend, StartedCore};

pub use crate::pipe_security::{current_user_sid_string, PipeSecurity};

const AF_INET: u16 = 2;
const AF_INET6: u16 = 23;
const MIB_IPPROTO_NETMGMT: u32 = 3;
/// Wait status a live process reports to `GetExitCodeProcess` (WinBase).
const STILL_ACTIVE: u32 = 259;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_SUSPENDED: u32 = 0x0000_0004;

fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

#[link(name = "kernel32")]
extern "system" {
    fn GetNamedPipeClientProcessId(named_pipe: *mut c_void, client_pid: *mut u32) -> i32;
    fn CreateProcessW(
        application_name: *const u16,
        command_line: *mut u16,
        process_attributes: *mut c_void,
        thread_attributes: *mut c_void,
        inherit_handles: i32,
        creation_flags: u32,
        environment: *mut c_void,
        current_directory: *const u16,
        startup_info: *mut StartupInfoW,
        process_information: *mut ProcessInformation,
    ) -> i32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
}

/// Arm a Job Object with `KILL_ON_JOB_CLOSE` so a managed core can never
/// outlive the helper that owns it.
fn configure_job_kill_on_close(job: HANDLE) -> Result<(), HelperError> {
    use windows::Win32::System::JobObjects::{
        JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of_val(&info) as u32,
        )
    }
    .map_err(|error| HelperError::Backend {
        detail: format!("SetInformationJobObject(KILL_ON_JOB_CLOSE) failed: {error}"),
    })
}

/// Bind a process to its Job Object. Any failure aborts the launch: without
/// the job the core could outlive the helper, so the session fails instead
/// of running unowned.
fn assign_to_job(job: HANDLE, process: HANDLE) -> Result<(), HelperError> {
    use windows::Win32::System::JobObjects::AssignProcessToJobObject;
    unsafe { AssignProcessToJobObject(job, process) }.map_err(|error| {
        HelperError::JobAssignFailed {
            detail: format!("AssignProcessToJobObject failed: {error}"),
        }
    })
}

/// Creation time of the process behind `handle`, as Unix milliseconds.
fn creation_time_of_handle(handle: HANDLE) -> Option<i64> {
    use windows::Win32::System::Threading::GetProcessTimes;
    unsafe {
        let mut creation = windows::Win32::Foundation::FILETIME::default();
        let mut exit = windows::Win32::Foundation::FILETIME::default();
        let mut kernel = windows::Win32::Foundation::FILETIME::default();
        let mut user = windows::Win32::Foundation::FILETIME::default();
        GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).ok()?;
        Some(filetime_to_unix_ms(&creation))
    }
}

/// Creation time of a live `pid`, as Unix milliseconds (`None` when the
/// process is gone or not visible).
fn pid_creation_time_ms(pid: u32) -> Option<i64> {
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let created = creation_time_of_handle(handle);
        let _ = CloseHandle(handle);
        created
    }
}

fn filetime_to_unix_ms(ft: &windows::Win32::Foundation::FILETIME) -> i64 {
    const WINDOWS_TO_UNIX_EPOCH_100NS: i64 = 116_444_736_000_000_000;
    let raw = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    ((raw as i64) - WINDOWS_TO_UNIX_EPOCH_100NS) / 10_000
}

#[link(name = "iphlpapi")]
extern "system" {
    fn CreateIpForwardEntry2(row: *const MibIpForwardRow2) -> u32;
    fn DeleteIpForwardEntry2(row: *const MibIpForwardRow2) -> u32;
    fn CreateUnicastIpAddressEntry(row: *const MibUnicastIpAddressRow) -> u32;
    fn DeleteUnicastIpAddressEntry(row: *const MibUnicastIpAddressRow) -> u32;
}

#[link(name = "shell32")]
extern "system" {
    fn IsUserAnAdmin() -> i32;
}

/// A raw `SOCKADDR_INET`-sized buffer.
#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct SockaddrInet {
    pub family: u16,
    pub data: [u8; 26],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpAddressPrefix {
    pub prefix: SockaddrInet,
    pub prefix_length: u8,
}

/// Local mirror of `MIB_IPFORWARD_ROW2` (the fields the helper sets).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MibIpForwardRow2 {
    pub interface_luid: u64,
    pub interface_index: u32,
    pub destination_prefix: IpAddressPrefix,
    pub next_hop: SockaddrInet,
    pub site_prefix_length: u8,
    pub valid_lifetime: u32,
    pub preferred_lifetime: u32,
    pub metric: u32,
    pub protocol: u32,
    pub loopback: u8,
    pub autoconfigure_address: u8,
    pub publish: u8,
    pub immortal: u8,
    pub age: u32,
    pub origin: u32,
}

/// Local mirror of the subset of `MIB_UNICASTIPADDRESS_ROW` the helper sets.
///
/// Field order and padding must match the SDK struct exactly: `Address` comes
/// first, `SkipAsSource` follows `OnLinkPrefixLength`, and the trailing
/// `CreationTimeStamp` keeps the size at 80 bytes. A wrong order makes
/// `CreateUnicastIpAddressEntry` read a garbage interface index and fail with
/// `ERROR_INVALID_PARAMETER (87)`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MibUnicastIpAddressRow {
    pub address: SockaddrInet,
    pub interface_luid: u64,
    pub interface_index: u32,
    pub prefix_origin: u32,
    pub suffix_origin: u32,
    pub valid_lifetime: u32,
    pub preferred_lifetime: u32,
    pub on_link_prefix_length: u8,
    pub skip_as_source: u8,
    pub dad_state: u32,
    pub scope_id: u32,
    pub creation_time_stamp: i64,
}

#[repr(C)]
pub struct StartupInfoW {
    pub cb: u32,
    pub lp_reserved: *mut u16,
    pub lp_desktop: *mut u16,
    pub lp_title: *mut u16,
    pub dw_x: u32,
    pub dw_y: u32,
    pub dw_x_size: u32,
    pub dw_y_size: u32,
    pub dw_x_count_chars: u32,
    pub dw_y_count_chars: u32,
    pub dw_fill_attribute: u32,
    pub dw_flags: u32,
    pub w_show_window: u16,
    pub cb_reserved2: u16,
    pub lp_reserved2: *mut u8,
    pub h_std_input: *mut c_void,
    pub h_std_output: *mut c_void,
    pub h_std_error: *mut c_void,
}

#[repr(C)]
pub struct ProcessInformation {
    pub h_process: *mut c_void,
    pub h_thread: *mut c_void,
    pub dw_process_id: u32,
    pub dw_thread_id: u32,
}

/// Retrieve the SID string of the process connected to a named pipe.
///
/// # Safety
/// `pipe_handle` must be a valid pipe handle owned by the caller for the duration of the call.
pub unsafe fn client_sid(pipe_handle: *mut c_void) -> std::io::Result<String> {
    unsafe {
        let mut pid = 0u32;
        if GetNamedPipeClientProcessId(pipe_handle, &mut pid) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(io_err)?;
        let mut token = HANDLE::default();
        let opened = OpenProcessToken(process, TOKEN_QUERY, &mut token);
        let sid = match opened {
            Ok(()) => token_user_sid(token),
            Err(e) => Err(io_err(e)),
        };
        let _ = CloseHandle(token);
        let _ = CloseHandle(process);
        sid
    }
}

fn token_user_sid(token: HANDLE) -> std::io::Result<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    unsafe {
        let mut buf = vec![0u64; 64];
        let mut ret = 0u32;
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut c_void),
            (buf.len() * 8) as u32,
            &mut ret,
        )
        .map_err(io_err)?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_str = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid_str).map_err(io_err)?;
        let sid = pwstr_to_string(sid_str).ok_or_else(|| io_err("invalid SID string"))?;
        let _ = LocalFree(HLOCAL(sid_str.0 as *mut c_void));
        Ok(sid)
    }
}

fn pwstr_to_string(value: windows::core::PWSTR) -> Option<String> {
    if value.0.is_null() {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *value.0.add(len) != 0 {
            len += 1;
        }
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(
            value.0, len,
        )))
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn sockaddr_from_ip(ip: &IpAddr) -> SockaddrInet {
    let mut sa = SockaddrInet {
        family: 0,
        data: [0u8; 26],
    };
    match ip {
        IpAddr::V4(v4) => {
            sa.family = AF_INET;
            sa.data[2..6].copy_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            sa.family = AF_INET6;
            sa.data[6..22].copy_from_slice(&v6.octets());
        }
    }
    sa
}

fn forward_row(entry: &RouteEntry) -> Result<MibIpForwardRow2, HelperError> {
    let (destination, prefix) = parse_cidr(&entry.destination)?;
    let next_hop = IpAddr::from_str(entry.next_hop.trim())
        .map_err(|_| HelperError::malformed("next_hop is not an IP address"))?;
    let mut row: MibIpForwardRow2 = unsafe { std::mem::zeroed() };
    row.interface_index = entry.interface_index;
    row.destination_prefix = IpAddressPrefix {
        prefix: sockaddr_from_ip(&destination),
        prefix_length: prefix,
    };
    row.next_hop = sockaddr_from_ip(&next_hop);
    row.site_prefix_length = prefix;
    row.valid_lifetime = u32::MAX;
    row.preferred_lifetime = u32::MAX;
    row.metric = entry.metric;
    row.protocol = MIB_IPPROTO_NETMGMT;
    Ok(row)
}

fn unicast_row(
    interface_index: u32,
    address: &str,
    prefix: u8,
) -> Result<MibUnicastIpAddressRow, HelperError> {
    let (parsed, _) = parse_cidr(&format!("{address}/{prefix}"))?;
    let mut row: MibUnicastIpAddressRow = unsafe { std::mem::zeroed() };
    row.interface_index = interface_index;
    row.address = sockaddr_from_ip(&parsed);
    row.on_link_prefix_length = prefix;
    row.skip_as_source = 0;
    row.valid_lifetime = u32::MAX;
    row.preferred_lifetime = u32::MAX;
    Ok(row)
}

struct CoreRecord {
    process: usize,
    job: usize,
    /// PID plus creation time (cf. `runtime::identity::ProcessIdentity`): a
    /// bare PID can be recycled, so `stop` re-verifies both before killing.
    pid: u32,
    created_at_ms: i64,
}

/// Real Windows backend. Constructor and method construction logic are compiled
/// and unit-reachable, but the OS calls are not exercised in this environment.
pub struct WindowsBackend {
    allowed_run_roots: Vec<String>,
    cores: Mutex<BTreeMap<u64, CoreRecord>>,
    known: Mutex<BTreeSet<u64>>,
    tun: Mutex<BTreeMap<u32, TunAddressConfig>>,
    next_handle: AtomicU64,
}

impl WindowsBackend {
    pub fn new(allowed_run_roots: Vec<String>) -> Self {
        Self {
            allowed_run_roots,
            cores: Mutex::new(BTreeMap::new()),
            known: Mutex::new(BTreeSet::new()),
            tun: Mutex::new(BTreeMap::new()),
            next_handle: AtomicU64::new(1),
        }
    }

    pub fn allowed_run_roots(&self) -> &[String] {
        &self.allowed_run_roots
    }
}

impl HelperBackend for WindowsBackend {
    fn is_elevated(&self) -> bool {
        unsafe { IsUserAnAdmin() != 0 }
    }

    fn add_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError> {
        validate_route_entries(entries)?;
        for entry in entries {
            let row = forward_row(entry)?;
            let status = unsafe { CreateIpForwardEntry2(&row) };
            if status != 0 {
                return Err(HelperError::Backend {
                    detail: format!("CreateIpForwardEntry2 failed with code {status}"),
                });
            }
        }
        Ok(entries.len() as u32)
    }

    fn remove_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError> {
        validate_route_entries(entries)?;
        for entry in entries {
            let row = forward_row(entry)?;
            let status = unsafe { DeleteIpForwardEntry2(&row) };
            if status != 0 {
                return Err(HelperError::Backend {
                    detail: format!("DeleteIpForwardEntry2 failed with code {status}"),
                });
            }
        }
        Ok(entries.len() as u32)
    }

    fn set_tun_address(&self, config: &TunAddressConfig) -> Result<(), HelperError> {
        validate_tun_address(config)?;
        for address in &config.addresses {
            let row = unicast_row(config.interface_index, &address.address, address.prefix_len)?;
            let status = unsafe { CreateUnicastIpAddressEntry(&row) };
            if status != 0 {
                return Err(HelperError::Backend {
                    detail: format!("CreateUnicastIpAddressEntry failed with code {status}"),
                });
            }
        }
        self.tun
            .lock()
            .expect("tun registry poisoned")
            .insert(config.interface_index, config.clone());
        Ok(())
    }

    fn reset_tun_address(&self, interface_index: u32) -> Result<(), HelperError> {
        let stored = self
            .tun
            .lock()
            .expect("tun registry poisoned")
            .remove(&interface_index);
        if let Some(config) = stored {
            for address in &config.addresses {
                let row = unicast_row(interface_index, &address.address, address.prefix_len)?;
                let status = unsafe { DeleteUnicastIpAddressEntry(&row) };
                if status != 0 {
                    return Err(HelperError::Backend {
                        detail: format!("DeleteUnicastIpAddressEntry failed with code {status}"),
                    });
                }
            }
        }
        Ok(())
    }

    fn run_elevated_core(&self, spec: &ElevatedCoreSpec) -> Result<StartedCore, HelperError> {
        validate_elevated_core(spec, &self.allowed_run_roots)?;

        let application = wide(&spec.exe_path);
        let mut command_line = wide(&command_line(spec));
        let current_directory = wide(&spec.run_dir);
        let mut startup: StartupInfoW = unsafe { std::mem::zeroed() };
        startup.cb = std::mem::size_of::<StartupInfoW>() as u32;
        let mut info: ProcessInformation = unsafe { std::mem::zeroed() };

        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED,
                std::ptr::null_mut(),
                current_directory.as_ptr(),
                &mut startup,
                &mut info,
            )
        };
        if created == 0 {
            return Err(HelperError::Backend {
                detail: format!("CreateProcessW failed: {}", std::io::Error::last_os_error()),
            });
        }

        let created_at_ms = creation_time_of_handle(HANDLE(info.h_process)).unwrap_or(0);
        let pid = info.dw_process_id;

        let job = {
            use windows::Win32::System::JobObjects::CreateJobObjectW;
            unsafe {
                CreateJobObjectW(None, None).map_err(|error| HelperError::Backend {
                    detail: format!("CreateJobObjectW failed: {error}"),
                })?
            }
        };
        if let Err(error) = configure_job_kill_on_close(job) {
            unsafe {
                TerminateProcess(info.h_process, 1);
                let _ = CloseHandle(HANDLE(info.h_process));
                let _ = CloseHandle(HANDLE(info.h_thread));
                let _ = CloseHandle(job);
            }
            return Err(error);
        }
        if let Err(error) = assign_to_job(job, HANDLE(info.h_process)) {
            unsafe {
                TerminateProcess(info.h_process, 1);
                let _ = CloseHandle(HANDLE(info.h_process));
                let _ = CloseHandle(HANDLE(info.h_thread));
                let _ = CloseHandle(job);
            }
            return Err(error);
        }
        unsafe {
            ResumeThread(info.h_thread);
            let _ = CloseHandle(HANDLE(info.h_thread));
        }

        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);
        self.known.lock().expect("known poisoned").insert(handle);
        self.cores.lock().expect("cores poisoned").insert(
            handle,
            CoreRecord {
                process: info.h_process as usize,
                job: job.0 as usize,
                pid,
                created_at_ms,
            },
        );
        Ok(StartedCore { handle, pid })
    }

    fn stop_elevated_core(&self, handle: u64) -> Result<(), HelperError> {
        if !self.known.lock().expect("known poisoned").contains(&handle) {
            return Err(HelperError::UnknownHandle { handle });
        }
        if let Some(record) = self.cores.lock().expect("cores poisoned").remove(&handle) {
            // PID + creation-time binding: refuse to kill a recycled PID that
            // merely reused the recorded number.
            let live = pid_creation_time_ms(record.pid);
            if live.is_none_or(|created| created != record.created_at_ms) {
                unsafe {
                    let _ = CloseHandle(HANDLE(record.process as *mut c_void));
                    if record.job != 0 {
                        let _ = CloseHandle(HANDLE(record.job as *mut c_void));
                    }
                }
                return Err(HelperError::UnknownHandle { handle });
            }
            unsafe {
                TerminateProcess(record.process as *mut c_void, 1);
                let _ = CloseHandle(HANDLE(record.process as *mut c_void));
                if record.job != 0 {
                    let _ = CloseHandle(HANDLE(record.job as *mut c_void));
                }
            }
        }
        Ok(())
    }

    /// SP-06: handle-scoped exit poll. The owned process handle makes the
    /// observation authoritative: no PID scan, so PID reuse cannot miskill.
    /// Never removes records and never kills; stop/disconnect own removal.
    fn poll_core_exits(&self) -> Vec<CoreExit> {
        let cores = self.cores.lock().expect("cores poisoned");
        let mut exited = Vec::new();
        for (handle, record) in cores.iter() {
            // A failed query leaves `STILL_ACTIVE` in place, which reads as
            // live/unknown below: never fabricate a transition.
            let mut code = STILL_ACTIVE;
            // SAFETY: `record.process` is a live owned process handle stored
            // at spawn; the call only writes the wait status into `code`.
            let queried =
                unsafe { GetExitCodeProcess(HANDLE(record.process as *mut c_void), &mut code) };
            if queried.is_ok() && code != STILL_ACTIVE {
                exited.push(CoreExit {
                    handle: *handle,
                    pid: record.pid,
                    exit_code: Some(code as i32),
                });
            }
        }
        exited
    }

    fn shutdown(&self) -> Result<(), HelperError> {
        let handles: Vec<u64> = self
            .cores
            .lock()
            .expect("cores poisoned")
            .keys()
            .copied()
            .collect();
        for handle in handles {
            self.stop_elevated_core(handle)?;
        }
        Ok(())
    }
}

/// Quote an argument for the Windows command line.
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
            quoted.push(ch);
        } else if ch == '"' {
            quoted.extend(std::iter::repeat_n('\\', backslashes + 1));
            quoted.push('"');
            backslashes = 0;
        } else {
            backslashes = 0;
            quoted.push(ch);
        }
    }
    if backslashes > 0 {
        quoted.extend(std::iter::repeat_n('\\', backslashes));
    }
    quoted.push('"');
    quoted
}

fn command_line(spec: &ElevatedCoreSpec) -> String {
    let mut line = format!("\"{}\"", spec.exe_path);
    for arg in &spec.args {
        line.push(' ');
        line.push_str(&quote_arg(arg));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_arguments() {
        let spec = ElevatedCoreSpec {
            core: "xray".into(),
            exe_path: r"C:\v2rayn-r\core\xray.exe".into(),
            args: vec!["-c".into(), r"C:\a b\config.json".into()],
            run_dir: r"C:\v2rayn-r\core".into(),
        };
        let line = command_line(&spec);
        assert!(line.starts_with(r#""C:\v2rayn-r\core\xray.exe""#));
        assert!(line.contains(r#""C:\a b\config.json""#));
    }

    #[test]
    fn sockaddr_families_match_address_kind() {
        let v4 = sockaddr_from_ip(&IpAddr::from_str("198.18.0.1").unwrap());
        assert_eq!(v4.family, AF_INET);
        let v6 = sockaddr_from_ip(&IpAddr::from_str("fd00::1").unwrap());
        assert_eq!(v6.family, AF_INET6);
    }

    #[test]
    fn forward_row_uses_prefix_from_cidr() {
        let entry = RouteEntry {
            destination: "0.0.0.0/0".into(),
            next_hop: "10.0.0.1".into(),
            interface_index: 7,
            metric: 3,
            family: ipc_contract::AddressFamily::V4,
        };
        let row = forward_row(&entry).unwrap();
        assert_eq!(row.interface_index, 7);
        assert_eq!(row.destination_prefix.prefix_length, 0);
        assert_eq!(row.metric, 3);
    }

    #[test]
    fn job_object_arms_kill_on_close() {
        use windows::Win32::System::JobObjects::CreateJobObjectW;
        let job = unsafe { CreateJobObjectW(None, None) }.expect("create job object");
        configure_job_kill_on_close(job).expect("arm KILL_ON_JOB_CLOSE");
        unsafe {
            let _ = CloseHandle(job);
        }
    }

    #[test]
    fn assign_to_invalid_job_fails_structured() {
        let err =
            assign_to_job(HANDLE(std::ptr::null_mut()), HANDLE(std::ptr::null_mut())).unwrap_err();
        assert!(matches!(err, HelperError::JobAssignFailed { .. }));
        // `privileged_helper` has no direct `domain` dependency; compare the
        // stable code string (`ipc_contract` maps it to E_JOB_ASSIGN_FAILED).
        assert_eq!(err.to_domain().code, "E_JOB_ASSIGN_FAILED");
    }

    #[test]
    fn process_identity_binds_pid_and_creation_time() {
        // `privileged_helper` cannot add a `runtime` dependency here, so the
        // PID + creation-time binding lives next to the Job code and mirrors
        // `runtime::identity::ProcessIdentity`.
        let me = std::process::id();
        let created = pid_creation_time_ms(me).expect("current process has a creation time");
        assert!(created > 0);
        // Same PID still matches itself; a bogus PID has no creation time.
        assert_eq!(pid_creation_time_ms(me), Some(created));
        assert_eq!(pid_creation_time_ms(0xFFFF_FFF0), None);
    }
}
