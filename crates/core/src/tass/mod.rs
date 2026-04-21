//! TASS primitives: target/actual state separation.
//!
//! See SPEC §5.2–§5.3. The entities themselves (`GridSetpoint`, `BatterySoc`,
//! …) are defined later, in the `world` module; this module only provides the
//! generic building blocks.

mod actual;
mod actuated;
mod freshness;
mod phase;
mod timestamped;

pub use actual::Actual;
pub use actuated::{Actuated, Target};
pub use freshness::Freshness;
pub use phase::TargetPhase;
pub use timestamped::Timestamped;
