//! Named-pipe DACL restricted to the current user (plan §5).
//!
//! Mirrors the net-host transport: tokio's `ServerOptions` does not expose the
//! security descriptor, so the pipe is created with an explicit
//! `SECURITY_ATTRIBUTES` built from an SDDL granting full access only to the
//! SID of the user running the helper.

use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::PSECURITY_DESCRIPTOR;
use windows::Win32::Security::SECURITY_ATTRIBUTES;

pub struct PipeSecurity {
    sd: PSECURITY_DESCRIPTOR,
    sa: SECURITY_ATTRIBUTES,
}

// The descriptor is owned by this struct and only read during
// `CreateNamedPipeW`; moving it between threads does not alias the pointer.
unsafe impl Send for PipeSecurity {}
unsafe impl Sync for PipeSecurity {}

fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// The current user's SID as an SDDL string (e.g. `S-1-5-21-...`).
pub fn current_user_sid_string() -> std::io::Result<String> {
    use core::ffi::c_void;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(io_err)?;

        let mut buf = vec![0u64; 64];
        let mut ret = 0u32;
        let info = GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut c_void),
            (buf.len() * 8) as u32,
            &mut ret,
        );
        let _ = CloseHandle(token);
        info.map_err(io_err)?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_str = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid_str).map_err(io_err)?;
        let sid = pwstr_to_string(sid_str).ok_or_else(|| io_err("invalid SID string"))?;
        let _ = LocalFree(HLOCAL(sid_str.0 as *mut c_void));
        Ok(sid)
    }
}

impl PipeSecurity {
    /// Build a descriptor granting `GENERIC_ALL` only to the current user.
    pub fn current_user_only() -> std::io::Result<Self> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::FALSE;

        unsafe {
            let sid = current_user_sid_string()?;
            let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut sd = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
            let mut size = 0u32;
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut sd,
                Some(&mut size),
            )
            .map_err(io_err)?;

            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd.0,
                bInheritHandle: FALSE,
            };
            Ok(Self { sd, sa })
        }
    }

    /// Raw pointer to pass to `create_with_security_attributes_raw`.
    pub fn as_mut_ptr(&self) -> *mut core::ffi::c_void {
        &self.sa as *const _ as *mut core::ffi::c_void
    }
}

impl Drop for PipeSecurity {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(HLOCAL(self.sd.0));
        }
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
