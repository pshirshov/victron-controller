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
use crate::types::{BookkeepingId, Effect, PublishPayload, SensorId, encode_sensor_body};
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
    /// Surface the freshly-derived `zappi_active` flag as the TASS DAG
    /// payload for the dashboard. PR-tass-dag-view.
    fn last_payload(&self, world: &World) -> Option<String> {
        Some(world.derived.zappi_active.to_string())
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

/// PR-ha-discovery-expand: emits one `Publish(Sensor{…})` per `SensorId`
/// and one `Publish(BookkeepingNumeric/Bool{…})` per published
/// bookkeeping field, dedup'd against `world.published_cache`.
///
/// Runs AFTER every actuator + derivation core (depends on
/// `WeatherSoc`, the topological tail of the actuator chain) so the
/// broadcast sees the latest `world.derived.zappi_active` and
/// post-controller bookkeeping. The dedup cache prevents this from
/// generating ~28 publishes/tick — only changed values get an effect.
pub(crate) struct SensorBroadcastCore;
impl Core for SensorBroadcastCore {
    fn id(&self) -> CoreId {
        CoreId::SensorBroadcast
    }
    fn depends_on(&self) -> &'static [CoreId] {
        // Depend on the actuator-chain tail so the broadcast picks up
        // any bookkeeping update controllers wrote during this tick,
        // and on `ZappiActive` as the spec says (so
        // `world.derived.zappi_active` is freshly written).
        &[CoreId::ZappiActive, CoreId::WeatherSoc]
    }
    fn run(
        &self,
        world: &mut World,
        _clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        // ----- Sensors -----
        // Iterate every variant. The `SensorId::ALL` table is the
        // single canonical list; `Sensors::by_id` is the matching
        // lookup helper.
        //
        // PR-ha-discovery-D03/D04 (resolved): dedup on the encoded WIRE
        // BODY rather than raw `f64::to_bits + freshness`. Reasons:
        // 1. Numeric formatting rounds to 3 decimals; raw `42.0001` and
        //    `42.0002` produce the same body but different bit patterns,
        //    so bit-dedup republishes identical bodies for noisy sensors.
        // 2. `(Fresh, None)` and `(Stale, None)` both encode to
        //    "unavailable"; bit-dedup would flap the publish on every
        //    flicker even though the wire value never changes.
        // The invariant we want is "publish iff the wire body differs",
        // so cache the body itself.
        for &id in SensorId::ALL {
            let actual = world.sensors.by_id(id);
            let body = encode_sensor_body(actual.value, actual.freshness);
            let prev = world.published_cache.sensors.get(&id);
            if prev.map(|s| s.as_str()) != Some(body.as_str()) {
                world.published_cache.sensors.insert(id, body);
                effects.push(Effect::Publish(PublishPayload::Sensor {
                    id,
                    value: actual.value,
                    freshness: actual.freshness,
                }));
            }
        }

        // ----- Bookkeeping booleans -----
        let bools: [(BookkeepingId, bool); 3] = [
            (BookkeepingId::ZappiActive, world.derived.zappi_active),
            (
                BookkeepingId::ChargeToFullRequired,
                world.bookkeeping.charge_to_full_required,
            ),
            (
                BookkeepingId::ChargeBatteryExtendedToday,
                world.bookkeeping.charge_battery_extended_today,
            ),
        ];
        for (id, value) in bools {
            let prev = world.published_cache.bookkeeping_bool.get(&id).copied();
            if prev != Some(value) {
                world.published_cache.bookkeeping_bool.insert(id, value);
                effects.push(Effect::Publish(PublishPayload::BookkeepingBool {
                    id,
                    value,
                }));
            }
        }

        // ----- Bookkeeping numerics -----
        // PR-ha-discovery-D01: `prev_ess_state` is intentionally NOT
        // surfaced here — its `bookkeeping/prev_ess_state/state` topic
        // is owned by the persistence path (`PublishPayload::Bookkeeping
        // (BookkeepingKey::PrevEssState, ...)`), which writes the
        // canonical `null`/int body for restore. Two writers on the
        // same retained topic would clobber.
        let nums: [(BookkeepingId, f64); 3] = [
            (
                BookkeepingId::SocEndOfDayTarget,
                world.bookkeeping.soc_end_of_day_target,
            ),
            (
                BookkeepingId::EffectiveExportSocThreshold,
                world.bookkeeping.effective_export_soc_threshold,
            ),
            (
                BookkeepingId::BatterySelectedSocTarget,
                world.bookkeeping.battery_selected_soc_target,
            ),
        ];
        for (id, value) in nums {
            let bits = value.to_bits();
            let prev = world
                .published_cache
                .bookkeeping_numeric
                .get(&id)
                .copied();
            if prev != Some(bits) {
                world
                    .published_cache
                    .bookkeeping_numeric
                    .insert(id, bits);
                effects.push(Effect::Publish(PublishPayload::BookkeepingNumeric {
                    id,
                    value,
                }));
            }
        }
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
        Box::new(SensorBroadcastCore),
    ]
}
