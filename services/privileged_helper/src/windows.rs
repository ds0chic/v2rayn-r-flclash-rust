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
    OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::backend::{HelperBackend, StartedCore};

pub use crate::pipe_security::{current_user_sid_string, PipeSecurity};

const AF_INET: u16 = 2;
const AF_INET6: u16 = 23;
const MIB_IPPROTO_NETMGMT: u32 = 3;
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
    fn CreateJobObjectW(job_attributes: *mut c_void, name: *const u16) -> *mut c_void;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
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
#[repr(C)]
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
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MibUnicastIpAddressRow {
    pub interface_luid: u64,
    pub interface_index: u32,
    pub address: SockaddrInet,
    pub prefix_origin: u32,
    pub suffix_origin: u32,
    pub valid_lifetime: u32,
    pub preferred_lifetime: u32,
    pub on_link_prefix_length: u8,
    pub prefix_length: u8,
    pub dad_state: u32,
    pub scope_id: u32,
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
pub fn client_sid(pipe_handle: *mut c_void) -> std::io::Result<String> {
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
    row.prefix_length = prefix;
    row.valid_lifetime = u32::MAX;
    row.preferred_lifetime = u32::MAX;
    Ok(row)
}

struct CoreRecord {
    process: usize,
    job: usize,
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

        let job = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
        if !job.is_null() {
            unsafe {
                AssignProcessToJobObject(job, info.h_process);
            }
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
                job: job as usize,
            },
        );
        Ok(StartedCore {
            handle,
            pid: info.dw_process_id,
        })
    }

    fn stop_elevated_core(&self, handle: u64) -> Result<(), HelperError> {
        if !self.known.lock().expect("known poisoned").contains(&handle) {
            return Err(HelperError::UnknownHandle { handle });
        }
        if let Some(record) = self.cores.lock().expect("cores poisoned").remove(&handle) {
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
            quoted.extend(std::iter::repeat('\\').take(backslashes + 1));
            quoted.push('"');
            backslashes = 0;
        } else {
            backslashes = 0;
            quoted.push(ch);
        }
    }
    if backslashes > 0 {
        quoted.extend(std::iter::repeat('\\').take(backslashes));
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
}
