mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::{pick_spare_port, MockHttp, MockResponse};
use core_adapters::error::HttpError;
use core_adapters::stats::{
    classify_tag, parse_xray_vars, to_display_units, StatsError, StatsSource, TagClass,
    XrayStatsSource, DIRECT_TAG, PROXY_TAG,
};

/// Shape captured from a real Xray v26.3.27 `metrics.listen` endpoint.
const VARS: &str = r#"{"cmdline":["xray"],"memstats":{"Alloc":1},"stats":{"inbound":{"socks":{"downlink":13416,"uplink":309}},"outbound":{"direct":{"downlink":0,"uplink":0},"proxy":{"downlink":13380,"uplink":267}},"user":{}}}"#;

#[test]
fn parses_outbound_counters() {
    let samples = parse_xray_vars(VARS).expect("parse");
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].tag, DIRECT_TAG);
    assert_eq!(samples[1].tag, PROXY_TAG);
    assert_eq!(samples[1].up, 267);
    assert_eq!(samples[1].down, 13380);
}

#[test]
fn ignores_non_outbound_sections() {
    let samples = parse_xray_vars(VARS).unwrap();
    assert!(samples
        .iter()
        .all(|s| s.tag == DIRECT_TAG || s.tag == PROXY_TAG));
}

#[test]
fn missing_stats_is_empty_snapshot() {
    assert!(parse_xray_vars("{}").unwrap().is_empty());
    assert!(parse_xray_vars(r#"{"stats":{"inbound":{}}}"#)
        .unwrap()
        .is_empty());
}

#[test]
fn malformed_json_is_decode_error() {
    match parse_xray_vars("{ not json") {
        Err(StatsError::Decode(_)) => {}
        other => panic!("expected decode error, got {other:?}"),
    }
}

#[test]
fn negative_counters_are_clamped() {
    let samples =
        parse_xray_vars(r#"{"stats":{"outbound":{"proxy":{"uplink":-5,"downlink":9}}}}"#).unwrap();
    assert_eq!(samples[0].up, 0);
    assert_eq!(samples[0].down, 9);
}

#[test]
fn tag_classification_matches_upstream() {
    assert_eq!(classify_tag("proxy"), TagClass::Proxy);
    assert_eq!(classify_tag("proxy-auto"), TagClass::Proxy);
    assert_eq!(classify_tag("direct"), TagClass::Direct);
    assert_eq!(classify_tag("inbound"), TagClass::Other);
    assert_eq!(classify_tag("direct-x"), TagClass::Other);
}

#[test]
fn display_units_divide_by_1024() {
    assert_eq!(to_display_units(13380), 13);
    assert_eq!(to_display_units(1024), 1);
}

#[tokio::test]
async fn polls_mock_debug_vars() {
    let mock = MockHttp::spawn(|request| {
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "/debug/vars");
        MockResponse::json(VARS)
    });
    let mut source = XrayStatsSource::from_url(
        format!("{}/debug/vars", mock.base_url()),
        Duration::from_secs(2),
    )
    .unwrap();
    let samples = source.poll().await.unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(source.generation(), 0);
}

#[tokio::test]
async fn unauthorized_is_classified() {
    let mock = MockHttp::spawn(|_| MockResponse::json("{}").status(401));
    let mut source = XrayStatsSource::from_url(
        format!("{}/debug/vars", mock.base_url()),
        Duration::from_secs(2),
    )
    .unwrap();
    match source.poll().await {
        Err(StatsError::Http(HttpError::Unauthorized)) => {}
        other => panic!("expected unauthorized, got {other:?}"),
    }
}

#[tokio::test]
async fn server_error_is_classified() {
    let mock = MockHttp::spawn(|_| MockResponse::json("boom").status(500));
    let mut source = XrayStatsSource::from_url(
        format!("{}/debug/vars", mock.base_url()),
        Duration::from_secs(2),
    )
    .unwrap();
    match source.poll().await {
        Err(StatsError::Http(HttpError::Status(500))) => {}
        other => panic!("expected status 500, got {other:?}"),
    }
}

#[tokio::test]
async fn connection_refused_is_classified() {
    let port = pick_spare_port();
    // See clash_api::connection_refused_is_classified: loopback refusals can
    // take ~2s on this host, so allow enough budget to reach the RST.
    let mut source = XrayStatsSource::from_url(
        format!("http://127.0.0.1:{port}/debug/vars"),
        Duration::from_secs(5),
    )
    .unwrap();
    match source.poll().await {
        Err(StatsError::Http(HttpError::Connect(_))) => {}
        other => panic!("expected connect error, got {other:?}"),
    }
}

#[tokio::test]
async fn request_timeout_is_classified() {
    let mock = MockHttp::spawn(|_| {
        std::thread::sleep(Duration::from_millis(500));
        MockResponse::json("{}")
    });
    let mut source = XrayStatsSource::from_url(
        format!("{}/debug/vars", mock.base_url()),
        Duration::from_millis(100),
    )
    .unwrap();
    match source.poll().await {
        Err(StatsError::Http(HttpError::Timeout)) => {}
        other => panic!("expected timeout, got {other:?}"),
    }
}

#[tokio::test]
async fn counter_rebase_bumps_generation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mock = MockHttp::spawn(move |_| {
        let n = calls.fetch_add(1, Ordering::SeqCst);
        let up = if n == 0 { 1000 } else { 10 };
        MockResponse::json(format!(
            r#"{{"stats":{{"outbound":{{"proxy":{{"uplink":{up},"downlink":0}}}}}}}}"#
        ))
    });
    let mut source = XrayStatsSource::from_url(
        format!("{}/debug/vars", mock.base_url()),
        Duration::from_secs(2),
    )
    .unwrap();
    let first = source.poll().await.unwrap();
    assert_eq!(first[0].up, 1000);
    assert_eq!(source.generation(), 0);
    let second = source.poll().await.unwrap();
    assert_eq!(second[0].up, 10);
    assert_eq!(source.generation(), 1, "decreasing counter must rebase");
}
