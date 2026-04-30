//! Clock abstraction for the pure core.
//!
//! The core never reads the system clock directly; every function that
//! needs time takes `&dyn Clock`. The shell implements a real-clock; tests
//! use [`FixedClock`].

use std::time::Instant;

use chrono::NaiveDateTime;

/// Time source, abstracted for testability.
///
/// `monotonic` returns a monotonically-nondecreasing `Instant`, used for
/// TASS freshness and phase timestamps.
///
/// `naive` returns a *naive* (no-timezone) wall-clock datetime. Controllers
/// use this for tariff-band checks and schedule math, matching the legacy
/// Node-RED flow's use of `new Date()` (which in Node.js is system-local
/// time). The shell's real clock converts system-local → naive before
/// handing to the core; tests supply deterministic naive values.
///
/// `wall_clock_epoch_ms` returns the current wall-clock time as milliseconds
/// since the Unix epoch (UTC). Used for per-sample timestamps in ring
/// buffers (e.g. `ZappiDrainState`). Matches the SoC-chart history
/// convention in the shell (`soc_history`).
pub trait Clock {
    fn monotonic(&self) -> Instant;
    fn naive(&self) -> NaiveDateTime;
    /// Wall-clock milliseconds since the Unix epoch (UTC). Used for
    /// per-sample timestamps in observability ring buffers. Saturates
    /// per chrono's i64 timestamp range; well outside operational lifetime.
    fn wall_clock_epoch_ms(&self) -> i64;
}

/// Fixed clock for deterministic tests.
///
/// Copy both fields at construction; re-construct to advance time.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    pub monotonic: Instant,
    pub naive: NaiveDateTime,
}

impl FixedClock {
    pub const fn new(monotonic: Instant, naive: NaiveDateTime) -> Self {
        Self { monotonic, naive }
    }

    /// Returns a clock with monotonic time 0-seconds after `Instant::now()`
    /// (captured at call time, not guaranteed stable across calls).
    #[must_use]
    pub fn at(naive: NaiveDateTime) -> Self {
        Self {
            monotonic: Instant::now(),
            naive,
        }
    }
}

impl Clock for FixedClock {
    fn monotonic(&self) -> Instant {
        self.monotonic
    }
    fn naive(&self) -> NaiveDateTime {
        self.naive
    }
    fn wall_clock_epoch_ms(&self) -> i64 {
        // Treat `naive` as UTC for deterministic test timestamps.
        use chrono::TimeZone;
        chrono::Utc
            .from_utc_datetime(&self.naive)
            .timestamp_millis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Timelike, Utc};

    #[test]
    fn fixed_clock_returns_exactly_what_was_set() {
        let nt = NaiveDate::from_ymd_opt(2026, 1, 15)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap();
        let c = FixedClock::at(nt);
        assert_eq!(c.naive(), nt);
        assert_eq!(c.naive().hour(), 14);
        assert_eq!(c.naive().minute(), 30);
    }

    #[test]
    fn fixed_clock_wall_clock_epoch_ms_matches_utc_naive() {
        let naive = NaiveDate::from_ymd_opt(2026, 4, 30)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let clock = FixedClock::at(naive);
        let expected = Utc
            .with_ymd_and_hms(2026, 4, 30, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(clock.wall_clock_epoch_ms(), expected);
    }
}
