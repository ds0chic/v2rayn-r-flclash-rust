//! Reversible protection for small secrets at rest (H19 ownership journal).
//!
//! [`ProtectBackend`] abstracts the mechanism: [`DpapiBackend`] (Windows,
//! current-user DPAPI via handwritten `crypt32` FFI, no new dependency),
//! [`PlainBackend`] (identity transform for non-Windows builds; the journal
//! file itself carries 0600 permissions there) and [`FakeProtectBackend`]
//! (tests; a keyed transform so wrong-key decryption fails deterministically).
//! Nothing here logs or surfaces plaintext; errors are opaque labels.

use crate::error::{PlatformError, Result};

/// Reversible protection of small byte strings.
pub trait ProtectBackend: Send + Sync {
    /// Short algorithm label stored in the journal envelope (`dpapi-user`,
    /// `plain`, `fake`). Never a key.
    fn label(&self) -> &'static str;
    /// Protect `plaintext` for storage under the current user/context.
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    /// Reverse [`ProtectBackend::protect`]. Any mismatch (wrong key, tampered
    /// bytes, foreign blob) is an opaque error, never partial plaintext.
    fn unprotect(&self, blob: &[u8]) -> Result<Vec<u8>>;
}

/// Identity transform. Production use is non-Windows only, where the journal
/// file relies on filesystem permissions (0600); tests use it everywhere.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlainBackend;

impl ProtectBackend for PlainBackend {
    fn label(&self) -> &'static str {
        "plain"
    }

    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, blob: &[u8]) -> Result<Vec<u8>> {
        Ok(blob.to_vec())
    }
}

/// Keyed toy transform for tests. The first blob byte records the key, so a
/// backend constructed with a different key fails closed like a real
/// wrong-key decrypt.
#[derive(Debug, Clone, Copy)]
pub struct FakeProtectBackend {
    key: u8,
}

impl FakeProtectBackend {
    pub fn new(key: u8) -> Self {
        Self { key }
    }
}

impl ProtectBackend for FakeProtectBackend {
    fn label(&self) -> &'static str {
        "fake"
    }

    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut blob = Vec::with_capacity(plaintext.len() + 1);
        blob.push(self.key);
        blob.extend(plaintext.iter().map(|b| b.wrapping_add(self.key)));
        Ok(blob)
    }

    fn unprotect(&self, blob: &[u8]) -> Result<Vec<u8>> {
        let (marker, body) = blob
            .split_first()
            .ok_or_else(|| PlatformError::Backend("protected blob is empty".to_string()))?;
        if *marker != self.key {
            return Err(PlatformError::Backend(
                "protected blob key mismatch".to_string(),
            ));
        }
        Ok(body.iter().map(|b| b.wrapping_sub(self.key)).collect())
    }
}

/// Current-user DPAPI via `crypt32.dll` (Windows only).
#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub struct DpapiBackend;

#[cfg(windows)]
mod imp {
    use core::ffi::c_void;

    use super::DpapiBackend;
    use crate::error::{PlatformError, Result};
    use crate::protect::ProtectBackend;

    /// `CRYPTPROTECT_UI_FORBIDDEN`: never prompt; unattended use only.
    const UI_FORBIDDEN: u32 = 0x1;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            data_in: *const DataBlob,
            descr: *const u16,
            entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const DataBlob,
            descr: *mut *mut u16,
            entropy: *const DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(mem: *mut c_void) -> *mut c_void;
    }

    /// Copy a DPAPI out-blob into Rust memory and free the native buffer.
    ///
    /// # Safety
    /// `blob.pb_data` must be a `LocalAlloc`-owned buffer of `cb_data` bytes
    /// as returned by `CryptProtectData`/`CryptUnprotectData` (null when
    /// `cb_data` is 0). The pointer is freed exactly once here.
    unsafe fn take_blob(blob: &DataBlob) -> Vec<u8> {
        if blob.cb_data == 0 || blob.pb_data.is_null() {
            return Vec::new();
        }
        // SAFETY: upheld by the caller contract above; the slice is copied
        // before the free, so no use-after-free is possible below.
        let bytes = unsafe { std::slice::from_raw_parts(blob.pb_data, blob.cb_data as usize) };
        let out = bytes.to_vec();
        // SAFETY: same contract; `LocalFree` exactly once per out-blob.
        unsafe {
            LocalFree(blob.pb_data as *mut c_void);
        }
        out
    }

    impl ProtectBackend for DpapiBackend {
        fn label(&self) -> &'static str {
            "dpapi-user"
        }

        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
            if plaintext.is_empty() {
                return Ok(Vec::new());
            }
            let input = DataBlob {
                cb_data: plaintext.len() as u32,
                pb_data: plaintext.as_ptr() as *mut u8,
            };
            let mut output = DataBlob {
                cb_data: 0,
                pb_data: core::ptr::null_mut(),
            };
            // SAFETY: `input` borrows live caller bytes; all optional
            // parameters are null; `output` is a fresh out-blob taken over
            // with `take_blob` on success only.
            let ok = unsafe {
                CryptProtectData(
                    &input,
                    core::ptr::null(),
                    core::ptr::null(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    UI_FORBIDDEN,
                    &mut output,
                )
            };
            if ok == 0 {
                return Err(PlatformError::Backend("DPAPI protect failed".to_string()));
            }
            // SAFETY: `output` is a fresh success out-blob per the API contract.
            Ok(unsafe { take_blob(&output) })
        }

        fn unprotect(&self, blob: &[u8]) -> Result<Vec<u8>> {
            if blob.is_empty() {
                return Ok(Vec::new());
            }
            let input = DataBlob {
                cb_data: blob.len() as u32,
                pb_data: blob.as_ptr() as *mut u8,
            };
            let mut output = DataBlob {
                cb_data: 0,
                pb_data: core::ptr::null_mut(),
            };
            // SAFETY: same contract as `protect`; a tampered or foreign blob
            // fails here and yields no output buffer to free.
            let ok = unsafe {
                CryptUnprotectData(
                    &input,
                    core::ptr::null_mut(),
                    core::ptr::null(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    UI_FORBIDDEN,
                    &mut output,
                )
            };
            if ok == 0 {
                return Err(PlatformError::Backend("DPAPI unprotect failed".to_string()));
            }
            // SAFETY: `output` is a fresh success out-blob per the API contract.
            Ok(unsafe { take_blob(&output) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_is_identity() {
        let backend = PlainBackend;
        assert_eq!(backend.label(), "plain");
        assert_eq!(backend.protect(b"abc").unwrap(), b"abc");
        assert_eq!(backend.unprotect(b"abc").unwrap(), b"abc");
    }

    #[test]
    fn fake_roundtrips_and_rejects_foreign_key() {
        let backend = FakeProtectBackend::new(7);
        let blob = backend.protect(b"server=127.0.0.1:11809").unwrap();
        assert_eq!(backend.unprotect(&blob).unwrap(), b"server=127.0.0.1:11809");
        let other = FakeProtectBackend::new(9);
        assert!(other.unprotect(&blob).is_err());
        assert!(backend.unprotect(&[]).is_err());
    }

    /// Real DPAPI round-trip plus tamper rejection. Memory-only: no system
    /// proxy, registry or file is touched.
    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrips_and_rejects_tampering() {
        let backend = DpapiBackend;
        assert_eq!(backend.label(), "dpapi-user");
        let plaintext = b"proxy before-value canary";
        let blob = backend.protect(plaintext).expect("protect");
        assert!(!blob.is_empty());
        assert_ne!(blob.as_slice(), plaintext.as_slice());
        assert_eq!(backend.unprotect(&blob).expect("unprotect"), plaintext);
        let mut tampered = blob.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(
            backend.unprotect(&tampered).is_err(),
            "tampered DPAPI blob must fail closed"
        );
    }
}
