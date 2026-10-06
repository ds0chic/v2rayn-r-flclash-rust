//! Stable-port contract surface (SP-00).
//!
//! Exposes the frozen shared-contract version so every caller (Dart windows,
//! tests) can compile against the same family. The concrete contract types
//! live in `ipc_contract::stable` and are wired by the later SP cards.

/// Version of the stable shared contract family (`ipc_contract::stable`).
pub fn stable_contract_version() -> u32 {
    ipc_contract::stable::STABLE_CONTRACT_VERSION
}
