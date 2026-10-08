//! SP28-L1-002: a profile certificate CHAIN (leaf + CA) survives
//! save/reopen byte-for-byte, and reaches the generator model verbatim.
//! Fixtures are synthetic base64 bodies only; no real certificates.

use std::sync::Arc;

use application::codegen::to_codegen_profile;
use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::{ConfigType, Profile};

/// Synthetic leaf body ("Leaf").
const LEAF_BODY: &str = "TGVhZg==";
/// Synthetic CA body ("CA").
const CA_BODY: &str = "Q0E=";

fn chain_cert() -> String {
    format!(
        "-----BEGIN CERTIFICATE-----\n{LEAF_BODY}\n-----END CERTIFICATE-----\n\
         -----BEGIN CERTIFICATE-----\n{CA_BODY}\n-----END CERTIFICATE-----\n"
    )
}

fn chain_profile() -> Profile {
    Profile {
        index_id: "cert-chain-1".into(),
        config_type: ConfigType::Vless,
        remarks: "chain-node".into(),
        address: "192.0.2.44".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        network: "raw".into(),
        security: domain::SecurityParams {
            stream_security: Some("tls".into()),
            cert: Some(chain_cert()),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn cert_chain_round_trips_save_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let expected = chain_cert();
    let saved = {
        let engine = AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new()))
            .expect("open engine");
        let revision = engine.snapshot().unwrap().revisions.desired;
        let (saved, _) = engine.save_profile(chain_profile(), revision).unwrap();
        // Save normalization must not touch certificate material.
        assert_eq!(saved.security.cert.as_deref(), Some(expected.as_str()));
        saved
    };
    drop(saved);
    // Fresh engine over the same directory: independent-process semantics.
    let reopened = AppEngine::open_with_runtime(dir.path(), Arc::new(NullRuntimeClient::new()))
        .expect("reopen engine");
    let loaded = reopened
        .profile_by_id("cert-chain-1")
        .unwrap()
        .expect("profile present");
    assert_eq!(loaded.security.cert.as_deref(), Some(expected.as_str()));
    // Leaf stays first, CA second: the stored order is the emitted order.
    let stored = loaded.security.cert.unwrap();
    assert!(stored.find(LEAF_BODY).unwrap() < stored.find(CA_BODY).unwrap());
}

#[test]
fn cert_chain_reaches_codegen_model_verbatim() {
    let node = to_codegen_profile(&chain_profile(), None);
    assert_eq!(node.cert, chain_cert());
    assert_eq!(node.stream_security, "tls");
}
