//! Single-instance guard.
//!
//! Wraps a named Windows mutex, matching the T01 feasibility probe
//! (`CreateMutexW` + `ERROR_ALREADY_EXISTS`). The mutex name controls the
//! namespace: `Global\...` for a machine-wide instance, `Local\...` for the
//! current session.
//!
//! The `windows` crate is intentionally avoided here (see the module docs in
//! `lib.rs`); the same kernel32 calls are used through FFI.

use crate::error::{PlatformError, Result};

/// Default machine-wide instance name.
pub const DEFAULT_INSTANCE_NAME: &str = "Global\\v2rayN-R-single-instance";

/// A held single-instance mutex. Dropping it releases the instance.
pub struct SingleInstanceGuard {
    name: String,
    #[cfg(windows)]
    handle: *mut core::ffi::c_void,
}

// The guard only owns an OS handle; it is safe to move/share across threads.
unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl SingleInstanceGuard {
    /// Acquire the named instance or fail with
    /// [`PlatformError::AlreadyRunning`].
    pub fn acquire(name: &str) -> Result<Self> {
        if name.is_empty() {
            return Err(PlatformError::Invalid(
                "single-instance name must not be empty".to_string(),
            ));
        }
        imp::acquire(name)
    }

    /// Acquire [`DEFAULT_INSTANCE_NAME`].
    pub fn acquire_default() -> Result<Self> {
        Self::acquire(DEFAULT_INSTANCE_NAME)
    }

    /// The mutex name held by this guard.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl core::fmt::Debug for SingleInstanceGuard {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SingleInstanceGuard")
            .field("name", &self.name)
            .finish()
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        imp::release(self);
    }
}

#[cfg(windows)]
mod imp {
    use core::ffi::c_void;

    use super::SingleInstanceGuard;
    use crate::error::{PlatformError, Result};

    const ERROR_ALREADY_EXISTS: u32 = 183;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateMutexW(
            attributes: *mut c_void,
            initial_owner: i32,
            name: *const u16,
        ) -> *mut c_void;
        fn GetLastError() -> u32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    fn to_wide(name: &str) -> Vec<u16> {
        name.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn acquire(name: &str) -> Result<SingleInstanceGuard> {
        let wide = to_wide(name);
        let handle = unsafe { CreateMutexW(core::ptr::null_mut(), 0, wide.as_ptr()) };
        if handle.is_null() {
            return Err(PlatformError::Backend(
                "CreateMutexW returned a null handle".to_string(),
            ));
        }
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(handle);
            }
            return Err(PlatformError::AlreadyRunning(name.to_string()));
        }
        Ok(SingleInstanceGuard {
            name: name.to_string(),
            handle,
        })
    }

    pub(super) fn release(guard: &mut SingleInstanceGuard) {
        if !guard.handle.is_null() {
            unsafe {
                CloseHandle(guard.handle);
            }
            guard.handle = core::ptr::null_mut();
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SingleInstanceGuard;
    use crate::error::{PlatformError, Result};

    pub(super) fn acquire(_name: &str) -> Result<SingleInstanceGuard> {
        Err(PlatformError::Unsupported(
            "single-instance guard needs a named kernel object (Windows only)",
        ))
    }

    pub(super) fn release(_guard: &mut SingleInstanceGuard) {}
}
