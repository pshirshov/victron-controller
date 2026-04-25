//! Real-clock implementation of [`victron_controller_core::Clock`].
//!
//! PR-tz-from-victron: `naive()` consults a shared [`TzHandle`] (stored
//! on `Topology`) and converts the current UTC instant into the
//! Victron-supplied display zone. Default is UTC until the first
//! `/Settings/System/TimeZone` reading lands. Lock-free reads via
//! `arc-swap`.

use std::time::Instant;

use chrono::NaiveDateTime;

use victron_controller_core::tz::TzHandle;
use victron_controller_core::Clock;

/// Real-clock implementation. Cheap to clone (`TzHandle` is an
/// `Arc<ArcSwap>`).
#[derive(Debug, Clone)]
pub struct RealClock {
    tz: TzHandle,
}

impl RealClock {
    /// New clock backed by the given Tz handle. The shell wires the
    /// same handle through `Topology::tz_handle` so D-Bus subscriber
    /// updates land here.
    #[must_use]
    pub fn new(tz: TzHandle) -> Self {
        Self { tz }
    }
}

impl Clock for RealClock {
    fn monotonic(&self) -> Instant {
        Instant::now()
    }

    fn naive(&self) -> NaiveDateTime {
        let tz = self.tz.current();
        chrono::Utc::now().with_timezone(&tz).naive_local()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    /// PR-tz-from-victron: a fresh `TzHandle` defaults to UTC, so
    /// `naive()` reports the current UTC hour (within a small clock-
    /// skew tolerance).
    #[test]
    fn default_is_utc() {
        let h = TzHandle::new_utc();
        let clock = RealClock::new(h);
        let naive = clock.naive();
        let utc_now = chrono::Utc::now().naive_utc();
        let delta = (naive - utc_now).num_seconds().abs();
        assert!(delta < 5, "naive() should be near UTC now (delta={delta}s)");
    }

    /// PR-tz-from-victron: setting the TzHandle to London updates the
    /// next `naive()` reading. We don't pin a specific hour (depends
    /// on summer/winter) — assert the wall-clock difference matches
    /// the live offset in `Europe/London` for the current instant.
    #[test]
    fn tz_handle_updates_clock() {
        let h = TzHandle::new_utc();
        let clock = RealClock::new(h.clone());
        h.set(chrono_tz::Europe::London);
        let naive_local = clock.naive();
        let utc_now = chrono::Utc::now();
        let london_now = utc_now.with_timezone(&chrono_tz::Europe::London).naive_local();
        let delta = (naive_local - london_now).num_seconds().abs();
        assert!(
            delta < 5,
            "naive() should track Europe/London (delta={delta}s)"
        );
    }

    /// PR-tz-from-victron: helper that pins the UTC instant and asserts
    /// the converted naive matches the expected `Europe/London` wall
    /// time. Drives both DST sides via parameter values.
    fn london_naive_for(utc_iso: &str) -> chrono::NaiveDateTime {
        use std::str::FromStr;
        let utc = chrono::DateTime::<chrono::Utc>::from_str(utc_iso).unwrap();
        let tz = chrono_tz::Europe::London;
        utc.with_timezone(&tz).naive_local()
    }

    #[test]
    fn dst_boundary_london_summer() {
        // 2026-07-01T12:00:00Z — Europe/London observes IST = UTC+1.
        let n = london_naive_for("2026-07-01T12:00:00Z");
        assert_eq!(n.time().hour(), 13, "summer offset = +1h");
        assert_eq!(n.date(), chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
    }

    #[test]
    fn dst_boundary_london_winter() {
        // 2026-01-01T12:00:00Z — Europe/London observes UTC offset = 0.
        let n = london_naive_for("2026-01-01T12:00:00Z");
        assert_eq!(n.time().hour(), 12, "winter offset = 0h");
        assert_eq!(n.date(), chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    }

}
