mod common;

use std::time::Duration;

use common::{accept_ws, text_message, ws_listener};
use core_adapters::stats::{
    parse_traffic, CounterSample, SingboxTrafficSource, StatsError, StatsSource, TrafficConfig,
};
use futures_util::SinkExt;

fn fast_config(port: u16) -> TrafficConfig {
    TrafficConfig {
        url: format!("ws://127.0.0.1:{port}/traffic"),
        initial_backoff: Duration::from_millis(50),
        max_backoff: Duration::from_millis(200),
    }
}

async fn wait_for<F>(source: &SingboxTrafficSource, predicate: F) -> Vec<CounterSample>
where
    F: Fn(&[CounterSample]) -> bool,
{
    for _ in 0..200 {
        let snapshot = source.snapshot();
        if predicate(&snapshot) {
            return snapshot;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("condition not met, last={:?}", source.snapshot());
}

#[test]
fn parses_delta_message() {
    assert_eq!(parse_traffic(r#"{"up":10,"down":20}"#), Some((10, 20)));
}

#[test]
fn accepts_upstream_capitalised_fields() {
    assert_eq!(parse_traffic(r#"{"Up":1,"Down":2}"#), Some((1, 2)));
}

#[test]
fn ignores_non_traffic_json() {
    assert_eq!(parse_traffic("{}"), None);
    assert_eq!(parse_traffic("not json"), None);
    assert_eq!(parse_traffic(r#"{"type":"foo"}"#), None);
}

#[tokio::test]
async fn accumulates_streaming_deltas() {
    let (port, listener) = ws_listener().await;
    tokio::spawn(async move {
        let mut ws = accept_ws(&listener).await;
        ws.send(text_message(r#"{"up":10,"down":20}"#))
            .await
            .unwrap();
        ws.send(text_message(r#"{"up":5,"down":5}"#)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });
    let mut source = SingboxTrafficSource::new(fast_config(port)).unwrap();
    let snapshot = wait_for(&source, |samples| {
        samples.first().is_some_and(|s| s.up == 15 && s.down == 25)
    })
    .await;
    assert_eq!(snapshot[0].tag, "proxy");
    // Cumulative counters are exposed through the StatsSource trait too.
    let polled = source.poll().await.unwrap();
    assert_eq!(polled[0].up, 15);
    assert_eq!(source.generation(), 0);
}

#[tokio::test]
async fn tolerates_malformed_frames() {
    let (port, listener) = ws_listener().await;
    tokio::spawn(async move {
        let mut ws = accept_ws(&listener).await;
        ws.send(text_message("garbage")).await.unwrap();
        ws.send(text_message(r#"{"up":42,"down":0}"#))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
    });
    let source = SingboxTrafficSource::new(fast_config(port)).unwrap();
    let snapshot = wait_for(&source, |samples| {
        samples.first().is_some_and(|s| s.up == 42)
    })
    .await;
    assert_eq!(snapshot[0].up, 42);
}

#[tokio::test]
async fn reconnects_with_backoff_and_keeps_accumulating() {
    let (port, listener) = ws_listener().await;
    tokio::spawn(async move {
        let mut first = accept_ws(&listener).await;
        first
            .send(text_message(r#"{"up":3,"down":0}"#))
            .await
            .unwrap();
        drop(first);
        let mut second = accept_ws(&listener).await;
        second
            .send(text_message(r#"{"up":7,"down":0}"#))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
    });
    let source = SingboxTrafficSource::new(fast_config(port)).unwrap();
    let snapshot = wait_for(&source, |samples| {
        samples.first().is_some_and(|s| s.up == 10)
    })
    .await;
    assert_eq!(snapshot[0].up, 10);
}

#[tokio::test]
async fn cancel_stops_a_live_stream() {
    let (port, listener) = ws_listener().await;
    tokio::spawn(async move {
        let mut ws = accept_ws(&listener).await;
        loop {
            if ws.send(text_message(r#"{"up":1,"down":1}"#)).await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    let source = SingboxTrafficSource::new(fast_config(port)).unwrap();
    wait_for(&source, |samples| {
        samples.first().is_some_and(|s| s.up >= 1)
    })
    .await;
    source.cancel();
    for _ in 0..200 {
        if source.is_closed() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("live source did not close after cancel");
}

#[tokio::test]
async fn cancel_stops_a_pending_connect() {
    // No mock is listening on this port; the connect blocks until the OS
    // gives up, which must not delay cancellation.
    let source = SingboxTrafficSource::new(fast_config(common::pick_spare_port())).unwrap();
    source.cancel();
    for _ in 0..200 {
        if source.is_closed() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("source did not close while connecting");
}

#[tokio::test]
async fn drop_stops_the_source() {
    let port = common::pick_port();
    let source = SingboxTrafficSource::new(fast_config(port)).unwrap();
    drop(source);
    // Nothing to assert beyond a clean teardown; giving the runtime a tick
    // ensures the aborted task does not panic.
    tokio::time::sleep(Duration::from_millis(20)).await;
}

#[test]
fn rejects_non_websocket_url() {
    let config = TrafficConfig {
        url: "http://127.0.0.1:1/traffic".to_string(),
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(10),
    };
    match SingboxTrafficSource::new(config) {
        Err(StatsError::WebSocket(_)) => {}
        Ok(_) => panic!("expected websocket error, got Ok"),
        Err(other) => panic!("expected websocket error, got {other:?}"),
    }
}

#[test]
fn default_config_targets_state_port2() {
    let config = TrafficConfig::new(12081);
    assert_eq!(config.url, "ws://127.0.0.1:12081/traffic");
    assert_eq!(config.initial_backoff, Duration::from_secs(3));
}
