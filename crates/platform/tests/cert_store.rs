//! Live certificate-store test.
//!
//! The portable provider/command/fake coverage lives in the crate's unit tests.
//! This file adds the one real OS side effect the FIX-16E card authorises: with
//! a synthetic self-signed certificate, write the current-user `ROOT` store,
//! read it back, then delete it. It is `#[ignore]`d and additionally requires
//! `V2RAYN_R_CERT_STORE_TEST=1`, so neither `cargo test` nor the workspace gate
//! can mutate the host store by accident.

#[cfg(windows)]
mod windows_live {
    use platform::{CertBlob, CertStoreScope, CertificateStore, WindowsCertificateStore};

    const SYNTHETIC_DER: &[u8] = include_bytes!("fixtures/synthetic-root.cer");
    const SYNTHETIC_SHA256: &str =
        "6435cc53fe1b4e23bf3714482da5777fba2d8089f3767bbd1645728ab8e458fa";

    #[test]
    #[ignore = "writes the current-user ROOT store; run with --ignored and V2RAYN_R_CERT_STORE_TEST=1"]
    fn current_user_root_store_write_read_restore() {
        if std::env::var("V2RAYN_R_CERT_STORE_TEST").ok().as_deref() != Some("1") {
            eprintln!("skipping: set V2RAYN_R_CERT_STORE_TEST=1 to run the live store test");
            return;
        }

        let cert = CertBlob::new(
            SYNTHETIC_DER.to_vec(),
            SYNTHETIC_SHA256.to_string(),
            "CN=v2rayn-R-FIX-16E-Synthetic".to_string(),
        );
        let store = WindowsCertificateStore::new(CertStoreScope::CurrentUser);

        let before = store.contains(&cert).expect("open current-user ROOT store");
        assert!(!before, "synthetic certificate must not pre-exist");

        store.install(&cert).expect("install synthetic certificate");
        assert!(
            store.contains(&cert).expect("read back after install"),
            "certificate must be readable after install"
        );

        store.remove(&cert).expect("remove synthetic certificate");
        assert!(
            !store.contains(&cert).expect("read back after remove"),
            "certificate must be gone after remove"
        );
    }
}
