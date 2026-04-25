//! Pure core for the victron-controller service.
//!
//! This crate has no async, no I/O, and no external deps beyond `chrono`
//! (with clock feature disabled so the core can't read wall time by
//! accident). See `SPEC.md` at the repo root for the broader architecture.

pub mod clock;
pub mod controllers;
pub mod core_dag;
pub mod knobs;
pub mod myenergi;
pub mod owner;
pub mod process;
pub mod tass;
pub mod topology;
pub mod types;
pub mod world;

pub use clock::{Clock, FixedClock};
pub use knobs::{DebugFullCharge, DischargeTime, ForecastDisagreementStrategy, Knobs};
pub use owner::Owner;
pub use process::process;
pub use tass::{Actual, Actuated, Freshness, Target, TargetPhase, Timestamped};
pub use topology::{ControllerParams, HardwareParams, Topology};
pub use types::{
    check_staleness_invariant, ActuatedId, ActuatedReadback, BookkeepingId, BookkeepingKey,
    BookkeepingValue, Command, DbusTarget, DbusValue, Effect, Event, ForecastProvider,
    FreshnessRegime, KnobId, KnobValue, LogLevel, MyenergiAction, PublishPayload, ScheduleField,
    SensorId, SensorReading, TypedReading,
};
pub use world::{Bookkeeping, ForecastSnapshot, Sensors, TypedSensors, World};
