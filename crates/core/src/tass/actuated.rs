use std::time::{Duration, Instant};

use super::{Actual, TargetPhase};
use crate::Owner;

/// The target half of an actuated entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target<V> {
    pub value: Option<V>,
    pub owner: Owner,
    pub phase: TargetPhase,
    /// Monotonic instant at which the current `(value, owner, phase)` tuple
    /// became current.
    pub since: Instant,
}

impl<V> Target<V> {
    pub const fn unset(now: Instant) -> Self {
        Self {
            value: None,
            owner: Owner::Unset,
            phase: TargetPhase::Unset,
            since: now,
        }
    }
}

/// An actuated entity: both a target (what we want) and an actual (what we
/// last observed). See SPEC §5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actuated<V> {
    pub target: Target<V>,
    pub actual: Actual<V>,
}

impl<V> Actuated<V> {
    /// Fresh-from-boot state.
    pub const fn new(now: Instant) -> Self {
        Self {
            target: Target::unset(now),
            actual: Actual::unknown(now),
        }
    }

    /// Transition `Pending → Commanded`. Idempotent no-op outside `Pending`.
    pub fn mark_commanded(&mut self, now: Instant) {
        if matches!(self.target.phase, TargetPhase::Pending) {
            self.target.phase = TargetPhase::Commanded;
            self.target.since = now;
        }
    }

    /// Record an actual reading. Freshness becomes `Fresh`.
    ///
    /// Does **not** automatically confirm the target — call [`Self::confirm_if`]
    /// after `on_reading` to attempt confirmation.
    pub fn on_reading(&mut self, value: V, now: Instant) {
        self.actual.on_reading(value, now);
    }

    /// Attempt `Commanded → Confirmed` using a user-supplied predicate.
    ///
    /// `close(&target, &actual)` should return true when the actual reading
    /// is close enough to the target to consider the write confirmed. Use
    /// strict equality for discrete values, or a tolerance check for analog
    /// ones.
    ///
    /// Returns `true` if the phase transitioned.
    pub fn confirm_if<F: FnOnce(&V, &V) -> bool>(&mut self, close: F, now: Instant) -> bool {
        if !matches!(self.target.phase, TargetPhase::Commanded) {
            return false;
        }
        let Some(target) = &self.target.value else {
            return false;
        };
        let Some(actual) = &self.actual.value else {
            return false;
        };
        if close(target, actual) {
            self.target.phase = TargetPhase::Confirmed;
            self.target.since = now;
            true
        } else {
            false
        }
    }

    /// Decay `Fresh → Stale` on the actual side when the reading is older
    /// than `threshold`.
    pub fn tick(&mut self, now: Instant, threshold: Duration) {
        self.actual.tick(now, threshold);
    }
}

impl<V: PartialEq> Actuated<V> {
    /// Set a new target.
    ///
    /// - If the proposed `(value, owner)` matches the current target and the
    ///   phase is already past `Unset`, this is a no-op and returns `false`.
    /// - Otherwise transitions target phase to `Pending` (regardless of
    ///   whether the previous phase was `Unset`, `Pending`, `Commanded` or
    ///   `Confirmed`), and deprecates any `Fresh`/`Stale` actual value.
    ///
    /// Returns `true` if a change was applied.
    ///
    /// This is the primitive API. Higher-level rules — dead-band filtering
    /// ("don't retarget within ±25 W of current target"), owner-priority
    /// hold ("dashboard suppresses HA for 1 s"), etc. — are the caller's
    /// responsibility and should be applied **before** calling this method.
    pub fn propose_target(&mut self, value: V, owner: Owner, now: Instant) -> bool {
        let same_value = matches!(&self.target.value, Some(current) if *current == value);
        let same_owner = self.target.owner == owner;

        if same_value && same_owner && self.target.phase != TargetPhase::Unset {
            return false;
        }

        self.target.value = Some(value);
        self.target.owner = owner;
        self.target.phase = TargetPhase::Pending;
        self.target.since = now;

        self.actual.deprecate(now);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::Freshness;
    use super::*;

    fn at(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn new_starts_unset_and_unknown() {
        let t0 = Instant::now();
        let e: Actuated<i32> = Actuated::new(t0);
        assert_eq!(e.target.phase, TargetPhase::Unset);
        assert_eq!(e.target.owner, Owner::Unset);
        assert_eq!(e.target.value, None);
        assert_eq!(e.actual.freshness, Freshness::Unknown);
        assert_eq!(e.actual.value, None);
    }

    #[test]
    fn propose_target_from_unset_goes_pending() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        let changed = e.propose_target(-2300, Owner::SetpointController, at(t0, 1));
        assert!(changed);
        assert_eq!(e.target.phase, TargetPhase::Pending);
        assert_eq!(e.target.value, Some(-2300));
        assert_eq!(e.target.owner, Owner::SetpointController);
        assert_eq!(e.target.since, at(t0, 1));
    }

    #[test]
    fn mark_commanded_transitions_pending_to_commanded() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(100, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        assert_eq!(e.target.phase, TargetPhase::Commanded);
        assert_eq!(e.target.since, at(t0, 2));
    }

    #[test]
    fn mark_commanded_is_noop_outside_pending() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        // From Unset
        e.mark_commanded(at(t0, 1));
        assert_eq!(e.target.phase, TargetPhase::Unset);

        // From Confirmed
        e.propose_target(1, Owner::SetpointController, at(t0, 2));
        e.mark_commanded(at(t0, 3));
        e.on_reading(1, at(t0, 4));
        assert!(e.confirm_if(|t, a| t == a, at(t0, 5)));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);
        e.mark_commanded(at(t0, 6));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);
    }

    #[test]
    fn on_reading_alone_does_not_confirm() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(42, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(42, at(t0, 3));
        assert_eq!(
            e.target.phase,
            TargetPhase::Commanded,
            "reading alone must not promote to Confirmed"
        );
    }

    #[test]
    fn confirm_if_promotes_when_predicate_holds() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(42, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(42, at(t0, 3));
        let confirmed = e.confirm_if(|t, a| t == a, at(t0, 4));
        assert!(confirmed);
        assert_eq!(e.target.phase, TargetPhase::Confirmed);
        assert_eq!(e.target.since, at(t0, 4));
    }

    #[test]
    fn confirm_if_rejects_when_predicate_fails() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(42, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(99, at(t0, 3));
        let confirmed = e.confirm_if(|t, a| t == a, at(t0, 4));
        assert!(!confirmed);
        assert_eq!(e.target.phase, TargetPhase::Commanded);
    }

    #[test]
    fn confirm_if_outside_commanded_is_noop() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        // Unset
        assert!(!e.confirm_if(|_, _| true, at(t0, 1)));
        // Pending
        e.propose_target(1, Owner::SetpointController, at(t0, 2));
        e.on_reading(1, at(t0, 3));
        assert!(!e.confirm_if(|t, a| t == a, at(t0, 4)));
        assert_eq!(e.target.phase, TargetPhase::Pending);
    }

    #[test]
    fn confirm_if_with_tolerance_for_analog_values() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(-2300, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(-2312, at(t0, 3));
        let within_50w = |t: &i32, a: &i32| (*t - *a).abs() <= 50;
        assert!(e.confirm_if(within_50w, at(t0, 4)));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);
    }

    #[test]
    fn propose_same_value_same_owner_after_unset_is_noop() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        assert!(e.propose_target(5, Owner::SetpointController, at(t0, 1)));
        e.mark_commanded(at(t0, 2));
        e.on_reading(5, at(t0, 3));
        assert!(e.confirm_if(|t, a| t == a, at(t0, 4)));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);

        let since_before = e.target.since;
        let changed = e.propose_target(5, Owner::SetpointController, at(t0, 10));
        assert!(!changed);
        assert_eq!(
            e.target.phase,
            TargetPhase::Confirmed,
            "no retarget on identical proposal"
        );
        assert_eq!(e.target.since, since_before);
    }

    #[test]
    fn propose_different_value_supersedes_commanded() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        assert_eq!(e.target.phase, TargetPhase::Commanded);

        let changed = e.propose_target(20, Owner::SetpointController, at(t0, 3));
        assert!(changed);
        assert_eq!(e.target.phase, TargetPhase::Pending);
        assert_eq!(e.target.value, Some(20));
    }

    #[test]
    fn propose_different_value_from_confirmed_restarts_cycle() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(10, at(t0, 3));
        e.confirm_if(|t, a| t == a, at(t0, 4));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);

        let changed = e.propose_target(20, Owner::SetpointController, at(t0, 5));
        assert!(changed);
        assert_eq!(e.target.phase, TargetPhase::Pending);
    }

    #[test]
    fn propose_different_owner_same_value_is_a_change() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::HaMqtt, at(t0, 1));
        let changed = e.propose_target(10, Owner::Dashboard, at(t0, 2));
        assert!(changed);
        assert_eq!(e.target.owner, Owner::Dashboard);
        assert_eq!(e.target.phase, TargetPhase::Pending);
    }

    #[test]
    fn propose_deprecates_fresh_actual() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(10, at(t0, 3));
        assert_eq!(e.actual.freshness, Freshness::Fresh);

        e.propose_target(20, Owner::SetpointController, at(t0, 4));
        assert_eq!(
            e.actual.freshness,
            Freshness::Deprecated,
            "old reading describes the old target"
        );
        assert_eq!(e.actual.value, Some(10), "value preserved for diagnostics");
    }

    #[test]
    fn propose_leaves_unknown_actual_alone() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::SetpointController, at(t0, 1));
        assert_eq!(
            e.actual.freshness,
            Freshness::Unknown,
            "Unknown doesn't become Deprecated (there's no reading to deprecate)"
        );
    }

    #[test]
    fn reading_after_deprecation_returns_fresh_then_confirms() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(10, at(t0, 3));
        e.confirm_if(|t, a| t == a, at(t0, 4));

        // Supersede to a new target — old reading is now Deprecated.
        e.propose_target(20, Owner::SetpointController, at(t0, 5));
        e.mark_commanded(at(t0, 6));
        assert_eq!(e.actual.freshness, Freshness::Deprecated);

        // New reading arrives matching the new target.
        e.on_reading(20, at(t0, 7));
        assert_eq!(e.actual.freshness, Freshness::Fresh);
        assert!(e.confirm_if(|t, a| t == a, at(t0, 8)));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);
    }

    #[test]
    fn tick_decays_actual_side_only() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);
        e.propose_target(10, Owner::SetpointController, at(t0, 1));
        e.mark_commanded(at(t0, 2));
        e.on_reading(10, at(t0, 3));
        e.confirm_if(|t, a| t == a, at(t0, 4));

        let target_phase_before = e.target.phase;
        let target_since_before = e.target.since;

        e.tick(at(t0, 30), Duration::from_secs(5));
        assert_eq!(e.actual.freshness, Freshness::Stale);
        assert_eq!(e.target.phase, target_phase_before);
        assert_eq!(e.target.since, target_since_before);
    }

    #[test]
    fn full_lifecycle_integration() {
        let t0 = Instant::now();
        let mut e: Actuated<i32> = Actuated::new(t0);

        // 1. Setpoint controller proposes -2300 W.
        assert!(e.propose_target(-2300, Owner::SetpointController, at(t0, 1)));
        assert_eq!(e.target.phase, TargetPhase::Pending);

        // 2. Shell emits the D-Bus write.
        e.mark_commanded(at(t0, 2));
        assert_eq!(e.target.phase, TargetPhase::Commanded);

        // 3. A few ms later, readback arrives close enough.
        e.on_reading(-2312, at(t0, 3));
        assert!(e.confirm_if(|t, a| (*t - *a).abs() <= 50, at(t0, 4)));
        assert_eq!(e.target.phase, TargetPhase::Confirmed);

        // 4. 30 s pass with no fresh readback — decay.
        e.tick(at(t0, 34), Duration::from_secs(10));
        assert_eq!(e.actual.freshness, Freshness::Stale);

        // 5. Controller reproposes the same value — no-op.
        assert!(!e.propose_target(-2300, Owner::SetpointController, at(t0, 35)));

        // 6. Controller proposes a new value — restart cycle, deprecate stale.
        assert!(e.propose_target(-2500, Owner::SetpointController, at(t0, 36)));
        assert_eq!(e.target.phase, TargetPhase::Pending);
        assert_eq!(e.actual.freshness, Freshness::Deprecated);
    }
}
