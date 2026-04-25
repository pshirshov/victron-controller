//! Lock-free timezone handle threaded through `Topology` so the shell's
//! `RealClock` can read the Victron-supplied display TZ from any thread.
//!
//! Venus's `/etc/timezone` is fixed at `Universal` (UTC); the GX
//! software does timezone conversion in-process. To keep wall-clock-of-
//! day controllers (eddi dwell, schedules, full-charge rollover) honest
//! we read `/Settings/System/TimeZone` over D-Bus and feed it through
//! here. Default is `chrono_tz::UTC` until the first reading lands —
//! the controller still functions correctly during that brief window
//! since the shell's `RealClock::naive` falls back to UTC, which is the
//! same value Venus's `/etc/timezone` reports.

use std::sync::Arc;

use arc_swap::ArcSwap;

/// Shared, lock-free Tz cell. Every clone refers to the same atomic
/// pointer — calling `set` from one clone is observed by every other.
///
/// `Clone + Send + Sync` so it can be embedded in `Topology` and read
/// from the controller-thread `RealClock` without any locking.
#[derive(Clone)]
pub struct TzHandle {
    tz: Arc<ArcSwap<chrono_tz::Tz>>,
}

impl TzHandle {
    /// New handle defaulted to UTC.
    #[must_use]
    pub fn new_utc() -> Self {
        Self {
            tz: Arc::new(ArcSwap::from_pointee(chrono_tz::UTC)),
        }
    }

    /// Read the current Tz. Lock-free atomic pointer load.
    #[must_use]
    pub fn current(&self) -> chrono_tz::Tz {
        **self.tz.load()
    }

    /// Atomically swap in a new Tz. Subsequent reads from any clone
    /// observe the new value.
    pub fn set(&self, tz: chrono_tz::Tz) {
        self.tz.store(Arc::new(tz));
    }

    /// A no-op handle for tests that don't care whether
    /// `apply_event(Event::Timezone, ...)` mutates anything visible.
    /// Equivalent to [`Self::new_utc`]; named separately so the
    /// intent reads at the test call site.
    #[must_use]
    pub fn noop() -> Self {
        Self::new_utc()
    }
}

impl std::fmt::Debug for TzHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TzHandle")
            .field("current", &self.current())
            .finish()
    }
}

impl PartialEq for TzHandle {
    /// `Topology` derives `PartialEq` (used in tests). Two TzHandles
    /// compare equal iff they currently hold the same Tz — this is
    /// what tests asserting "topology equality" want.
    fn eq(&self, other: &Self) -> bool {
        self.current() == other.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_utc() {
        let h = TzHandle::new_utc();
        assert_eq!(h.current(), chrono_tz::UTC);
    }

    #[test]
    fn set_propagates_across_clones() {
        let a = TzHandle::new_utc();
        let b = a.clone();
        a.set(chrono_tz::Europe::London);
        assert_eq!(b.current(), chrono_tz::Europe::London);
    }
}
