//! Pure core for the victron-controller service.
//!
//! This crate has no async, no I/O, and no external deps beyond `chrono`
//! (opt-out of its clock feature so the core can't read wall time by
//! accident). See `SPEC.md` at the repo root for the broader architecture.
//!
//! Sections relevant to this crate:
//!
//! - §4 Design principles (TASS, pure-core/async-shell split)
//! - §5.2 TASS entity catalogue
//! - §5.3 Target phase and freshness state machines
//! - §5.4 Target owner
//! - §5.11 Grid export hard cap (via `prepare_setpoint` + the grid-side cap
//!   applied by the outer `process()` — not yet wired in this crate)
//! - §7 Hard-coded safe defaults ([`knobs::Knobs::safe_defaults`])
//!
//! The shell crate (not in this crate) handles I/O, connects events to this
//! core, and executes the effects the core returns.

pub mod clock;
pub mod controllers;
pub mod knobs;
pub mod owner;
pub mod tass;

pub use clock::{Clock, FixedClock};
pub use knobs::{DebugFullCharge, DischargeTime, ForecastDisagreementStrategy, Knobs};
pub use owner::Owner;
pub use tass::{Actual, Actuated, Freshness, Target, TargetPhase, Timestamped};
