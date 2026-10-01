mod common;

use std::time::{Duration, Instant};

use core_adapters::stats::{CounterSample, StatsAggregator};

fn sample(tag: &str, up: u64, down: u64) -> CounterSample {
    CounterSample {
        tag: tag.to_string(),
        up,
        down,
    }
}

#[test]
fn first_snapshot_has_zero_rate() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    assert!(agg.apply(&[sample("proxy", 500, 900)], 0, t0));
    let stat = &agg.stats()["proxy"];
    assert_eq!(stat.up_total, 500);
    assert_eq!(stat.down_total, 900);
    assert_eq!(stat.up_bps, 0);
    assert_eq!(stat.down_bps, 0);
}

#[test]
fn second_snapshot_computes_rate() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    agg.apply(&[sample("proxy", 0, 0)], 0, t0);
    agg.apply(
        &[sample("proxy", 2048, 4096)],
        0,
        t0 + Duration::from_secs(1),
    );
    let stat = &agg.stats()["proxy"];
    assert_eq!(stat.up_bps, 2048);
    assert_eq!(stat.down_bps, 4096);
    assert_eq!(stat.up_total, 2048);
}

#[test]
fn throttles_within_interval() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    assert!(agg.apply(&[sample("proxy", 100, 100)], 0, t0));
    assert!(!agg.apply(
        &[sample("proxy", 200, 200)],
        0,
        t0 + Duration::from_millis(400)
    ));
    assert_eq!(agg.applied_count(), 1);
    assert_eq!(agg.throttled_count(), 1);
}

#[test]
fn throttle_does_not_lose_accumulated_bytes() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    agg.apply(&[sample("proxy", 0, 0)], 0, t0);
    // Intermediate tick is throttled away; its bytes must still land in the
    // next applied window.
    agg.apply(
        &[sample("proxy", 400, 400)],
        0,
        t0 + Duration::from_millis(400),
    );
    agg.apply(
        &[sample("proxy", 1000, 2000)],
        0,
        t0 + Duration::from_secs(1),
    );
    let stat = &agg.stats()["proxy"];
    assert_eq!(stat.up_total, 1000);
    assert_eq!(
        stat.up_bps, 1000,
        "delta measured from the last applied baseline"
    );
    assert_eq!(stat.down_bps, 2000);
}

#[test]
fn generation_change_resets_and_always_applies() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    agg.apply(&[sample("proxy", 9000, 9000)], 0, t0);
    let applied = agg.apply(
        &[sample("proxy", 10, 20)],
        1,
        t0 + Duration::from_millis(10),
    );
    assert!(applied, "generation change bypasses the throttle");
    assert_eq!(agg.generation(), 1);
    let stat = &agg.stats()["proxy"];
    assert_eq!(stat.up_total, 10);
    assert_eq!(stat.down_total, 20);
    assert_eq!(stat.up_bps, 0, "baseline was reset, no bogus delta");
}

#[test]
fn wide_u64_counters_do_not_overflow() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    let base = u64::MAX - 1_000_000;
    agg.apply(&[sample("proxy", base, 0)], 0, t0);
    agg.apply(
        &[sample("proxy", base + 1_000_000, 1_000_000)],
        0,
        t0 + Duration::from_secs(1),
    );
    let stat = &agg.stats()["proxy"];
    assert_eq!(stat.up_total, base + 1_000_000);
    assert_eq!(stat.up_bps, 1_000_000);
}

#[test]
fn tag_disappearance_keeps_last_stat() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    agg.apply(&[sample("proxy", 10, 10), sample("direct", 5, 5)], 0, t0);
    agg.apply(&[sample("proxy", 20, 20)], 0, t0 + Duration::from_secs(1));
    assert!(agg.stats().contains_key("direct"));
    assert_eq!(agg.stats()["direct"].up_total, 5);
}

#[test]
fn per_tag_rates_are_independent() {
    let mut agg = StatsAggregator::one_hz();
    let t0 = Instant::now();
    agg.apply(&[sample("proxy", 0, 0), sample("direct", 0, 0)], 0, t0);
    agg.apply(
        &[sample("proxy", 100, 0), sample("direct", 0, 50)],
        0,
        t0 + Duration::from_secs(1),
    );
    assert_eq!(agg.stats()["proxy"].up_bps, 100);
    assert_eq!(agg.stats()["proxy"].down_bps, 0);
    assert_eq!(agg.stats()["direct"].up_bps, 0);
    assert_eq!(agg.stats()["direct"].down_bps, 50);
}
