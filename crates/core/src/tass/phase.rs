/// Phase of a target value's lifecycle. See SPEC §5.3.
///
/// ```text
/// Unset ──set_target──> Pending ──mark_commanded──> Commanded ──confirm──> Confirmed
///                          ↑                                                   │
///                          └──set_target [new target]──────────────────────────┘
/// ```
///
/// Invariants enforced by [`super::Actuated`]:
///
/// - `Unset` is only reachable at construction.
/// - Transitions `Pending → Commanded → Confirmed` cannot skip steps.
/// - Setting a new target always returns the phase to `Pending`, regardless
///   of the current phase (including `Commanded`, i.e. supersession).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetPhase {
    /// No target value has been set.
    Unset,
    /// A target was set but the command has not been emitted yet.
    Pending,
    /// The command was emitted; awaiting an actual reading that matches.
    Commanded,
    /// An actual reading confirmed the target within tolerance.
    Confirmed,
}

impl TargetPhase {
    /// True when the phase represents an *outstanding* target — one that
    /// still needs action (emit command) or verification (await actual).
    pub const fn is_outstanding(self) -> bool {
        matches!(self, Self::Pending | Self::Commanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_outstanding_is_accurate() {
        assert!(!TargetPhase::Unset.is_outstanding());
        assert!(TargetPhase::Pending.is_outstanding());
        assert!(TargetPhase::Commanded.is_outstanding());
        assert!(!TargetPhase::Confirmed.is_outstanding());
    }
}
