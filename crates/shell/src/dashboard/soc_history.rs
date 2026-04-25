//! In-memory store of recent battery-SoC samples for the dashboard's
//! SoC chart (PR-soc-chart).
//!
//! A bounded ring of `(epoch_ms, soc)` pairs sampled every
//! `SAMPLE_INTERVAL` for `MAX_SAMPLES` slots — i.e. 48 h at 15 min
//! per sample. Lost on process restart by design (no local persistence
//! per the deploy constraints).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Sample cadence — every 15 minutes.
pub const SAMPLE_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// 48 hours × 4 samples/hour.
pub const MAX_SAMPLES: usize = 192;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SocHistorySample {
    pub epoch_ms: i64,
    pub soc: f64,
}

#[derive(Debug)]
pub struct SocHistoryStore {
    samples: Mutex<VecDeque<SocHistorySample>>,
}

impl SocHistoryStore {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            samples: Mutex::new(VecDeque::with_capacity(MAX_SAMPLES)),
        })
    }

    /// Push a sample; evicts oldest entries to keep `len() <= MAX_SAMPLES`.
    pub fn record(&self, soc: f64, epoch_ms: i64) {
        let mut q = self.samples.lock().expect("soc_history mutex poisoned");
        q.push_back(SocHistorySample { epoch_ms, soc });
        while q.len() > MAX_SAMPLES {
            q.pop_front();
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
}
