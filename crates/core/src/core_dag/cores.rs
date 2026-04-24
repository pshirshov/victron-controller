//! Zero-sized `Core` impls wrapping each existing `run_*` controller.
//!
//! **PR-DAG-A `depends_on` wiring is a placeholder.** Each actuator
//! core declares a single edge back to the previous entry in today's
//! hand-rolled execution order:
//!
//!   Setpoint → CurrentLimit → Schedules → ZappiMode → EddiMode → WeatherSoc
//!
//! These edges are NOT semantically meaningful yet — they only exist
//! to force the topological sort to reproduce the pre-refactor order
//! byte-for-byte. PR-DAG-C replaces them with real edges derived from
//! the bookkeeping-write/read audit in
//! `docs/drafts/20260424-1700-m-audit-2-pr-dag-plan.md` §4.
//!
//! **PR-DAG-A-D01:** `DerivedView` is computed ONCE per tick by
//! `CoreRegistry::run_all` and threaded in as a parameter. Cores that
//! don't consume it still accept it (`_derived`) so the trait signature
//! stays uniform. Computing `DerivedView` twice per tick — as a
//! previous revision did from SetpointCore/CurrentLimitCore
//! independently — re-opened the A-05 hazard across the
//! `WAIT_TIMEOUT_MIN` boundary: two uncached `clock.naive()` reads
//! could straddle 5 min and make the two cores disagree.

use crate::Clock;
use crate::process::{
    DerivedView, run_current_limit, run_eddi_mode, run_schedules, run_setpoint,
    run_weather_soc, run_zappi_mode,
};
use crate::topology::Topology;
use crate::types::Effect;
use crate::world::World;

use super::{Core, CoreId};

pub(crate) struct SetpointCore;
impl Core for SetpointCore {
    fn id(&self) -> CoreId {
        CoreId::Setpoint
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[]
    }
    fn run(
        &self,
        world: &mut World,
        derived: &DerivedView,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_setpoint(world, *derived, clock, topology, effects);
    }
}

pub(crate) struct CurrentLimitCore;
impl Core for CurrentLimitCore {
    fn id(&self) -> CoreId {
        CoreId::CurrentLimit
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[CoreId::Setpoint]
    }
    fn run(
        &self,
        world: &mut World,
        derived: &DerivedView,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_current_limit(world, *derived, clock, topology, effects);
    }
}

pub(crate) struct SchedulesCore;
impl Core for SchedulesCore {
    fn id(&self) -> CoreId {
        CoreId::Schedules
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[CoreId::CurrentLimit]
    }
    fn run(
        &self,
        world: &mut World,
        _derived: &DerivedView,
        clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_schedules(world, clock, effects);
    }
}

pub(crate) struct ZappiModeCore;
impl Core for ZappiModeCore {
    fn id(&self) -> CoreId {
        CoreId::ZappiMode
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[CoreId::Schedules]
    }
    fn run(
        &self,
        world: &mut World,
        _derived: &DerivedView,
        clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_zappi_mode(world, clock, effects);
    }
}

pub(crate) struct EddiModeCore;
impl Core for EddiModeCore {
    fn id(&self) -> CoreId {
        CoreId::EddiMode
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[CoreId::ZappiMode]
    }
    fn run(
        &self,
        world: &mut World,
        _derived: &DerivedView,
        clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_eddi_mode(world, clock, effects);
    }
}

pub(crate) struct WeatherSocCore;
impl Core for WeatherSocCore {
    fn id(&self) -> CoreId {
        CoreId::WeatherSoc
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[CoreId::EddiMode]
    }
    fn run(
        &self,
        world: &mut World,
        _derived: &DerivedView,
        clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_weather_soc(world, clock, effects);
    }
}

/// The production list of actuator cores, in registration order. The
/// registry reorders them topologically — registration order is
/// irrelevant for correctness.
///
/// PR-DAG-A does NOT yet include any derivation cores. `CoreId::ZappiActive`
/// is reserved but its `Core` impl lands in PR-DAG-B.
pub(crate) fn production_cores() -> Vec<Box<dyn Core>> {
    vec![
        Box::new(SetpointCore),
        Box::new(CurrentLimitCore),
        Box::new(SchedulesCore),
        Box::new(ZappiModeCore),
        Box::new(EddiModeCore),
        Box::new(WeatherSocCore),
    ]
}
