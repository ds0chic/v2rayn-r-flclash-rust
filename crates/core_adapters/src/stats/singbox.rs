//! sing-box WebSocket traffic adapter (`StatisticsSingboxService`).
//!
//! sing-box's `experimental.clash_api` controller serves `ws://.../traffic`
//! with per-interval byte *deltas* (`{"up":N,"down":M}`). The stream is
//! continuous, so unlike the Xray HTTP poll it needs an owned background task
//! with cancellation and reconnect backoff. Deltas are accumulated into
//! cumulative counters so the shape matches [`super::CounterSample`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::future::BoxFuture;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;

use super::{CounterSample, StatsError, StatsSource};
use crate::stats::PROXY_TAG;

/// Connection settings for the traffic stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficConfig {
    pub url: String,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl TrafficConfig {
    /// `ws://127.0.0.1:{state_port2}/traffic` with upstream-like backoff
    /// (`StatisticsSingboxService` sleeps 3s between attempts).
    pub fn new(port: u16) -> Self {
        Self {
            url: format!("ws://127.0.0.1:{port}/traffic"),
            initial_backoff: Duration::from_secs(3),
            max_backoff: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TrafficItem {
    #[serde(default, alias = "Up")]
    up: Option<u64>,
    #[serde(default, alias = "Down")]
    down: Option<u64>,
}

/// Parse one `/traffic` message. Returns `None` for non-traffic JSON frames so
/// stray control messages do not pollute the counters.
pub fn parse_traffic(text: &str) -> Option<(u64, u64)> {
    let item: TrafficItem = serde_json::from_str(text).ok()?;
    match (item.up, item.down) {
        (None, None) => None,
        (up, down) => Some((up.unwrap_or(0), down.unwrap_or(0))),
    }
}

#[derive(Clone)]
struct CancelHandle {
    tx: watch::Sender<bool>,
}

impl CancelHandle {
    fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self { tx }
    }

    fn cancel(&self) {
        // `send` fails when no receiver exists, which also means nobody is
        // listening; force the value regardless so a poll-based waiter (or a
        // later `subscribe`) still observes cancellation.
        self.tx.send_replace(true);
    }

    fn is_cancelled(&self) -> bool {
        *self.tx.borrow()
    }

    fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }
}

async fn wait_cancelled(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}

#[derive(Debug, Default)]
struct Collector {
    cumulative: HashMap<String, (u64, u64)>,
    closed: bool,
}

type Shared = Arc<Mutex<Collector>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpEnd {
    Ended,
    Cancelled,
}

/// Owned sing-box traffic stream. Dropping it cancels the background task.
pub struct SingboxTrafficSource {
    shared: Shared,
    cancel: CancelHandle,
    handle: tokio::task::JoinHandle<()>,
}

impl SingboxTrafficSource {
    /// Spawn the reconnect loop. Requires an ambient Tokio runtime.
    pub fn new(config: TrafficConfig) -> Result<Self, StatsError> {
        if !config.url.starts_with("ws://") && !config.url.starts_with("wss://") {
            return Err(StatsError::WebSocket(format!(
                "not a websocket url: {}",
                config.url
            )));
        }
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| StatsError::WebSocket("no tokio runtime available".into()))?;
        let shared: Shared = Arc::new(Mutex::new(Collector::default()));
        let cancel = CancelHandle::new();
        let task = handle.spawn(run(config, Arc::clone(&shared), cancel.clone()));
        Ok(Self {
            shared,
            cancel,
            handle: task,
        })
    }

    /// The call is upstream `ProxyTag` based: sing-box traffic is global, so it
    /// lands in the same `proxy` bucket the Xray adapter fills.
    pub fn snapshot(&self) -> Vec<CounterSample> {
        let guard = self.shared.lock().expect("collector poisoned");
        let mut samples: Vec<CounterSample> = guard
            .cumulative
            .iter()
            .map(|(tag, (up, down))| CounterSample {
                tag: tag.clone(),
                up: *up,
                down: *down,
            })
            .collect();
        samples.sort_by(|a, b| a.tag.cmp(&b.tag));
        samples
    }

    /// True once the background loop has stopped (cancel or fatal).
    pub fn is_closed(&self) -> bool {
        self.shared.lock().expect("collector poisoned").closed
    }

    /// Signal cancellation. Idempotent.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}

impl StatsSource for SingboxTrafficSource {
    fn poll(&mut self) -> BoxFuture<'_, Result<Vec<CounterSample>, StatsError>> {
        Box::pin(async move { Ok(self.snapshot()) })
    }

    fn generation(&self) -> u64 {
        // Deltas are accumulated here, so a core restart does not rebase the
        // counters and no generation bump is required.
        0
    }
}

impl Drop for SingboxTrafficSource {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.handle.abort();
    }
}

async fn run(config: TrafficConfig, shared: Shared, cancel: CancelHandle) {
    let mut rx = cancel.subscribe();
    let mut backoff = config.initial_backoff;
    loop {
        if cancel.is_cancelled() {
            break;
        }
        // The connect attempt itself is raced against cancellation: a core
        // that is down makes the TCP connect block until the OS gives up.
        let connect = tokio_tungstenite::connect_async(&config.url);
        tokio::pin!(connect);
        let connected = tokio::select! {
            result = &mut connect => Some(result),
            _ = wait_cancelled(&mut rx) => None,
        };
        match connected {
            None => break,
            Some(Ok((stream, _response))) => {
                backoff = config.initial_backoff;
                if pump(stream, Arc::clone(&shared), &cancel).await == PumpEnd::Cancelled {
                    break;
                }
            }
            Some(Err(_)) => {
                // Core not up yet or dropped; fall through to backoff.
            }
        }
        if cancel.is_cancelled() {
            break;
        }
        let sleep = tokio::time::sleep(backoff);
        tokio::pin!(sleep);
        tokio::select! {
            _ = sleep => {}
            _ = wait_cancelled(&mut rx) => break,
        }
        backoff = backoff.saturating_mul(2).min(config.max_backoff);
    }
    shared.lock().expect("collector poisoned").closed = true;
}

async fn pump<S>(mut stream: S, shared: Shared, cancel: &CancelHandle) -> PumpEnd
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    let mut rx = cancel.subscribe();
    loop {
        tokio::select! {
            message = stream.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        if let Some((up, down)) = parse_traffic(text.as_str()) {
                            add(&shared, up, down);
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            if let Some((up, down)) = parse_traffic(text) {
                                add(&shared, up, down);
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => return PumpEnd::Ended,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => return PumpEnd::Ended,
                }
            }
            _ = wait_cancelled(&mut rx) => return PumpEnd::Cancelled,
        }
    }
}

fn add(shared: &Shared, up: u64, down: u64) {
    let mut guard = shared.lock().expect("collector poisoned");
    let entry = guard
        .cumulative
        .entry(PROXY_TAG.to_string())
        .or_insert((0, 0));
    entry.0 = entry.0.saturating_add(up);
    entry.1 = entry.1.saturating_add(down);
}
