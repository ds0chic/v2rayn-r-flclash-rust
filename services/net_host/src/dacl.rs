//! Named-pipe DACL restricted to the current user (plan §5).
//!
//! tokio's `ServerOptions` does not expose the security descriptor, so the
//! pipe is created with an explicit `SECURITY_ATTRIBUTES` built from an SDDL
//! granting full access only to the SID of the user running net-host.

#[cfg(windows)]
pub struct PipeSecurity {
    sd: windows::Win32::Security::PSECURITY_DESCRIPTOR,
    sa: windows::Win32::Security::SECURITY_ATTRIBUTES,
}

#[cfg(windows)]
// The security descriptor is owned by this struct and only read during
// `CreateNamedPipeW`; moving it between threads does not alias the pointer.
unsafe impl Send for PipeSecurity {}
#[cfg(windows)]
unsafe impl Sync for PipeSecurity {}

#[cfg(windows)]
impl PipeSecurity {
    /// Build a descriptor granting `GENERIC_ALL` only to the current user.
    pub fn current_user_only() -> std::io::Result<Self> {
        use core::ffi::c_void;
        use windows::core::{PCWSTR, PWSTR};
        use windows::Win32::Foundation::{CloseHandle, LocalFree, FALSE, HANDLE, HLOCAL};
        use windows::Win32::Security::Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        };
        use windows::Win32::Security::{
            GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
            std::io::Error::other(e.to_string())
        }

        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(io_err)?;

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
            let _ = CloseHandle(token);

            let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut sd = windows::Win32::Security::PSECURITY_DESCRIPTOR(std::ptr::null_mut());
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

#[cfg(windows)]
impl Drop for PipeSecurity {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        unsafe {
            let _ = LocalFree(HLOCAL(self.sd.0));
        }
    }
}

/// Non-Windows placeholder (T03 is Windows-only).
#[cfg(not(windows))]
#[derive(Default)]
pub struct PipeSecurity;

#[cfg(not(windows))]
impl PipeSecurity {
    pub fn current_user_only() -> std::io::Result<Self> {
        Ok(Self)
    }

    pub fn as_mut_ptr(&self) -> *mut core::ffi::c_void {
        std::ptr::null_mut()
    }
}

#[cfg(windows)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn current_user_descriptor_is_built_and_freed() {
        let security = PipeSecurity::current_user_only().expect("build DACL");
        // The SECURITY_ATTRIBUTES pointer handed to CreateNamedPipeW is valid
        // for the lifetime of the guard.
        assert!(!security.as_mut_ptr().is_null());
        drop(security);
    }

    #[test]
    fn non_windows_placeholder_builds() {
        // Compiles and returns an empty guard on non-Windows.
        #[cfg(not(windows))]
        {
            assert!(PipeSecurity::current_user_only().is_ok());
        }
    }
}
