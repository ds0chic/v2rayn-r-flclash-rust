//! Silent Windows root-store backend via the CryptoAPI (`crypt32.dll`).
//!
//! `certutil -addstore` on the `ROOT` store raises the CryptUI consent dialog,
//! which cannot run unattended; `CertAddEncodedCertificateToStore` does not, so
//! the real backend uses it directly. Writes happen only when a caller
//! explicitly constructs this backend, mirroring the system-proxy boundary.

use std::os::raw::{c_char, c_void};

use super::{CertBlob, CertStoreScope, CertificateStore};
use crate::error::{PlatformError, Result};

type Hcertstore = *mut c_void;
type PccertContext = *mut CcertContext;

#[repr(C)]
struct CcertContext {
    dw_cert_encoding_type: u32,
    pb_cert_encoded: *const u8,
    cb_cert_encoded: u32,
    p_cert_info: *mut c_void,
    h_store: Hcertstore,
}

const CERT_STORE_PROV_SYSTEM_W: *const c_char = 10usize as *const c_char;
const CERT_SYSTEM_STORE_CURRENT_USER: u32 = 0x0001_0000;
const CERT_SYSTEM_STORE_LOCAL_MACHINE: u32 = 0x0002_0000;
const CERT_STORE_ADD_REPLACE_EXISTING: u32 = 3;
const X509_ASN_ENCODING: u32 = 0x0000_0001;
const PKCS_7_ASN_ENCODING: u32 = 0x0001_0000;

#[link(name = "crypt32")]
extern "system" {
    fn CertOpenStore(
        lpsz_store_provider: *const c_char,
        dw_encoding_type: u32,
        h_crypt_prov: usize,
        dw_flags: u32,
        pv_para: *const u16,
    ) -> Hcertstore;
    fn CertCloseStore(h_cert_store: Hcertstore, dw_flags: u32) -> i32;
    fn CertAddEncodedCertificateToStore(
        h_cert_store: Hcertstore,
        dw_cert_encoding_type: u32,
        pb_cert_encoded: *const u8,
        cb_cert_encoded: u32,
        dw_add_disposition: u32,
        pp_cert_context: *mut PccertContext,
    ) -> i32;
    fn CertEnumCertificatesInStore(
        h_cert_store: Hcertstore,
        p_prev_cert_context: PccertContext,
    ) -> PccertContext;
    fn CertDeleteCertificateFromStore(p_cert_context: PccertContext) -> i32;
    fn CertFreeCertificateContext(p_cert_context: PccertContext) -> i32;
}

/// Windows implementation of [`CertificateStore`] over the `ROOT` store.
#[derive(Debug, Clone, Copy)]
pub struct WindowsCertificateStore {
    scope: CertStoreScope,
}

impl WindowsCertificateStore {
    pub fn new(scope: CertStoreScope) -> Self {
        Self { scope }
    }

    fn open(&self) -> Result<Hcertstore> {
        let name: Vec<u16> = "ROOT\0".encode_utf16().collect();
        let flags = match self.scope {
            CertStoreScope::CurrentUser => CERT_SYSTEM_STORE_CURRENT_USER,
            CertStoreScope::LocalMachine => CERT_SYSTEM_STORE_LOCAL_MACHINE,
        };
        let store = unsafe { CertOpenStore(CERT_STORE_PROV_SYSTEM_W, 0, 0, flags, name.as_ptr()) };
        if store.is_null() {
            return Err(PlatformError::Io(std::io::Error::last_os_error()));
        }
        Ok(store)
    }

    /// Return the first context whose DER body equals `der`. The caller owns the
    /// returned context and must free or delete it.
    unsafe fn find_context(store: Hcertstore, der: &[u8]) -> Option<PccertContext> {
        let mut ctx = CertEnumCertificatesInStore(store, std::ptr::null_mut());
        while !ctx.is_null() {
            let cert = &*ctx;
            let bytes =
                std::slice::from_raw_parts(cert.pb_cert_encoded, cert.cb_cert_encoded as usize);
            if bytes == der {
                return Some(ctx);
            }
            ctx = CertEnumCertificatesInStore(store, ctx);
        }
        None
    }
}

impl CertificateStore for WindowsCertificateStore {
    fn scope(&self) -> CertStoreScope {
        self.scope
    }

    fn install(&self, cert: &CertBlob) -> Result<()> {
        if !cert.is_valid() {
            return Err(PlatformError::Invalid(
                "certificate must have DER bytes and a SHA-256 thumbprint".to_string(),
            ));
        }
        let store = self.open()?;
        let mut added: PccertContext = std::ptr::null_mut();
        let ok = unsafe {
            CertAddEncodedCertificateToStore(
                store,
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                cert.der.as_ptr(),
                cert.der.len() as u32,
                CERT_STORE_ADD_REPLACE_EXISTING,
                &mut added,
            )
        };
        if !added.is_null() {
            unsafe { CertFreeCertificateContext(added) };
        }
        unsafe { CertCloseStore(store, 0) };
        if ok == 0 {
            return Err(PlatformError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn contains(&self, cert: &CertBlob) -> Result<bool> {
        let store = self.open()?;
        let found = unsafe { Self::find_context(store, &cert.der) };
        if let Some(ctx) = found {
            unsafe { CertFreeCertificateContext(ctx) };
        }
        unsafe { CertCloseStore(store, 0) };
        Ok(found.is_some())
    }

    fn remove(&self, cert: &CertBlob) -> Result<()> {
        let store = self.open()?;
        let found = unsafe { Self::find_context(store, &cert.der) };
        let Some(ctx) = found else {
            unsafe { CertCloseStore(store, 0) };
            return Err(PlatformError::NotFound(format!(
                "certificate {} in {} root store",
                cert.thumbprint_sha256,
                self.scope.as_str()
            )));
        };
        let ok = unsafe { CertDeleteCertificateFromStore(ctx) };
        if ok == 0 {
            unsafe { CertFreeCertificateContext(ctx) };
        }
        unsafe { CertCloseStore(store, 0) };
        if ok == 0 {
            return Err(PlatformError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}
