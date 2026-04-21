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
pub trait Clock {
    fn monotonic(&self) -> Instant;
    fn naive(&self) -> NaiveDateTime;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Timelike};

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
}
