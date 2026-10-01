//! net-host: the sole owner of managed core processes and runtime state.
//!
//! Started on demand by the AppEngine RuntimeClient (or manually for tests).
//! It is deliberately *not* placed in a Job that dies with the GUI: it holds
//! the Job that owns the core, and reclaims that lease when the client is lost.

#![allow(clippy::result_large_err)]

mod dacl;
mod events;
mod helper_client;
mod journal;
mod server;
mod session;
mod tun_lease;

use std::sync::Arc;

use session::{HostConfig, HostState};

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = HostConfig::from_env();
    eprintln!(
        "[net_host] starting version={} pipe={} run_root={}",
        env!("CARGO_PKG_VERSION"),
        config.pipe_name,
        config.run_root.display()
    );
    let state = Arc::new(HostState::new(config));
    let watchdog_state = state.clone();
    tokio::spawn(async move {
        server::watchdog(watchdog_state).await;
    });
    if let Err(e) = server::run_server(state).await {
        eprintln!("[net_host] fatal: {e}");
        std::process::exit(1);
    }
    eprintln!("[net_host] shutdown complete");
}
