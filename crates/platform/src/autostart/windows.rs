//! Windows Run-key autostart backend.
//!
//! Reads/writes/deletes a `REG_SZ` value under
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Compile-only in this
//! task: the tests use [`FakeRegistry`](super::fake::FakeRegistry) so the host
//! registry is never touched.

use super::{AutoStartBackend, RUN_KEY_PATH};
use crate::error::{PlatformError, Result};

const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
const KEY_READ: u32 = 0x20019;
const KEY_WRITE: u32 = 0x20006;
const REG_SZ: u32 = 1;
const ERROR_FILE_NOT_FOUND: i32 = 2;

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
    fn RegDeleteValueW(hkey: isize, value_name: *const u16) -> i32;
    fn RegCloseKey(hkey: isize) -> i32;
}

/// Windows Run-key implementation of [`AutoStartBackend`].
#[derive(Debug, Default)]
pub struct WindowsRunKeyBackend;

impl WindowsRunKeyBackend {
    pub fn new() -> Self {
        Self
    }

    fn open(&self, access: u32) -> Result<isize> {
        let sub_key = to_utf16(RUN_KEY_PATH);
        let mut hkey: isize = 0;
        let rc =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub_key.as_ptr(), 0, access, &mut hkey) };
        if rc != 0 {
            return Err(PlatformError::Backend(format!(
                "RegOpenKeyExW(Run) failed: {rc}"
            )));
        }
        Ok(hkey)
    }
}

impl AutoStartBackend for WindowsRunKeyBackend {
    fn query(&self, name: &str) -> Result<Option<String>> {
        let hkey = self.open(KEY_READ)?;
        let result = query_string(hkey, name);
        unsafe {
            RegCloseKey(hkey);
        }
        Ok(result)
    }

    fn set(&self, name: &str, command: &str) -> Result<()> {
        let hkey = self.open(KEY_WRITE)?;
        let result = write_string(hkey, name, command);
        unsafe {
            RegCloseKey(hkey);
        }
        result
    }

    fn remove(&self, name: &str) -> Result<()> {
        let hkey = self.open(KEY_WRITE)?;
        let name_w = to_utf16(name);
        let rc = unsafe { RegDeleteValueW(hkey, name_w.as_ptr()) };
        unsafe {
            RegCloseKey(hkey);
        }
        if rc == 0 || rc == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(PlatformError::Backend(format!(
                "RegDeleteValueW({name}) failed: {rc}"
            )))
        }
    }
}

fn to_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn query_string(hkey: isize, name: &str) -> Option<String> {
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
    if rc != 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    if end == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&buffer[..end]))
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
            (data.len() * std::mem::size_of::<u16>()) as u32,
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
