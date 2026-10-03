//! Root-certificate source selection and the OS certificate-store helper.
//!
//! Upstream v2rayN 7.25.4 (`7d6a967`) uses `GuiItem.RootCertProvider`
//! (`system | chrome | mozilla`) only to pick the trust anchors for the app's
//! own HTTPS downloads: `CertPemManager.BuildCertificateChainPolicy` returns a
//! `CustomRootTrust` policy over the bundled Chrome/Mozilla PEM collection, or
//! `null` for `system` (the OS/rustls native store). It is consumed by
//! `DownloadService` / `DownloaderHelper`; it never writes the OS store and
//! upstream has no `InstallCert` path. ResUI states the same: "仅用于 v2rayN
//! 界面程序的下载及网络请求，不影响核心的证书验证。"
//!
//! This module mirrors that selection ([`RootCertProvider`], [`trust_source`])
//! and, in addition, exposes one audited certificate-store surface
//! ([`CertificateStore`]) so any future explicit OS-trust change does not get
//! scattered across call sites. Real backends are only selected by the caller;
//! the in-memory [`FakeCertificateStore`] is the default and the only one used
//! by automated non-isolated tests.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{PlatformError, Result};

#[cfg(windows)]
pub mod windows;

/// Upstream `Global.RootCertProviders` order; the first entry is the fallback.
pub const ROOT_CERT_PROVIDERS: [&str; 3] = ["system", "chrome", "mozilla"];

/// Bundled root-store file names (upstream `Global.ChromeRootCertFileName` /
/// `Global.MozillaRootCertFileName`, loaded from embedded resources there).
pub const CHROME_ROOT_CERT_FILE: &str = "root_ca_chrome.pem";
pub const MOZILLA_ROOT_CERT_FILE: &str = "root_ca_mozilla.pem";

/// Windows root (`ROOT`) certificate store name.
pub const ROOT_STORE_NAME: &str = "ROOT";

/// `GuiItem.RootCertProvider` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootCertProvider {
    System,
    Chrome,
    Mozilla,
}

impl RootCertProvider {
    /// Upstream fallback: the first entry of `Global.RootCertProviders`.
    pub const DEFAULT: Self = Self::System;

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "system" => Some(Self::System),
            "chrome" => Some(Self::Chrome),
            "mozilla" => Some(Self::Mozilla),
            _ => None,
        }
    }

    /// Upstream `ConfigHandler.LoadConfig`: a value outside
    /// `Global.RootCertProviders` is forced to the first entry (`system`).
    pub fn normalize(value: &str) -> Self {
        Self::parse(value).unwrap_or(Self::DEFAULT)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Chrome => "chrome",
            Self::Mozilla => "mozilla",
        }
    }
}

/// Where a provider's trust anchors come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustSource {
    /// `system`: upstream returns `null` chain policy, so the OS/rustls native
    /// roots are used.
    SystemStore,
    /// `chrome` / `mozilla`: a bundled PEM collection.
    Bundled(&'static str),
}

pub fn trust_source(provider: RootCertProvider) -> TrustSource {
    match provider {
        RootCertProvider::System => TrustSource::SystemStore,
        RootCertProvider::Chrome => TrustSource::Bundled(CHROME_ROOT_CERT_FILE),
        RootCertProvider::Mozilla => TrustSource::Bundled(MOZILLA_ROOT_CERT_FILE),
    }
}

/// Upstream `CertPemManager.IsSystemRootCertProvider`.
pub fn uses_system_store(provider: RootCertProvider) -> bool {
    matches!(trust_source(provider), TrustSource::SystemStore)
}

/// Windows certificate-store scope. `CurrentUser` needs no elevation;
/// `LocalMachine` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertStoreScope {
    CurrentUser,
    LocalMachine,
}

impl CertStoreScope {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "current_user" | "user" => Some(Self::CurrentUser),
            "local_machine" | "machine" => Some(Self::LocalMachine),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CurrentUser => "current_user",
            Self::LocalMachine => "local_machine",
        }
    }

    /// `certutil` flag selecting the scope (`-user` for `CurrentUser`).
    pub fn certutil_scope_flag(self) -> Option<&'static str> {
        match self {
            Self::CurrentUser => Some("-user"),
            Self::LocalMachine => None,
        }
    }

    pub fn requires_elevation(self) -> bool {
        matches!(self, Self::LocalMachine)
    }
}

/// An exported certificate (DER bytes) plus the identity needed to address it
/// in a store. The hash is the SHA-256 of the DER body, matching upstream
/// `CertPemManager.GetCertSha256Thumbprint`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertBlob {
    pub der: Vec<u8>,
    pub thumbprint_sha256: String,
    pub subject: String,
}

impl CertBlob {
    pub fn new(der: Vec<u8>, thumbprint_sha256: String, subject: String) -> Self {
        Self {
            der,
            thumbprint_sha256,
            subject,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.der.is_empty() && !self.thumbprint_sha256.trim().is_empty()
    }
}

/// Command tokens that import `cert_path` into the root store.
///
/// Preview/audit surface only: `certutil -addstore` to the root store raises the
/// CryptUI consent dialog, so the real backend uses the silent CryptoAPI path.
pub fn install_command(scope: CertStoreScope, cert_path: &str) -> Vec<String> {
    let mut cmd = vec!["certutil".to_string()];
    if let Some(flag) = scope.certutil_scope_flag() {
        cmd.push(flag.to_string());
    }
    cmd.extend(["-addstore", "-f", ROOT_STORE_NAME, cert_path].map(str::to_string));
    cmd
}

/// Command tokens that remove a certificate by thumbprint.
pub fn remove_command(scope: CertStoreScope, thumbprint: &str) -> Vec<String> {
    let mut cmd = vec!["certutil".to_string()];
    if let Some(flag) = scope.certutil_scope_flag() {
        cmd.push(flag.to_string());
    }
    cmd.extend(["-delstore", ROOT_STORE_NAME, thumbprint].map(str::to_string));
    cmd
}

/// Command tokens that verify a certificate is present in the store.
pub fn verify_command(scope: CertStoreScope, thumbprint: &str) -> Vec<String> {
    let mut cmd = vec!["certutil".to_string()];
    if let Some(flag) = scope.certutil_scope_flag() {
        cmd.push(flag.to_string());
    }
    cmd.extend(["-store", ROOT_STORE_NAME, thumbprint].map(str::to_string));
    cmd
}

/// One audited write surface over a root certificate store.
pub trait CertificateStore: Send + Sync {
    fn scope(&self) -> CertStoreScope;
    fn install(&self, cert: &CertBlob) -> Result<()>;
    fn contains(&self, cert: &CertBlob) -> Result<bool>;
    fn remove(&self, cert: &CertBlob) -> Result<()>;
}

/// Thread-safe, in-memory store used by tests and as the default backend.
pub struct FakeCertificateStore {
    scope: CertStoreScope,
    entries: Mutex<HashMap<String, CertBlob>>,
}

impl FakeCertificateStore {
    pub fn new(scope: CertStoreScope) -> Self {
        Self {
            scope,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn key(cert: &CertBlob) -> String {
        cert.thumbprint_sha256.trim().to_ascii_lowercase()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, CertBlob>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Default for FakeCertificateStore {
    fn default() -> Self {
        Self::new(CertStoreScope::CurrentUser)
    }
}

impl CertificateStore for FakeCertificateStore {
    fn scope(&self) -> CertStoreScope {
        self.scope
    }

    fn install(&self, cert: &CertBlob) -> Result<()> {
        if !cert.is_valid() {
            return Err(PlatformError::Invalid(
                "certificate must have DER bytes and a SHA-256 thumbprint".to_string(),
            ));
        }
        self.lock().insert(Self::key(cert), cert.clone());
        Ok(())
    }

    fn contains(&self, cert: &CertBlob) -> Result<bool> {
        Ok(self.lock().contains_key(&Self::key(cert)))
    }

    fn remove(&self, cert: &CertBlob) -> Result<()> {
        let removed = self.lock().remove(&Self::key(cert));
        match removed {
            Some(_) => Ok(()),
            None => Err(PlatformError::NotFound(format!(
                "certificate {} in {} root store",
                cert.thumbprint_sha256,
                self.scope.as_str()
            ))),
        }
    }
}

/// Portable tests for provider selection, command construction and the fake
/// store. Live Windows store tests live in `tests/cert_store.rs`.
#[cfg(test)]
mod tests {
    use super::*;

    fn blob(thumbprint: &str) -> CertBlob {
        CertBlob::new(
            vec![0x30, 0x01, 0x02],
            thumbprint.to_string(),
            "CN=t".to_string(),
        )
    }

    #[test]
    fn provider_list_matches_upstream_and_normalizes() {
        assert_eq!(ROOT_CERT_PROVIDERS, ["system", "chrome", "mozilla"]);
        assert_eq!(RootCertProvider::normalize(""), RootCertProvider::System);
        assert_eq!(
            RootCertProvider::normalize("bogus"),
            RootCertProvider::System
        );
        assert_eq!(
            RootCertProvider::normalize(" Chrome "),
            RootCertProvider::Chrome
        );
        assert_eq!(RootCertProvider::Mozilla.as_str(), "mozilla");
    }

    #[test]
    fn trust_source_maps_providers() {
        assert_eq!(
            trust_source(RootCertProvider::System),
            TrustSource::SystemStore
        );
        assert!(uses_system_store(RootCertProvider::System));
        assert!(!uses_system_store(RootCertProvider::Chrome));
        assert_eq!(
            trust_source(RootCertProvider::Chrome),
            TrustSource::Bundled(CHROME_ROOT_CERT_FILE)
        );
        assert_eq!(
            trust_source(RootCertProvider::Mozilla),
            TrustSource::Bundled(MOZILLA_ROOT_CERT_FILE)
        );
    }

    #[test]
    fn command_construction_is_scoped() {
        assert_eq!(
            install_command(CertStoreScope::CurrentUser, "C:\\c.cer"),
            vec!["certutil", "-user", "-addstore", "-f", "ROOT", "C:\\c.cer"]
        );
        assert_eq!(
            install_command(CertStoreScope::LocalMachine, "C:\\c.cer"),
            vec!["certutil", "-addstore", "-f", "ROOT", "C:\\c.cer"]
        );
        assert_eq!(
            remove_command(CertStoreScope::CurrentUser, "AABB"),
            vec!["certutil", "-user", "-delstore", "ROOT", "AABB"]
        );
        assert_eq!(
            verify_command(CertStoreScope::LocalMachine, "AABB"),
            vec!["certutil", "-store", "ROOT", "AABB"]
        );
        assert!(CertStoreScope::LocalMachine.requires_elevation());
        assert!(!CertStoreScope::CurrentUser.requires_elevation());
    }

    #[test]
    fn fake_store_write_read_restore_round_trip() {
        let store = FakeCertificateStore::new(CertStoreScope::CurrentUser);
        let cert = blob("AA11");
        assert!(!store.contains(&cert).unwrap());
        store.install(&cert).unwrap();
        assert!(store.contains(&cert).unwrap());
        store.remove(&cert).unwrap();
        assert!(!store.contains(&cert).unwrap());
        // Removing twice is a NotFound error branch.
        assert!(matches!(
            store.remove(&cert),
            Err(PlatformError::NotFound(_))
        ));
    }

    #[test]
    fn fake_store_rejects_invalid_certificate() {
        let store = FakeCertificateStore::default();
        let empty = CertBlob::new(Vec::new(), String::new(), String::new());
        assert!(matches!(
            store.install(&empty),
            Err(PlatformError::Invalid(_))
        ));
    }
}
