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
    ///
    /// A `Global\` name that the caller may not create (standard user token)
    /// falls back to the session-local [`fallback_name`] instead of failing
    /// outright; the guard reports the name actually held via [`Self::name`].
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

/// Session-local fallback for a `Global\` instance name (S7b-09).
///
/// Creating `Global\` objects needs a privilege a standard user token lacks;
/// callers that cannot create the machine-wide mutex guard the current
/// session instead. Non-`Global\` names (including `Local\`) have no fallback.
pub fn fallback_name(name: &str) -> Option<String> {
    let rest = name
        .strip_prefix("Global\\")
        .or_else(|| name.strip_prefix("GLOBAL\\"))?;
    if rest.is_empty() {
        return None;
    }
    Some(format!("Local\\{rest}"))
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

    use super::{fallback_name, SingleInstanceGuard};
    use crate::error::{PlatformError, Result};

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const ERROR_ACCESS_DENIED: u32 = 5;

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
        match try_create(name) {
            CreateOutcome::Held(guard) => Ok(guard),
            CreateOutcome::AlreadyRunning => Err(PlatformError::AlreadyRunning(name.to_string())),
            // A standard user token cannot create `Global\` objects: guard
            // the session instead. Any other failure (bad name, …) is
            // reported as-is so real errors are never masked by the fallback.
            CreateOutcome::AccessDenied => {
                let Some(fallback) = fallback_name(name) else {
                    return Err(PlatformError::Backend(format!(
                        "CreateMutexW({name}) failed: access denied"
                    )));
                };
                match try_create(&fallback) {
                    CreateOutcome::Held(guard) => Ok(guard),
                    CreateOutcome::AlreadyRunning => Err(PlatformError::AlreadyRunning(fallback)),
                    _ => Err(PlatformError::Backend(format!(
                        "CreateMutexW({fallback}) failed"
                    ))),
                }
            }
            CreateOutcome::Failed => Err(PlatformError::Backend(format!(
                "CreateMutexW({name}) returned a null handle"
            ))),
        }
    }

    enum CreateOutcome {
        Held(SingleInstanceGuard),
        AlreadyRunning,
        AccessDenied,
        Failed,
    }

    fn try_create(name: &str) -> CreateOutcome {
        let wide = to_wide(name);
        // SAFETY: `wide` is a live nul-terminated UTF-16 buffer for the call;
        // no ownership is transferred and the handle is closed on every error
        // path below.
        let handle = unsafe { CreateMutexW(core::ptr::null_mut(), 0, wide.as_ptr()) };
        if handle.is_null() {
            // SAFETY: `GetLastError` is valid immediately after the failed call.
            if unsafe { GetLastError() } == ERROR_ACCESS_DENIED {
                return CreateOutcome::AccessDenied;
            }
            return CreateOutcome::Failed;
        }
        // SAFETY: same; queried before any other fallible Win32 call.
        let last_error = unsafe { GetLastError() };
        if last_error == ERROR_ALREADY_EXISTS {
            // SAFETY: `handle` is ours from `CreateMutexW` above.
            unsafe {
                CloseHandle(handle);
            }
            return CreateOutcome::AlreadyRunning;
        }
        CreateOutcome::Held(SingleInstanceGuard {
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
