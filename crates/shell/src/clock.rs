//! Real-clock implementation of [`victron_controller_core::Clock`].

use std::time::Instant;

use chrono::{Local, NaiveDateTime};

use victron_controller_core::Clock;

#[derive(Debug, Clone, Copy, Default)]
pub struct RealClock;

impl Clock for RealClock {
    fn monotonic(&self) -> Instant {
        Instant::now()
    }

    fn naive(&self) -> NaiveDateTime {
        // Victron NR code used system-local time (JS `new Date()`).
        // Mirror that: strip the TZ and hand the naive datetime to the
        // core, which doesn't know anything about zones.
        Local::now().naive_local()
    }
}
