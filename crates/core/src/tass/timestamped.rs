use std::time::Instant;

/// A value plus the monotonic instant at which it became current.
///
/// Used for any "since" timestamp in the TASS machines (target phase
/// transitions, actual-reading timestamps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamped<T> {
    pub value: T,
    pub since: Instant,
}

impl<T> Timestamped<T> {
    pub const fn new(value: T, since: Instant) -> Self {
        Self { value, since }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_preserves_fields() {
        let t0 = Instant::now();
        let ts = Timestamped::new(42_i32, t0);
        assert_eq!(ts.value, 42);
        assert_eq!(ts.since, t0);
    }
}
