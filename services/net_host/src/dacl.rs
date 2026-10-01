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
fn io_err<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// The current user's SID as an SDDL string (e.g. `S-1-5-21-...`).
#[cfg(windows)]
fn current_user_sid_string() -> std::io::Result<String> {
    use core::ffi::c_void;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
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

/// Rewrite a file/directory DACL to grant full control only to the current
/// user. Best-effort: callers log but never abort a session on failure. When
/// `directory` is set, the ACE is inheritable so files created inside inherit
/// the restriction.
#[cfg(windows)]
pub fn restrict_to_current_user(path: &std::path::Path, directory: bool) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, BOOL, HLOCAL};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows::Win32::Security::{
        GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };

    let sid = current_user_sid_string()?;
    let ace = if directory {
        format!("D:P(A;OICI;GA;;;{sid})")
    } else {
        format!("D:P(A;;GA;;;{sid})")
    };
    let sddl: Vec<u16> = ace.encode_utf16().chain(std::iter::once(0)).collect();
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .map_err(io_err)?;

        let mut present = BOOL(0);
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut defaulted = BOOL(0);
        let got = GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted);
        if let Err(e) = got {
            let _ = LocalFree(HLOCAL(sd.0));
            return Err(io_err(e));
        }

        let result = SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            PSID(std::ptr::null_mut()),
            PSID(std::ptr::null_mut()),
            if dacl.is_null() { None } else { Some(dacl) },
            None,
        );
        let _ = LocalFree(HLOCAL(sd.0));
        if !result.is_ok() {
            return Err(std::io::Error::from_raw_os_error(result.0 as i32));
        }
    }
    Ok(())
}

#[cfg(windows)]
impl PipeSecurity {
    /// Build a descriptor granting `GENERIC_ALL` only to the current user.
    pub fn current_user_only() -> std::io::Result<Self> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::FALSE;
        use windows::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
        };
        use windows::Win32::Security::SECURITY_ATTRIBUTES;

        unsafe {
            let sid = current_user_sid_string()?;
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

/// Non-Windows placeholder (net-host process ownership is Windows-only in T03).
#[cfg(not(windows))]
pub fn restrict_to_current_user(_path: &std::path::Path, _directory: bool) -> std::io::Result<()> {
    Ok(())
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

    #[cfg(windows)]
    #[test]
    fn restrict_to_current_user_applies_to_file_and_dir() {
        let root = std::env::temp_dir().join(format!(
            "v2rayn-t03-dacl-{}",
            super::super::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        restrict_to_current_user(&root, true).expect("restrict dir");
        let file = root.join("config.json");
        std::fs::write(&file, b"{}").unwrap();
        restrict_to_current_user(&file, false).expect("restrict file");
        let _ = std::fs::remove_dir_all(&root);
    }
}
