//! Pure core for the victron-controller service.
//!
//! This crate has no async, no I/O, and no external dependencies — by design.
//! See `SPEC.md` at the repo root for the broader architecture; the relevant
//! sections for this crate are:
//!
//! - §4 Design principles (TASS, pure-core/async-shell split)
//! - §5.2 TASS entity catalogue
//! - §5.3 Target phase and freshness state machines
//! - §5.4 Target owner
//!
//! The shell crate (not in this crate) handles I/O, connects events to this
//! core, and executes the effects the core returns.

pub mod owner;
pub mod tass;

pub use owner::Owner;
pub use tass::{Actual, Actuated, Freshness, Target, TargetPhase, Timestamped};
