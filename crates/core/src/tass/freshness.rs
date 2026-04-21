/// Freshness of an actual observation. See SPEC §5.3.
///
/// ```text
///                   reading                    time passes
///   Unknown ──────────────→ Fresh ───────────────────→ Stale
///                             ↑                          │
///                             └──────── reading ─────────┘
///
///   On target change (when freshness is Fresh or Stale):
///   Fresh/Stale ──→ Deprecated ──reading──→ Fresh
/// ```
///
/// `Deprecated` captures the moment when the old value still describes the
/// *previous* target but the target has since changed — a new reading is
/// required before the value can be trusted again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Freshness {
    Unknown,
    Fresh,
    Stale,
    Deprecated,
}

impl Freshness {
    /// A value with this freshness is safe to consume for control decisions.
    /// Only `Fresh` qualifies; `Stale` / `Deprecated` / `Unknown` must be
    /// treated as "we don't know".
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fresh_is_usable() {
        assert!(!Freshness::Unknown.is_usable());
        assert!(Freshness::Fresh.is_usable());
        assert!(!Freshness::Stale.is_usable());
        assert!(!Freshness::Deprecated.is_usable());
    }
}
