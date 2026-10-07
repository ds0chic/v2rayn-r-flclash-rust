//! Platform backends for the v2rayN Rust rewrite (T13).
//!
//! This crate owns OS-facing side effects: the per-field Windows system proxy
//! settings, the local PAC HTTP server, the Windows Run-key autostart entry and
//! the single-instance guard. It is deliberately dependency-free beyond
//! `serde` so the workspace `--locked` gate keeps working; the Windows
//! implementations call the Win32 APIs through handwritten FFI instead of the
//! `windows` crate.
//!
//! Boundary rule (AGENTS.md): tests must never touch the host system proxy or
//! the registry. Real backends are compile-only here; behaviour is exercised
//! through the in-memory fakes and loopback PAC server.

pub mod autostart;
pub mod cert;
pub mod error;
pub mod hash;
pub mod http;
pub mod pac;
pub mod script;
pub mod single_instance;
pub mod sysproxy;

pub use error::{PlatformError, Result};
pub use http::{
    apply_trust as apply_http_trust, clear_cache_for_tests, construction_count, fetch_bytes,
    fetch_text, reset_construction_count_for_tests, FetchOptions, Fetched, HttpError, HttpPolicy,
    HttpsTrust as HttpTrust, RedirectPolicy, SharedHttpClient,
    DEFAULT_MAX_BYTES as HTTP_DEFAULT_MAX_BYTES,
    DEFAULT_MAX_REDIRECTS as HTTP_DEFAULT_MAX_REDIRECTS, DEFAULT_USER_AGENT,
};

pub use autostart::{
    decode_run_command, encode_run_command, run_value_name, AutoStartBackend, AutoStartEntry,
    FakeRegistry, AUTO_RUN_NAME,
};
pub use cert::{
    install_command, remove_command, trust_source, uses_system_store, verify_command, CertBlob,
    CertStoreScope, CertificateStore, FakeCertificateStore, RootCertProvider, TrustSource,
    CHROME_ROOT_CERT_FILE, MOZILLA_ROOT_CERT_FILE, ROOT_CERT_PROVIDERS, ROOT_STORE_NAME,
};
pub use pac::{
    render_pac, resolve_pac_path, resolve_pac_script, PacConfig, PacServer, PacSource, ResolvedPac,
    DEFAULT_PAC_FILE_NAME, DEFAULT_PAC_PORT_BASE, DEFAULT_PAC_TEMPLATE,
};
pub use script::{CustomSystemProxySetting, CUSTOM_PAC_FIELD, CUSTOM_SCRIPT_FIELD};
pub use single_instance::SingleInstanceGuard;
pub use sysproxy::{
    restore_if_owned, AppliedChange, FakeSystemProxyBackend, ProxyField, ProxySettings, ProxyState,
    RestoreAction, RestoreConflict, RestoreReport, SysProxyMode, SystemProxyBackend,
};

#[cfg(windows)]
pub use autostart::windows::WindowsRunKeyBackend;
#[cfg(windows)]
pub use cert::windows::WindowsCertificateStore;
#[cfg(windows)]
pub use sysproxy::windows::WindowsSystemProxyBackend;
