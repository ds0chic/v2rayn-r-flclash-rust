//! Core adapters: statistics, log stream, Clash API client (T15).
//!
//! This crate is intentionally isolated from the rest of the workspace: it only
//! speaks to the two kernels over loopback, never owns their processes, and
//! never touches the user's live proxy (port 10808). All clients build with
//! `no_proxy` so host proxy environment variables cannot redirect a control
//! request.
//!
//! Modules:
//! * [`stats`] - Xray `/debug/vars` HTTP poll and sing-box `/traffic` WebSocket
//!   stream, normalised to cumulative [`stats::CounterSample`]s; aggregator with
//!   1 Hz throttling and generation-based reset handling.
//! * [`clash_api`] - read/observe Clash controller client (`ClashApiManager`).
//! * [`log_stream`] - line reader, bounded ring buffer and level/keyword filter.

pub mod clash_api;
pub mod error;
pub mod log_stream;
pub mod stats;
