//! In-memory store of recent battery-SoC samples for the dashboard's
//! SoC chart (PR-soc-chart).
//!
//! A bounded ring of `(epoch_ms, soc)` pairs sampled every
//! `SAMPLE_INTERVAL` for `MAX_SAMPLES` slots — i.e. 48 h at 15 min
//! per sample.
//!
//! PR-soc-history-persist: the ring is also published as JSON to a
//! single retained MQTT topic so it survives restarts. After every
//! `record()` we serialize the current ring into a wire payload and
//! best-effort-send it through an mpsc channel to a publisher task
//! (see `main.rs`). On boot, the MQTT subscriber's bootstrap phase
//! reads the retained payload and calls `restore_from_wire` before
//! the periodic sampler starts.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Sample cadence — every 15 minutes.
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// 48 hours × 4 samples/hour.
pub const MAX_SAMPLES: usize = 192;

/// Wire-format schema version. Bump on incompatible changes; the
/// decoder rejects unknown versions and seeds an empty buffer.
const SCHEMA_V: u32 = 1;

/// Reject samples older than 48 h (the ring's natural window).
const RESTORE_MAX_AGE_MS: i64 = 48 * 3600 * 1000;
/// Reject samples in the future by more than 5 min (clock skew).
const RESTORE_MAX_SKEW_MS: i64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SocHistorySample {
    pub epoch_ms: i64,
    pub soc: f64,
}

/// Compact retained-MQTT wire format. Tuples instead of structs to
/// minimise payload size — ~3.2 KB at full 192 samples.
#[derive(Debug, Serialize, Deserialize)]
struct Wire {
    v: u32,
    samples: Vec<(i64, f64)>,
}

#[derive(Debug)]
pub struct SocHistoryStore {
    samples: Mutex<VecDeque<SocHistorySample>>,
    /// Optional sink for serialized retained-MQTT payloads. `None`
    /// until `set_publisher` is called from `main.rs` — keeps tests
    /// (and the no-MQTT branch) from needing a channel.
    publish_tx: Mutex<Option<mpsc::Sender<String>>>,
}

impl SocHistoryStore {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            samples: Mutex::new(VecDeque::with_capacity(MAX_SAMPLES)),
            publish_tx: Mutex::new(None),
        })
    }

    /// Install the publisher channel. Called once from `main.rs`
    /// after MQTT connects; absent the call, `record()` is silent on
    /// the publish side.
    pub fn set_publisher(&self, tx: mpsc::Sender<String>) {
        *self
            .publish_tx
            .lock()
            .expect("soc_history publish_tx mutex poisoned") = Some(tx);
    }

    /// Push a sample; evicts oldest entries to keep `len() <= MAX_SAMPLES`.
    /// After every push, best-effort republishes the entire ring as a
    /// wire payload to the configured publisher channel (if any).
    pub fn record(&self, soc: f64, epoch_ms: i64) {
        {
            let mut q = self.samples.lock().expect("soc_history mutex poisoned");
            q.push_back(SocHistorySample { epoch_ms, soc });
            while q.len() > MAX_SAMPLES {
                q.pop_front();
            }
        }
        // Serialise + best-effort publish OUTSIDE the samples lock so
        // a stuck publisher (channel full) cannot back-pressure the
        // sampler that called us.
        let payload = self.to_wire();
        let guard = self
            .publish_tx
            .lock()
            .expect("soc_history publish_tx mutex poisoned");
        if let Some(tx) = guard.as_ref() {
            // try_send: drop on Full. The next 15-min record will
            // republish the full ring anyway.
            let _ = tx.try_send(payload);
        }
    }

    /// Sync clone of the entire history in insertion order. Used by
    /// `world_to_snapshot` (sync). Holds the lock only long enough to
    /// copy ≤192 small structs.
    #[must_use]
    pub fn snapshot_blocking(&self) -> Vec<SocHistorySample> {
        let q = self.samples.lock().expect("soc_history mutex poisoned");
        q.iter().copied().collect()
    }

    /// Serialise the current ring into the retained-MQTT wire format.
    #[must_use]
    pub fn to_wire(&self) -> String {
        let samples = self.snapshot_blocking();
        let wire = Wire {
            v: SCHEMA_V,
            samples: samples.into_iter().map(|s| (s.epoch_ms, s.soc)).collect(),
        };
        serde_json::to_string(&wire).expect("trivial JSON serialise")
    }

    /// Parse a wire payload and seed the buffer. Drops samples that:
    ///  - have non-finite or out-of-range `soc_pct`
    ///  - have an `epoch_ms` more than 48 h before `now_ms` (too old)
    ///  - have an `epoch_ms` more than 5 min after `now_ms` (clock skew)
    ///
    /// Returns the number of samples accepted, or `None` on parse
    /// error / unknown schema version. Existing buffer contents are
    /// only cleared on a successful parse with a known schema; on
    /// `None` they are left untouched.
    pub fn restore_from_wire(&self, payload: &str, now_ms: i64) -> Option<usize> {
        let wire: Wire = match serde_json::from_str(payload) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "soc_history retained payload failed to parse; discarding",
                );
                return None;
            }
        };
        if wire.v != SCHEMA_V {
            tracing::warn!(
                wire_v = wire.v,
                expected_v = SCHEMA_V,
                "soc_history retained payload has unknown schema version; discarding",
            );
            return None;
        }
        let cutoff_min = now_ms - RESTORE_MAX_AGE_MS;
        let cutoff_max = now_ms + RESTORE_MAX_SKEW_MS;
        let mut accepted = 0usize;
        let mut samples = self.samples.lock().expect("soc_history mutex poisoned");
        samples.clear();
        for (epoch_ms, soc) in wire.samples {
            if !soc.is_finite() || !(0.0..=100.0).contains(&soc) {
                continue;
            }
            if epoch_ms < cutoff_min || epoch_ms > cutoff_max {
                continue;
            }
            if samples.len() >= MAX_SAMPLES {
                samples.pop_front();
            }
            samples.push_back(SocHistorySample { epoch_ms, soc });
            accepted += 1;
        }
        Some(accepted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drops_oldest_when_full() {
        let store = SocHistoryStore::new();
        // Push 200 samples; the first 8 should be evicted, leaving 192
        // entries whose first epoch_ms is the 9th-pushed (i.e. epoch_ms=9
        // when we use the 1-indexed sequence).
        for i in 1..=200_i64 {
            store.record(50.0 + (i as f64) * 0.1, i);
        }
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), MAX_SAMPLES);
        assert_eq!(snap.first().expect("non-empty").epoch_ms, 9);
        assert_eq!(snap.last().expect("non-empty").epoch_ms, 200);
    }

    #[test]
    fn snapshot_returns_clone_in_order() {
        let store = SocHistoryStore::new();
        store.record(10.0, 100);
        store.record(20.0, 200);
        store.record(30.0, 300);
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].epoch_ms, 100);
        assert_eq!(snap[0].soc, 10.0);
        assert_eq!(snap[1].epoch_ms, 200);
        assert_eq!(snap[2].epoch_ms, 300);
        // Mutating after snapshot must not affect the returned vec.
        store.record(40.0, 400);
        assert_eq!(snap.len(), 3);
    }

    /// Anchor `now_ms` at a realistic recent wall-clock so the 48 h /
    /// 5 min cutoffs are exercised meaningfully.
    const NOW_MS: i64 = 1_730_000_000_000;

    #[test]
    fn wire_roundtrip_preserves_samples() {
        let store = SocHistoryStore::new();
        // Pack 192 samples into the 48 h window. Step the timestamps
        // by SAMPLE_INTERVAL but keep the OLDEST sample strictly inside
        // the cutoff: base = now - (MAX_SAMPLES - 1) * step.
        let step_ms = i64::try_from(SAMPLE_INTERVAL.as_millis()).expect("step fits");
        let base = NOW_MS - i64::try_from(MAX_SAMPLES - 1).expect("len fits") * step_ms;
        for i in 0..MAX_SAMPLES {
            let epoch = base + i64::try_from(i).expect("idx fits") * step_ms;
            let soc = 30.0 + (i as f64) * 0.1;
            store.record(soc, epoch);
        }
        let payload = store.to_wire();

        let restored = SocHistoryStore::new();
        let accepted = restored
            .restore_from_wire(&payload, NOW_MS + 1_000)
            .expect("parse ok");
        assert_eq!(accepted, MAX_SAMPLES);
        let snap = restored.snapshot_blocking();
        assert_eq!(snap.len(), MAX_SAMPLES);
        // Order preserved.
        for (i, sample) in snap.iter().enumerate() {
            let epoch = base + i64::try_from(i).expect("idx fits") * step_ms;
            assert_eq!(sample.epoch_ms, epoch);
            // Use bit-exact compare since serde_json round-trips f64
            // through its decimal representation; small offsets like
            // 0.1 increments are stable here.
            assert!((sample.soc - (30.0 + (i as f64) * 0.1)).abs() < 1e-9);
        }
    }

    #[test]
    fn restore_drops_too_old_samples() {
        let store = SocHistoryStore::new();
        // 50 h in the past — older than the 48 h cutoff.
        let too_old = NOW_MS - 50 * 3600 * 1000;
        let payload = format!(
            r#"{{"v":1,"samples":[[{too_old},42.0]]}}"#
        );
        let accepted = store
            .restore_from_wire(&payload, NOW_MS)
            .expect("parse ok");
        assert_eq!(accepted, 0);
        assert!(store.snapshot_blocking().is_empty());
    }

    #[test]
    fn restore_drops_future_samples() {
        let store = SocHistoryStore::new();
        // 10 min in the future — exceeds the 5 min skew cutoff.
        let too_new = NOW_MS + 10 * 60 * 1000;
        let payload = format!(
            r#"{{"v":1,"samples":[[{too_new},42.0]]}}"#
        );
        let accepted = store
            .restore_from_wire(&payload, NOW_MS)
            .expect("parse ok");
        assert_eq!(accepted, 0);
        assert!(store.snapshot_blocking().is_empty());
    }

    #[test]
    fn restore_clamps_oversized_payload() {
        use std::fmt::Write;
        // Synth 300 samples in the payload, all packed inside the 48 h
        // window via 1-min spacing (300 min = 5 h). Assert all 300 are
        // accepted-and-pushed but the ring cap retains only the LAST
        // 192.
        let count: i64 = 300;
        let step_ms_dense: i64 = 60 * 1000;
        let dense_base = NOW_MS - count * step_ms_dense;
        let mut tuples = String::new();
        for i in 0..count {
            if i > 0 {
                tuples.push(',');
            }
            let epoch = dense_base + i * step_ms_dense;
            let _ = write!(tuples, "[{epoch},50.0]");
        }
        let payload = format!(r#"{{"v":1,"samples":[{tuples}]}}"#);
        let store = SocHistoryStore::new();
        let accepted = store
            .restore_from_wire(&payload, NOW_MS)
            .expect("parse ok");
        // All 300 are valid and pass the filters; restore_from_wire's
        // contract is "ring-cap-aware" so it pushes all 300 with
        // pop_front evictions, yielding 192 final entries and a 300
        // accepted count.
        assert_eq!(accepted, count as usize);
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), MAX_SAMPLES);
        // The 192 surviving entries are the LAST 192 of the input.
        let first_kept_idx = count - i64::try_from(MAX_SAMPLES).expect("len fits");
        assert_eq!(
            snap.first().expect("non-empty").epoch_ms,
            dense_base + first_kept_idx * step_ms_dense
        );
        assert_eq!(
            snap.last().expect("non-empty").epoch_ms,
            dense_base + (count - 1) * step_ms_dense
        );
    }

    #[test]
    fn restore_rejects_unknown_schema_version() {
        let store = SocHistoryStore::new();
        // Pre-seed a sample so we can confirm the buffer is untouched
        // on rejection.
        store.record(75.0, NOW_MS - 1_000);
        let payload = r#"{"v":99,"samples":[[1730000000000,42.0]]}"#;
        let result = store.restore_from_wire(payload, NOW_MS);
        assert!(result.is_none());
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].soc, 75.0);
    }

    #[test]
    fn restore_rejects_malformed_json() {
        let store = SocHistoryStore::new();
        store.record(75.0, NOW_MS - 1_000);
        let result = store.restore_from_wire("not json", NOW_MS);
        assert!(result.is_none());
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].soc, 75.0);
    }

    #[test]
    fn restore_drops_invalid_soc() {
        let store = SocHistoryStore::new();
        let t1 = NOW_MS - 3_000;
        let t2 = NOW_MS - 2_000;
        let t3 = NOW_MS - 1_000;
        // -1.0 (below 0%) and 101.0 (above 100%) are rejected; 50.0
        // survives. (NaN can't be expressed in standard JSON, so the
        // numeric range check is the meaningful guard for parsed input.)
        let payload = format!(
            r#"{{"v":1,"samples":[[{t1},-1.0],[{t2},101.0],[{t3},50.0]]}}"#
        );
        let accepted = store
            .restore_from_wire(&payload, NOW_MS)
            .expect("parse ok");
        // Only the 50.0 sample survives.
        assert_eq!(accepted, 1);
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].soc, 50.0);
        assert_eq!(snap[0].epoch_ms, t3);
    }

    #[test]
    fn record_after_restore_appends() {
        use std::fmt::Write;
        let store = SocHistoryStore::new();
        let mut tuples = String::new();
        let base = NOW_MS - 10 * 60 * 1000;
        for i in 0..5_i64 {
            if i > 0 {
                tuples.push(',');
            }
            let epoch = base + i * 60 * 1000;
            let soc = 50.0 + (i as f64);
            let _ = write!(tuples, "[{epoch},{soc}]");
        }
        let payload = format!(r#"{{"v":1,"samples":[{tuples}]}}"#);
        let accepted = store
            .restore_from_wire(&payload, NOW_MS)
            .expect("parse ok");
        assert_eq!(accepted, 5);
        // Now record one more sample.
        store.record(99.0, NOW_MS);
        let snap = store.snapshot_blocking();
        assert_eq!(snap.len(), 6);
        assert_eq!(snap[0].soc, 50.0);
        assert_eq!(snap[5].soc, 99.0);
        assert_eq!(snap[5].epoch_ms, NOW_MS);
    }
}
