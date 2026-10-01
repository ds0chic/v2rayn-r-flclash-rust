//! Per-tag rate/cumulative aggregation with 1 Hz throttling.
//!
//! Upstream recomputes a delta on every timer tick (`PeriodicTimer(1s)` for
//! Xray, one message per interval for sing-box) and adds it to the persisted
//! totals. This module keeps the same observable output but makes the counters
//! and the throttle explicit:
//!
//! * cumulative counters are `u64` and never wrapped down,
//! * the rate is computed from the *last applied* snapshot, so dropping
//!   intermediate ticks to satisfy the 1 Hz budget does not lose bytes,
//! * a generation change (core restart) clears the baseline instead of
//!   emitting a bogus negative delta.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use super::CounterSample;

/// Aggregated view of one tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagStat {
    pub tag: String,
    /// Latest cumulative counter reported by the core.
    pub up_total: u64,
    pub down_total: u64,
    /// Bytes/second over the last applied window (`0` on the first snapshot).
    pub up_bps: u64,
    pub down_bps: u64,
}

/// 1 Hz aggregator (upstream `PeriodicTimer(TimeSpan.FromSeconds(1))`).
pub struct StatsAggregator {
    min_interval: Duration,
    generation: u64,
    last_apply: Option<Instant>,
    baselines: HashMap<String, (u64, u64)>,
    stats: BTreeMap<String, TagStat>,
    applied_count: u64,
    throttled_count: u64,
}

impl StatsAggregator {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            generation: 0,
            last_apply: None,
            baselines: HashMap::new(),
            stats: BTreeMap::new(),
            applied_count: 0,
            throttled_count: 0,
        }
    }

    /// The upstream default: one update per second.
    pub fn one_hz() -> Self {
        Self::new(Duration::from_secs(1))
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn applied_count(&self) -> u64 {
        self.applied_count
    }

    pub fn throttled_count(&self) -> u64 {
        self.throttled_count
    }

    pub fn stats(&self) -> &BTreeMap<String, TagStat> {
        &self.stats
    }

    /// Ingest a snapshot.
    ///
    /// Returns `true` when the snapshot was applied, `false` when it was
    /// throttled away. A generation change always applies (and resets).
    pub fn apply(&mut self, samples: &[CounterSample], generation: u64, now: Instant) -> bool {
        let generation_changed = generation != self.generation;
        if generation_changed {
            self.reset(generation);
        }

        let due = generation_changed
            || match self.last_apply {
                None => true,
                Some(previous) => now.saturating_duration_since(previous) >= self.min_interval,
            };
        if !due {
            self.throttled_count += 1;
            return false;
        }

        let elapsed = self
            .last_apply
            .map(|previous| now.saturating_duration_since(previous));
        for sample in samples {
            let baseline = self.baselines.get(&sample.tag).copied();
            // A decrease without a generation bump still must not wrap: treat
            // the new value as a fresh baseline.
            let (delta_up, delta_down) = match baseline {
                Some((prev_up, prev_down)) => (
                    sample.up.saturating_sub(prev_up),
                    sample.down.saturating_sub(prev_down),
                ),
                None => (0, 0),
            };
            let (up_bps, down_bps) = match elapsed {
                Some(window) if window.as_nanos() > 0 => {
                    let nanos = window.as_nanos() as u64;
                    (per_second(delta_up, nanos), per_second(delta_down, nanos))
                }
                _ => (0, 0),
            };
            self.baselines
                .insert(sample.tag.clone(), (sample.up, sample.down));
            self.stats.insert(
                sample.tag.clone(),
                TagStat {
                    tag: sample.tag.clone(),
                    up_total: sample.up,
                    down_total: sample.down,
                    up_bps,
                    down_bps,
                },
            );
        }

        self.last_apply = Some(now);
        self.applied_count += 1;
        true
    }

    fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.last_apply = None;
        self.baselines.clear();
        self.stats.clear();
    }
}

fn per_second(delta: u64, window_nanos: u64) -> u64 {
    // Integer-only to avoid precision loss on very large counters.
    ((delta as u128 * 1_000_000_000u128) / window_nanos as u128) as u64
}
