use std::time::{Duration, Instant};

use super::Freshness;

/// Observation of a sensor entity (no target).
///
/// Used directly for read-only entities like `BatterySoc`, `GridPower`, etc.,
/// and as a field of [`super::Actuated`] for actuated entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actual<V> {
    pub value: Option<V>,
    pub freshness: Freshness,
    /// Monotonic instant at which the current `(value, freshness)` pair
    /// became current.
    pub since: Instant,
}

impl<V> Actual<V> {
    /// Fresh-from-boot state: no reading yet, freshness `Unknown`.
    pub const fn unknown(now: Instant) -> Self {
        Self {
            value: None,
            freshness: Freshness::Unknown,
            since: now,
        }
    }

    /// Record a reading. Freshness becomes `Fresh`; `since` resets.
    ///
    /// Valid from any current freshness including `Unknown` and `Deprecated`.
    pub fn on_reading(&mut self, value: V, now: Instant) {
        self.value = Some(value);
        self.freshness = Freshness::Fresh;
        self.since = now;
    }

    /// Mark the current reading as `Deprecated`. Call when the corresponding
    /// target changed — the stored value still describes the old target.
    ///
    /// No-op on `Unknown` (nothing to deprecate) or `Deprecated` (already is).
    pub fn deprecate(&mut self, now: Instant) {
        if matches!(self.freshness, Freshness::Fresh | Freshness::Stale) {
            self.freshness = Freshness::Deprecated;
            self.since = now;
        }
    }

    /// Decay `Fresh → Stale` when the reading has aged past `threshold`.
    /// No-op on `Unknown` / `Stale` / `Deprecated`.
    pub fn tick(&mut self, now: Instant, threshold: Duration) {
        if matches!(self.freshness, Freshness::Fresh)
            && now.saturating_duration_since(self.since) > threshold
        {
            self.freshness = Freshness::Stale;
        }
    }

    /// True when `freshness == Fresh` and a value is present. Convenience
    /// for controllers.
    pub const fn is_usable(&self) -> bool {
        matches!(self.freshness, Freshness::Fresh) && self.value.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_starts_without_value() {
        let t0 = Instant::now();
        let a: Actual<i32> = Actual::unknown(t0);
        assert_eq!(a.value, None);
        assert_eq!(a.freshness, Freshness::Unknown);
        assert_eq!(a.since, t0);
        assert!(!a.is_usable());
    }

    #[test]
    fn reading_transitions_unknown_to_fresh() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_secs(1);
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(42, t1);
        assert_eq!(a.value, Some(42));
        assert_eq!(a.freshness, Freshness::Fresh);
        assert_eq!(a.since, t1);
        assert!(a.is_usable());
    }

    #[test]
    fn tick_within_threshold_keeps_fresh() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        a.tick(t0 + Duration::from_secs(4), Duration::from_secs(5));
        assert_eq!(a.freshness, Freshness::Fresh);
    }

    #[test]
    fn tick_past_threshold_decays_to_stale() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        a.tick(t0 + Duration::from_secs(6), Duration::from_secs(5));
        assert_eq!(a.freshness, Freshness::Stale);
    }

    #[test]
    fn tick_does_not_upgrade_stale_back_to_fresh() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        a.tick(t0 + Duration::from_secs(6), Duration::from_secs(5));
        assert_eq!(a.freshness, Freshness::Stale);

        // Further ticks leave it Stale — only a reading can refresh.
        a.tick(t0 + Duration::from_secs(20), Duration::from_secs(5));
        assert_eq!(a.freshness, Freshness::Stale);
    }

    #[test]
    fn new_reading_after_stale_returns_to_fresh() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        a.tick(t0 + Duration::from_secs(6), Duration::from_secs(5));
        assert_eq!(a.freshness, Freshness::Stale);

        a.on_reading(2, t0 + Duration::from_secs(7));
        assert_eq!(a.freshness, Freshness::Fresh);
        assert_eq!(a.value, Some(2));
    }

    #[test]
    fn tick_on_unknown_is_noop() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.tick(t0 + Duration::from_secs(100), Duration::from_secs(5));
        assert_eq!(a.freshness, Freshness::Unknown);
    }

    #[test]
    fn deprecate_transitions_fresh_to_deprecated() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        let t1 = t0 + Duration::from_secs(2);
        a.deprecate(t1);
        assert_eq!(a.freshness, Freshness::Deprecated);
        assert_eq!(a.since, t1);
        assert_eq!(a.value, Some(1), "value preserved even when deprecated");
    }

    #[test]
    fn deprecate_transitions_stale_to_deprecated() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        a.tick(t0 + Duration::from_secs(6), Duration::from_secs(5));
        a.deprecate(t0 + Duration::from_secs(7));
        assert_eq!(a.freshness, Freshness::Deprecated);
    }

    #[test]
    fn deprecate_on_unknown_is_noop() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.deprecate(t0 + Duration::from_secs(1));
        assert_eq!(a.freshness, Freshness::Unknown);
    }

    #[test]
    fn deprecate_on_deprecated_is_noop() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        let t1 = t0 + Duration::from_secs(1);
        a.deprecate(t1);
        a.deprecate(t1 + Duration::from_secs(1));
        assert_eq!(a.since, t1, "since should not advance on no-op deprecate");
    }

    #[test]
    fn reading_out_of_deprecated_returns_fresh() {
        let t0 = Instant::now();
        let mut a: Actual<i32> = Actual::unknown(t0);
        a.on_reading(1, t0);
        a.deprecate(t0 + Duration::from_secs(1));
        a.on_reading(2, t0 + Duration::from_secs(2));
        assert_eq!(a.freshness, Freshness::Fresh);
        assert_eq!(a.value, Some(2));
    }

    #[test]
    fn is_usable_requires_fresh_and_value() {
        let t0 = Instant::now();
        let a_unknown: Actual<i32> = Actual::unknown(t0);
        assert!(!a_unknown.is_usable());

        let mut a_fresh: Actual<i32> = Actual::unknown(t0);
        a_fresh.on_reading(1, t0);
        assert!(a_fresh.is_usable());

        let mut a_stale = a_fresh;
        a_stale.tick(t0 + Duration::from_secs(10), Duration::from_secs(1));
        assert!(!a_stale.is_usable());

        let mut a_dep = a_fresh;
        a_dep.deprecate(t0 + Duration::from_secs(1));
        assert!(!a_dep.is_usable());
    }
}
