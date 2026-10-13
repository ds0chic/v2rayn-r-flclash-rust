//! T01 FRB functions: synthetic node generation, paging, ping and an event
//! round-trip used to verify the complete UI -> FRB -> Rust -> UI chain.

use std::sync::{Mutex, OnceLock};

use crate::frb_generated::StreamSink;
use domain::ProfileSummary;
use flutter_rust_bridge::frb;

/// Progress event streamed while generating synthetic rows.
pub struct ProgressEvent {
    pub seq: u32,
    pub done: u32,
    pub total: u32,
}

/// Acknowledgement for a UI event, echoing the sequence number back so the UI
/// can match its own counter with the Rust side.
pub struct UiEventAck {
    pub seq: u64,
    pub kind: String,
    pub rust_profile_count: u32,
}

static STORE: OnceLock<Mutex<Vec<ProfileSummary>>> = OnceLock::new();

/// Upper bound for synthetic row generation (S8-01). The bench drives 10k
/// rows by default; larger requests are truncated instead of exhausting memory.
pub const MAX_SYNTHETIC_ROWS: u32 = 200_000;
/// Upper bound for the synthetic blocking probe (S8-01).
pub const MAX_BLOCKING_MS: u64 = 30_000;

fn store() -> &'static Mutex<Vec<ProfileSummary>> {
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}

/// Generate and retain `count` synthetic rows, returning all of them.
#[frb(sync)]
pub fn generate_profiles(count: u32) -> Vec<ProfileSummary> {
    let rows = application::generate_synthetic_profiles(count.min(MAX_SYNTHETIC_ROWS));
    if let Ok(mut guard) = store().lock() {
        *guard = rows.clone();
    }
    rows
}

/// Fetch one deterministic page without replacing the retained data set.
#[frb(sync)]
pub fn fetch_profiles_page(offset: u32, limit: u32) -> Vec<ProfileSummary> {
    application::generate_profiles_page(offset, limit)
}

/// Number of rows currently retained on the Rust side, for UI cross-check.
#[frb(sync)]
pub fn rust_profile_count() -> u32 {
    store().lock().map(|g| g.len() as u32).unwrap_or(0)
}

/// Fetch a retained row by index.
#[frb(sync)]
pub fn profile_at(index: u32) -> Option<ProfileSummary> {
    store()
        .lock()
        .ok()
        .and_then(|g| g.get(index as usize).cloned())
}

/// Deterministic ping simulation for a retained row id. No network I/O.
#[frb(sync)]
pub fn ping_profile(id: String) -> i32 {
    store()
        .lock()
        .ok()
        .and_then(|g| g.iter().find(|p| p.id == id).map(application::ping))
        .unwrap_or(-1)
}

/// Stream progress events while "generating" `count` rows in pages. Used by
/// the UI to confirm events flow back from Rust.
pub fn progress_stream(count: u32, sink: StreamSink<ProgressEvent>) -> Result<(), String> {
    const PAGE: u32 = 1_000;
    let count = count.min(MAX_SYNTHETIC_ROWS);
    let mut done = 0u32;
    let mut seq = 0u32;
    while done < count {
        let step = PAGE.min(count - done);
        done += step;
        seq += 1;
        sink.add(ProgressEvent {
            seq,
            done,
            total: count,
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Echo a UI event sequence back to Dart, proving a full round-trip.
#[frb(sync)]
pub fn echo_ui_event(seq: u64, kind: String) -> UiEventAck {
    UiEventAck {
        seq,
        kind,
        rust_profile_count: rust_profile_count(),
    }
}

/// Run a synthetic blocking loop off the UI thread and return elapsed millis.
pub async fn simulate_blocking(ms: u64) -> u64 {
    application::blocking_probe(ms.min(MAX_BLOCKING_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S8-01: oversized requests are truncated instead of exhausting memory.
    /// Pre-fix this returns all 300k rows; post-fix it stops at the cap.
    #[test]
    fn synthetic_generation_is_bounded() {
        let rows = generate_profiles(300_000);
        assert_eq!(rows.len(), 200_000);
    }

    /// S8-01: small probes still pass through unclamped.
    #[test]
    fn small_generation_is_untouched() {
        assert_eq!(generate_profiles(7).len(), 7);
    }
}
