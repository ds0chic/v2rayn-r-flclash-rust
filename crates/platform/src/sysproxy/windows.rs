//! Windows system proxy backend (WinINET per-connection + HKCU registry).
//!
//! Two layers, matching the upstream `ProxySettingWindows` handler:
//! 1. `InternetQueryOptionW` / `InternetSetOptionW` with
//!    `INTERNET_OPTION_PER_CONNECTION_OPTION` (LAN / current connection).
//! 2. The `HKCU\...\Internet Settings` registry values, used when the WinINET
//!    call is unavailable (RAS-only setups, restricted tokens, ...).
//!
//! This module is **compile-only** in this task: no automated test invokes it,
//! because that would mutate the host proxy. It is exported for the wiring
//! stage to exercise under an explicit, isolated environment.
//!
//! The Win32 calls are handwritten FFI rather than the `windows` crate so the
//! workspace `--locked` gate and the T13 write boundary stay intact.

use core::ffi::c_void;
use std::mem::size_of;

use super::{ProxyField, ProxyState, SystemProxyBackend};
use crate::error::{PlatformError, Result};

const INTERNET_OPTION_PER_CONNECTION_OPTION: u32 = 75;
const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
const INTERNET_OPTION_REFRESH: u32 = 37;

const PER_CONN_FLAGS: u32 = 1;
const PER_CONN_PROXY_SERVER: u32 = 2;
const PER_CONN_PROXY_BYPASS: u32 = 3;
const PER_CONN_AUTOCONFIG_URL: u32 = 4;

const PROXY_TYPE_DIRECT: u32 = 0x0001;
const PROXY_TYPE_PROXY: u32 = 0x0002;
const PROXY_TYPE_AUTO_PROXY_URL: u32 = 0x0004;
const PROXY_TYPE_AUTO_DETECT: u32 = 0x0008;

const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
const KEY_READ: u32 = 0x20019;
const KEY_WRITE: u32 = 0x20006;
const REG_SZ: u32 = 1;
const REG_DWORD: u32 = 4;
const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

#[repr(C)]
struct InternetPerConnOptionListW {
    dw_size: u32,
    sz_connection: *mut u16,
    dw_option_count: u32,
    dw_option_error: u32,
    options: *mut InternetConnectionOptionW,
}

#[repr(C)]
struct InternetConnectionOptionW {
    dw_option: u32,
    value: PerConnValue,
}

#[repr(C)]
union PerConnValue {
    int_value: u32,
    string_value: *mut u16,
    filetime: [u32; 2],
}

#[link(name = "wininet")]
extern "system" {
    fn InternetSetOptionW(
        internet: *mut c_void,
        option: u32,
        buffer: *mut c_void,
        buffer_length: u32,
    ) -> i32;
    fn InternetQueryOptionW(
        internet: *mut c_void,
        option: u32,
        buffer: *mut c_void,
        buffer_length: *mut u32,
    ) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        hkey: isize,
        sub_key: *const u16,
        options: u32,
        sam_desired: u32,
        result: *mut isize,
    ) -> i32;
    fn RegQueryValueExW(
        hkey: isize,
        value_name: *const u16,
        reserved: *mut u32,
        ty: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        hkey: isize,
        value_name: *const u16,
        reserved: u32,
        ty: u32,
        data: *const u8,
        data_len: u32,
    ) -> i32;
    fn RegCloseKey(hkey: isize) -> i32;
}

/// Windows implementation of [`SystemProxyBackend`].
#[derive(Debug, Default)]
pub struct WindowsSystemProxyBackend;

impl WindowsSystemProxyBackend {
    pub fn new() -> Self {
        Self
    }
}

impl SystemProxyBackend for WindowsSystemProxyBackend {
    fn snapshot(&self) -> Result<ProxyState> {
        // Registry is the authoritative layer for this backend; the WinINET
        // per-connection blob is pushed best-effort by `write_state`.
        read_registry().or_else(|_| query_wininet())
    }

    fn set_field(&self, field: ProxyField, value: Option<&str>) -> Result<()> {
        let mut state = self.snapshot()?;
        state.set_field(field, value);
        write_state(&state)
    }

    fn notify_changed(&self) -> Result<()> {
        // Best effort: mirror the upstream SETTINGS_CHANGED + REFRESH pair.
        unsafe {
            InternetSetOptionW(
                core::ptr::null_mut(),
                INTERNET_OPTION_SETTINGS_CHANGED,
                core::ptr::null_mut(),
                0,
            );
            InternetSetOptionW(
                core::ptr::null_mut(),
                INTERNET_OPTION_REFRESH,
                core::ptr::null_mut(),
                0,
            );
        }
        Ok(())
    }
}

fn to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_utf16_buffer(buffer: &[u16]) -> Option<String> {
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..end]))
}

fn query_wininet() -> Result<ProxyState> {
    let mut server = vec![0u16; 2048];
    let mut bypass = vec![0u16; 2048];
    let mut auto_config = vec![0u16; 2048];

    let mut options = [
        InternetConnectionOptionW {
            dw_option: PER_CONN_FLAGS,
            value: PerConnValue { int_value: 0 },
        },
        InternetConnectionOptionW {
            dw_option: PER_CONN_PROXY_SERVER,
            value: PerConnValue {
                string_value: server.as_mut_ptr(),
            },
        },
        InternetConnectionOptionW {
            dw_option: PER_CONN_PROXY_BYPASS,
            value: PerConnValue {
                string_value: bypass.as_mut_ptr(),
            },
        },
        InternetConnectionOptionW {
            dw_option: PER_CONN_AUTOCONFIG_URL,
            value: PerConnValue {
                string_value: auto_config.as_mut_ptr(),
            },
        },
    ];

    let mut list = InternetPerConnOptionListW {
        dw_size: size_of::<InternetPerConnOptionListW>() as u32,
        sz_connection: core::ptr::null_mut(),
        dw_option_count: options.len() as u32,
        dw_option_error: 0,
        options: options.as_mut_ptr(),
    };
    let mut list_size = size_of::<InternetPerConnOptionListW>() as u32;

    let ok = unsafe {
        InternetQueryOptionW(
            core::ptr::null_mut(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            std::ptr::from_mut(&mut list).cast::<c_void>(),
            std::ptr::from_mut(&mut list_size),
        )
    };
    if ok == 0 {
        return Err(PlatformError::Backend(
            "InternetQueryOptionW(PER_CONNECTION_OPTION) failed".to_string(),
        ));
    }

    let flags = unsafe { options[0].value.int_value };
    Ok(ProxyState {
        enabled: flags & PROXY_TYPE_PROXY != 0,
        auto_detect: flags & PROXY_TYPE_AUTO_DETECT != 0,
        server: read_utf16_buffer(&server),
        bypass: read_utf16_buffer(&bypass),
        auto_config_url: read_utf16_buffer(&auto_config),
    })
}

fn write_state(state: &ProxyState) -> Result<()> {
    // Persist to the registry first (authoritative and always consistent with
    // `snapshot`), then push to the WinINET per-connection blob so running
    // applications refresh immediately. Environments that reject the
    // per-connection set still get the registry value plus `notify_changed`.
    write_registry(state)?;
    let _ = write_wininet(state);
    Ok(())
}

fn write_wininet(state: &ProxyState) -> Result<()> {
    let mut flags = PROXY_TYPE_DIRECT;
    if state.enabled {
        flags |= PROXY_TYPE_PROXY;
    }
    if state.auto_config_url.is_some() {
        flags |= PROXY_TYPE_AUTO_PROXY_URL;
    }
    if state.auto_detect {
        flags |= PROXY_TYPE_AUTO_DETECT;
    }

    let server = to_utf16(state.server.as_deref().unwrap_or(""));
    let bypass = to_utf16(state.bypass.as_deref().unwrap_or(""));
    let auto_config = to_utf16(state.auto_config_url.as_deref().unwrap_or(""));

    let mut options = [
        InternetConnectionOptionW {
            dw_option: PER_CONN_FLAGS,
            value: PerConnValue { int_value: flags },
        },
        InternetConnectionOptionW {
            dw_option: PER_CONN_PROXY_SERVER,
            value: PerConnValue {
                string_value: server.as_ptr().cast_mut(),
            },
        },
        InternetConnectionOptionW {
            dw_option: PER_CONN_PROXY_BYPASS,
            value: PerConnValue {
                string_value: bypass.as_ptr().cast_mut(),
            },
        },
        InternetConnectionOptionW {
            dw_option: PER_CONN_AUTOCONFIG_URL,
            value: PerConnValue {
                string_value: auto_config.as_ptr().cast_mut(),
            },
        },
    ];

    let mut list = InternetPerConnOptionListW {
        dw_size: size_of::<InternetPerConnOptionListW>() as u32,
        sz_connection: core::ptr::null_mut(),
        dw_option_count: options.len() as u32,
        dw_option_error: 0,
        options: options.as_mut_ptr(),
    };

    let ok = unsafe {
        InternetSetOptionW(
            core::ptr::null_mut(),
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            std::ptr::from_mut(&mut list).cast::<c_void>(),
            list.dw_size,
        )
    };
    if ok == 0 {
        return Err(PlatformError::Backend(
            "InternetSetOptionW(PER_CONNECTION_OPTION) failed".to_string(),
        ));
    }
    Ok(())
}

fn open_settings(access: u32) -> Result<isize> {
    let sub_key = to_utf16(INTERNET_SETTINGS);
    let mut hkey: isize = 0;
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub_key.as_ptr(), 0, access, &mut hkey) };
    if rc != 0 {
        return Err(PlatformError::Backend(format!(
            "RegOpenKeyExW(Internet Settings) failed: {rc}"
        )));
    }
    Ok(hkey)
}

fn read_registry() -> Result<ProxyState> {
    let hkey = open_settings(KEY_READ)?;
    let state = ProxyState {
        enabled: read_dword(hkey, "ProxyEnable").unwrap_or(0) != 0,
        auto_detect: read_dword(hkey, "AutoDetect").unwrap_or(0) != 0,
        server: read_string(hkey, "ProxyServer"),
        bypass: read_string(hkey, "ProxyOverride"),
        auto_config_url: read_string(hkey, "AutoConfigURL"),
    };
    unsafe {
        RegCloseKey(hkey);
    }
    Ok(state)
}

fn write_registry(state: &ProxyState) -> Result<()> {
    let hkey = open_settings(KEY_WRITE)?;
    write_dword(hkey, "ProxyEnable", u32::from(state.enabled))?;
    write_string(hkey, "ProxyServer", state.server.as_deref().unwrap_or(""))?;
    write_string(hkey, "ProxyOverride", state.bypass.as_deref().unwrap_or(""))?;
    write_string(
        hkey,
        "AutoConfigURL",
        state.auto_config_url.as_deref().unwrap_or(""),
    )?;
    write_dword(hkey, "AutoDetect", u32::from(state.auto_detect))?;
    unsafe {
        RegCloseKey(hkey);
    }
    Ok(())
}

fn read_dword(hkey: isize, name: &str) -> Option<u32> {
    let name_w = to_utf16(name);
    let mut value: u32 = 0;
    let mut len = size_of::<u32>() as u32;
    let mut ty: u32 = 0;
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            name_w.as_ptr(),
            core::ptr::null_mut(),
            &mut ty,
            std::ptr::from_mut(&mut value).cast::<u8>(),
            &mut len,
        )
    };
    if rc == 0 {
        Some(value)
    } else {
        None
    }
}

fn read_string(hkey: isize, name: &str) -> Option<String> {
    let name_w = to_utf16(name);
    let mut ty: u32 = 0;
    let mut len: u32 = 0;
    let probe = unsafe {
        RegQueryValueExW(
            hkey,
            name_w.as_ptr(),
            core::ptr::null_mut(),
            &mut ty,
            core::ptr::null_mut(),
            &mut len,
        )
    };
    if probe != 0 || len == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (len as usize / 2) + 1];
    let rc = unsafe {
        RegQueryValueExW(
            hkey,
            name_w.as_ptr(),
            core::ptr::null_mut(),
            &mut ty,
            buffer.as_mut_ptr().cast::<u8>(),
            &mut len,
        )
    };
    if rc == 0 {
        read_utf16_buffer(&buffer)
    } else {
        None
    }
}

fn write_dword(hkey: isize, name: &str, value: u32) -> Result<()> {
    let name_w = to_utf16(name);
    let bytes = value.to_le_bytes();
    let rc = unsafe {
        RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_DWORD,
            bytes.as_ptr(),
            bytes.len() as u32,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(PlatformError::Backend(format!(
            "RegSetValueExW({name}) failed: {rc}"
        )))
    }
}

fn write_string(hkey: isize, name: &str, value: &str) -> Result<()> {
    let name_w = to_utf16(name);
    let data = to_utf16(value);
    let rc = unsafe {
        RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_SZ,
            data.as_ptr().cast::<u8>(),
            (data.len() * size_of::<u16>()) as u32,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(PlatformError::Backend(format!(
            "RegSetValueExW({name}) failed: {rc}"
        )))
    }
}

/// Suggested named-proxy string for the given port, mirroring the upstream
/// `GetWindowsProxyString` default. `port` is the SOCKS/HTTP inbound port.
pub fn default_proxy_string(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Build the bypass list with the `<local>` token, mirroring upstream when
/// `NotProxyLocalAddress` is enabled.
pub fn windows_bypass(exceptions: &str, not_proxy_local_address: bool) -> String {
    let compact: String = exceptions
        .split([';'])
        .map(|item| item.replace(' ', ""))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(";");
    if not_proxy_local_address {
        if compact.is_empty() {
            "<local>".to_string()
        } else {
            format!("<local>;{compact}")
        }
    } else {
        compact
    }
}
