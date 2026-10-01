//! Runtime session orchestration primitives (T03).
//!
//! This crate is the platform-facing half of the runtime boundary:
//!
//! - [`adapter`] — core executable location and command construction;
//! - [`graph`] — deterministic start/stop/rollback order for `ProcessGraph`;
//! - [`identity`] — `(pid, creation_time)` process identity;
//! - [`job`] — Windows Job Object ownership (`KILL_ON_JOB_CLOSE`);
//! - [`sha256`] — config hashing without extra crates;
//! - [`wire`] — shared length-prefixed IPC frame helpers.
//!
//! It contains no business/domain policy and never touches the UI.

// `DomainError` is the shared error contract; boxing it would ripple through
// every signature. See the same allowance in `domain`/`application`.
#![allow(clippy::result_large_err)]

pub mod adapter;
pub mod graph;
pub mod identity;
pub mod job;
pub mod sha256;
pub mod wire;

pub use adapter::{adapter_for, core_dir, CoreAdapter, CoreLocator, SingBoxAdapter, XrayAdapter};
pub use graph::ExecutionPlan;
pub use identity::{
    current_identity, matches_identity, process_creation_time_ms, terminate_identity,
    ProcessIdentity,
};
pub use job::JobGuard;
pub use sha256::{sha256, sha256_hex};
pub use wire::{
    decode_payload, encode_frame, frame_len, frame_len_ok, RuntimeDetail, ServerFrame,
    LEN_PREFIX_BYTES, NET_HOST_PIPE_NAME, RUNTIME_DETAIL_EVENT,
};
