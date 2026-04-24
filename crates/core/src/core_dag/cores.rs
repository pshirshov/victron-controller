//! Zero-sized `Core` impls wrapping each existing `run_*` controller
//! plus the first derivation core.
//!
//! PR-DAG-B: `ZappiActiveCore` is a first-class derivation core that
//! writes `world.derived.zappi_active` at the top of every tick. The
//! three actuator cores that read it (`Setpoint`, `CurrentLimit`,
//! `Schedules`) declare `depends_on = [ZappiActive]` so the topological
//! sort runs the derivation first.
//!
//! The residual `Setpoint → CurrentLimit → Schedules → ZappiMode →
//! EddiMode → WeatherSoc` chain is still a PR-DAG-A placeholder —
//! PR-DAG-C replaces each of those edges with a semantic one derived
//! from the bookkeeping-write/read audit in
//! `docs/drafts/20260424-1700-m-audit-2-pr-dag-plan.md` §4.

use crate::Clock;
use crate::controllers::zappi_active::classify_zappi_active;
use crate::process::{
    run_current_limit, run_eddi_mode, run_schedules, run_setpoint, run_weather_soc,
    run_zappi_mode,
};
use crate::topology::Topology;
use crate::types::Effect;
use crate::world::World;

use super::{Core, CoreId};

pub(crate) struct ZappiActiveCore;
impl Core for ZappiActiveCore {
    fn id(&self) -> CoreId {
        CoreId::ZappiActive
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[]
    }
    /// Writes `world.derived.zappi_active` from a single canonical
    /// `classify_zappi_active` call per tick.
    ///
    /// Semantic choice: when BOTH `typed_sensors.zappi_state` and
    /// `sensors.evcharger_ac_power` are unusable (`Stale` / `Unknown`),
    /// the classifier returns `false`. The prior-tick value is NOT
    /// latched — this is a deliberate departure from PR-04's
    /// `bookkeeping.zappi_active`, which effectively latched through
    /// sensor loss because `run_current_limit` early-returned on the
    /// freshness gate and left the stored global untouched. Latching
    /// hid sensor loss; the new semantic surfaces it honestly and is
    /// safer — don't hog EV current for a car we can't see. Locked by
    /// `zappi_active_drops_to_false_when_both_sensor_paths_unusable`
    /// and `zappi_active_uses_power_fallback_when_typed_state_is_stale`
    /// in `core_dag::tests`.
    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        _topology: &Topology,
        _effects: &mut Vec<Effect>,
    ) {
        world.derived.zappi_active = classify_zappi_active(world, clock);
    }
}

pub(crate) struct SetpointCore;
impl Core for SetpointCore {
    fn id(&self) -> CoreId {
        CoreId::Setpoint
    }
    fn depends_on(&self) -> &'static [CoreId] {
        &[CoreId::ZappiActive]
    }
    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_setpoint(world, clock, topology, effects);
    }
}

pub(crate) struct CurrentLimitCore;
impl Core for CurrentLimitCore {
    fn id(&self) -> CoreId {
        CoreId::CurrentLimit
    }
    fn depends_on(&self) -> &'static [CoreId] {
        // Semantic edge on ZappiActive (reads `world.derived.zappi_active`);
        // the edge on Setpoint is PR-DAG-A placeholder chain preservation.
        &[CoreId::ZappiActive, CoreId::Setpoint]
    }
    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_current_limit(world, clock, topology, effects);
    }
}

pub(crate) struct SchedulesCore;
impl Core for SchedulesCore {
    fn id(&self) -> CoreId {
        CoreId::Schedules
    }
    fn depends_on(&self) -> &'static [CoreId] {
        // Semantic edge on ZappiActive (reads `world.derived.zappi_active`);
        // the edge on CurrentLimit is PR-DAG-A placeholder chain preservation.
        &[CoreId::ZappiActive, CoreId::CurrentLimit]
    }
    fn run(
        &self,
        world: &mut World,
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
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        run_weather_soc(world, clock, topology, effects);
    }
}

/// The production list of cores, in registration order. The registry
/// reorders them topologically — registration order is irrelevant for
/// correctness.
pub(crate) fn production_cores() -> Vec<Box<dyn Core>> {
    vec![
        Box::new(ZappiActiveCore),
        Box::new(SetpointCore),
        Box::new(CurrentLimitCore),
        Box::new(SchedulesCore),
        Box::new(ZappiModeCore),
        Box::new(EddiModeCore),
        Box::new(WeatherSocCore),
    ]
}
