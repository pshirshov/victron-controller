//! Zero-sized `Core` impls wrapping each existing `run_*` controller
//! plus the first derivation core.
//!
//! PR-DAG-B: `ZappiActiveCore` is a first-class derivation core that
//! writes `world.derived.zappi_active` at the top of every tick. The
//! three actuator cores that read it (`Setpoint`, `CurrentLimit`,
//! `Schedules`) declare a `DepEdge` on `ZappiActive` so the topological
//! sort runs the derivation first.
//!
//! PR-DAG-C: every `depends_on` below is a semantic edge — the `from`
//! is the producing core, the `fields` are the live `world.<area>.<field>`
//! identifiers actually read by the consumer. The PR-DAG-A linear chain
//! placeholder edges (Setpoint → CurrentLimit → Schedules → ZappiMode →
//! EddiMode → WeatherSoc) are gone. Sources of truth: the
//! bookkeeping-write/read audit in
//! `docs/drafts/20260424-1700-m-audit-2-pr-dag-plan.md` §4 and each
//! controller's `last_inputs`/`last_outputs` impl in this file.
//!
//! Notable behavioural change in PR-DAG-C: `CurrentLimit` now declares
//! a real edge on `Schedules` (via `battery_selected_soc_target`). Pre-
//! PR-DAG-C, `CurrentLimit` ran *before* `Schedules` in the linear
//! chain, so it always read yesterday's tick's value — a one-tick
//! semantic latency. The new edge flips the order to give zero-latency
//! same-tick reads. Locked by
//! `current_limit_reads_same_tick_battery_selected_soc_target`.

use std::time::Duration;

use crate::Clock;
use crate::Owner;
use crate::controllers::ess_state_override::{
    EssStateOverrideInput, evaluate_ess_state_override,
};
use crate::controllers::heat_pump::evaluate_heat_pump;
use crate::controllers::zappi_active::classify_zappi_active;
use crate::process::{
    build_current_limit_input, build_eddi_mode_input, build_schedules_input,
    build_setpoint_input, build_zappi_mode_input, cbe_derivation, run_current_limit,
    run_eddi_mode, run_schedules, run_setpoint, run_weather_soc, run_zappi_mode,
};
use crate::tass::{Actual, Freshness};
use crate::topology::{HardwareParams, Topology};
use crate::types::{
    ActuatedId, BookkeepingId, ControllerObservableId, DbusTarget, DbusValue, Effect,
    ForecastProvider, LgThinqAction, LogLevel, PublishPayload, SensorId, ZappiDrainBranch,
    encode_sensor_body,
};
use crate::world::{CoreFactor, SUNRISE_SUNSET_FRESHNESS, World};

use super::{Core, CoreId, DepEdge};

/// Pretty-print an `Actual<f64>` with a `Fresh`/`Stale`/`Unknown` suffix
/// for the popup. Mirrors how the dashboard renders sensor values
/// inline. PR-core-io-popups.
fn fmt_actual_f64(a: Actual<f64>) -> String {
    match a.value {
        Some(v) => format!("{v:.2} ({:?})", a.freshness),
        None => format!("— ({:?})", a.freshness),
    }
}

fn factor(id: &str, value: impl Into<String>) -> CoreFactor {
    CoreFactor { id: id.to_string(), value: value.into() }
}

pub(crate) struct ZappiActiveCore;
impl Core for ZappiActiveCore {
    fn id(&self) -> CoreId {
        CoreId::ZappiActive
    }
    fn depends_on(&self) -> &'static [DepEdge] {
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

    /// PR-core-io-popups: surface the sensor + typed-state inputs that
    /// `classify_zappi_active` consults, plus the elapsed time since the
    /// last (zmo, sta, pst) tuple change, so the popup makes the
    /// derivation legible without rerunning the classifier.
    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        let zappi = world.typed_sensors.zappi_state;
        let zappi_mode = match zappi.value {
            Some(s) => format!("{:?} ({:?})", s.zappi_mode, zappi.freshness),
            None => format!("— ({:?})", zappi.freshness),
        };
        let plug_state = match zappi.value {
            Some(s) => format!("{:?} ({:?})", s.zappi_plug_state, zappi.freshness),
            None => format!("— ({:?})", zappi.freshness),
        };
        let zappi_status = match zappi.value {
            Some(s) => format!("{:?}", s.zappi_status),
            None => "—".to_string(),
        };
        let evcharger_w = fmt_actual_f64(world.sensors.evcharger_ac_power);
        let zappi_a = fmt_actual_f64(world.sensors.evcharger_ac_current);
        vec![
            factor("zappi_mode", zappi_mode),
            factor("zappi_plug_state", plug_state),
            factor("zappi_status", zappi_status),
            factor("evcharger_ac_power_W", evcharger_w),
            factor("zappi_amps_A", zappi_a),
        ]
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        vec![factor("zappi_active", world.derived.zappi_active.to_string())]
    }
}

pub(crate) struct SetpointCore;
impl Core for SetpointCore {
    fn id(&self) -> CoreId {
        CoreId::Setpoint
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        &[DepEdge {
            from: CoreId::ZappiActive,
            fields: &["derived.zappi_active"],
        }]
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

    /// PR-core-io-popups: surface every field of the live `SetpointInput`
    /// that `run_setpoint` would build this tick, or a placeholder
    /// "safety fallback" entry when the Fresh-sensor preconditions fail
    /// (which is the path that drives the `apply_setpoint_safety` 10 W
    /// idle target). `last_outputs` surfaces the actuated target plus
    /// the bookkeeping the controller wrote on the last successful tick.
    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        match build_setpoint_input(world, HardwareParams::defaults().idle_setpoint_w as i32) {
            None => vec![factor("status", "safety fallback (required sensors not usable)")],
            Some(i) => {
                let g = &i.globals;
                vec![
                    factor("force_disable_export", format!("{}", g.force_disable_export)),
                    factor("export_soc_threshold", format!("{:.2}", g.export_soc_threshold)),
                    factor("discharge_soc_target", format!("{:.2}", g.discharge_soc_target)),
                    factor(
                        "full_charge_export_soc_threshold",
                        format!("{:.2}", g.full_charge_export_soc_threshold),
                    ),
                    factor(
                        "full_charge_discharge_soc_target",
                        format!("{:.2}", g.full_charge_discharge_soc_target),
                    ),
                    factor("zappi_active", format!("{}", g.zappi_active)),
                    factor("allow_battery_to_car", format!("{}", g.allow_battery_to_car)),
                    factor("discharge_time", format!("{:?}", g.discharge_time)),
                    factor("debug_full_charge", format!("{:?}", g.debug_full_charge)),
                    factor(
                        "pessimism_multiplier_modifier",
                        format!("{:.2}", g.pessimism_multiplier_modifier),
                    ),
                    factor(
                        "next_full_charge",
                        g.next_full_charge.map_or("—".to_string(), |d| format!("{d}")),
                    ),
                    factor("power_consumption_W", format!("{:.2}", i.power_consumption)),
                    factor("battery_soc_pct", format!("{:.2}", i.battery_soc)),
                    factor("battery_soh_pct", format!("{:.2}", i.soh)),
                    factor("mppt_power_0_W", format!("{:.2}", i.mppt_power_0)),
                    factor("mppt_power_1_W", format!("{:.2}", i.mppt_power_1)),
                    factor("soltaro_power_W", format!("{:.2}", i.soltaro_power)),
                    factor("evcharger_ac_power_W", format!("{:.2}", i.evcharger_ac_power)),
                    factor("battery_capacity_Ah", format!("{:.2}", i.capacity)),
                ]
            }
        }
    }

    /// We don't keep last `SetpointOutput` around, so surface the values
    /// the controller persisted into bookkeeping plus the actuated target.
    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let target = world
            .grid_setpoint
            .target
            .value
            .map_or("—".to_string(), |v| format!("{v} W"));
        let bk = &world.bookkeeping;
        vec![
            factor("setpoint_target_W", target),
            factor(
                "next_full_charge",
                bk.next_full_charge.map_or("—".to_string(), |d| format!("{d}")),
            ),
            factor("charge_to_full_required", format!("{}", bk.charge_to_full_required)),
            factor("soc_end_of_day_target", format!("{:.2}", bk.soc_end_of_day_target)),
            factor(
                "effective_export_soc_threshold",
                format!("{:.2}", bk.effective_export_soc_threshold),
            ),
        ]
    }
}

pub(crate) struct CurrentLimitCore;
impl Core for CurrentLimitCore {
    fn id(&self) -> CoreId {
        CoreId::CurrentLimit
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        &[
            DepEdge {
                from: CoreId::ZappiActive,
                fields: &["derived.zappi_active"],
            },
            DepEdge {
                from: CoreId::Setpoint,
                fields: &["bookkeeping.charge_to_full_required"],
            },
            // PR-DAG-C: real ordering change. Pre-PR `run_current_limit`
            // ran before `run_schedules` and read yesterday's
            // `battery_selected_soc_target`. The new edge flips the order
            // to give zero-tick latency same-tick reads.
            DepEdge {
                from: CoreId::Schedules,
                fields: &["bookkeeping.battery_selected_soc_target"],
            },
        ]
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

    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        match build_current_limit_input(world) {
            None => vec![factor("status", "skipped (required sensors / zappi_state not usable)")],
            Some(i) => {
                let g = &i.globals;
                vec![
                    factor("zappi_current_target_A", format!("{:.2}", g.zappi_current_target)),
                    factor("zappi_emergency_margin_A", format!("{:.2}", g.zappi_emergency_margin)),
                    factor("zappi_state.zappi_mode", format!("{:?}", g.zappi_state.zappi_mode)),
                    factor(
                        "zappi_state.zappi_plug_state",
                        format!("{:?}", g.zappi_state.zappi_plug_state),
                    ),
                    factor("zappi_state.zappi_status", format!("{:?}", g.zappi_state.zappi_status)),
                    factor("zappi_active", format!("{}", g.zappi_active)),
                    factor("extended_charge_required", format!("{}", g.extended_charge_required)),
                    factor(
                        "disable_night_grid_discharge",
                        format!("{}", g.disable_night_grid_discharge),
                    ),
                    factor("battery_soc_target_pct", format!("{:.2}", g.battery_soc_target)),
                    factor("consumption_power_W", format!("{:.2}", i.consumption_power)),
                    factor("offgrid_power_W", format!("{:.2}", i.offgrid_power)),
                    factor("offgrid_current_A", format!("{:.2}", i.offgrid_current)),
                    factor("grid_voltage_V", format!("{:.2}", i.grid_voltage)),
                    factor("grid_power_W", format!("{:.2}", i.grid_power)),
                    factor("mppt_power_0_W", format!("{:.2}", i.mppt_power_0)),
                    factor("mppt_power_1_W", format!("{:.2}", i.mppt_power_1)),
                    factor("soltaro_power_W", format!("{:.2}", i.soltaro_power)),
                    factor("zappi_current_A", format!("{:.2}", i.zappi_current)),
                    factor("ess_state", format!("{}", i.ess_state)),
                    factor("battery_power_W", format!("{:.2}", i.battery_power)),
                    factor("battery_soc_pct", format!("{:.2}", i.battery_soc)),
                ]
            }
        }
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let target = world
            .input_current_limit
            .target
            .value
            .map_or("—".to_string(), |v| format!("{v:.2} A"));
        vec![factor("input_current_limit_A", target)]
    }
}

pub(crate) struct SchedulesCore;
impl Core for SchedulesCore {
    fn id(&self) -> CoreId {
        CoreId::Schedules
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        &[
            DepEdge {
                from: CoreId::ZappiActive,
                fields: &["derived.zappi_active"],
            },
            DepEdge {
                from: CoreId::Setpoint,
                fields: &["bookkeeping.charge_to_full_required"],
            },
            DepEdge {
                from: CoreId::WeatherSoc,
                fields: &["bookkeeping.charge_battery_extended_today"],
            },
        ]
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

    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        let cbe = cbe_derivation(world);
        match build_schedules_input(world) {
            None => vec![factor("status", "skipped (battery_soc not usable)")],
            Some(i) => {
                let g = &i.globals;
                vec![
                    factor("charge_battery_extended", format!("{}", g.charge_battery_extended)),
                    factor("cbe_from_full", format!("{}", cbe.from_full)),
                    factor("cbe_from_weather", format!("{}", cbe.from_weather)),
                    factor("cbe_derived", format!("{}", cbe.derived)),
                    factor(
                        "cbe_mode",
                        format!("{:?}", world.knobs.charge_battery_extended_mode),
                    ),
                    factor("charge_car_extended", format!("{}", g.charge_car_extended)),
                    factor("charge_to_full_required", format!("{}", g.charge_to_full_required)),
                    factor(
                        "disable_night_grid_discharge",
                        format!("{}", g.disable_night_grid_discharge),
                    ),
                    factor("zappi_active", format!("{}", g.zappi_active)),
                    factor(
                        "above_soc_date",
                        g.above_soc_date.map_or("—".to_string(), |d| format!("{d}")),
                    ),
                    factor("battery_soc_target_pct", format!("{:.2}", g.battery_soc_target)),
                    factor("battery_soc_pct", format!("{:.2}", i.battery_soc)),
                ]
            }
        }
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let s0 = world
            .schedule_0
            .target
            .value
            .map_or("—".to_string(), |s| format!("{s:?}"));
        let s1 = world
            .schedule_1
            .target
            .value
            .map_or("—".to_string(), |s| format!("{s:?}"));
        let bk = &world.bookkeeping;
        vec![
            factor("schedule_0", s0),
            factor("schedule_1", s1),
            factor(
                "battery_selected_soc_target",
                format!("{:.2}", bk.battery_selected_soc_target),
            ),
            factor(
                "above_soc_date",
                bk.above_soc_date.map_or("—".to_string(), |d| format!("{d}")),
            ),
        ]
    }
}

pub(crate) struct ZappiModeCore;
impl Core for ZappiModeCore {
    fn id(&self) -> CoreId {
        CoreId::ZappiMode
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        // No real cross-core reads — `evaluate_zappi_mode` consumes
        // sensors + knobs only. The PR-DAG-A `[Schedules]` edge was
        // pure linear-chain placeholder.
        &[]
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

    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        match build_zappi_mode_input(world) {
            None => vec![factor("status", "skipped (zappi_state not usable)")],
            Some(i) => {
                let g = &i.globals;
                vec![
                    factor("charge_car_boost", format!("{}", g.charge_car_boost)),
                    factor("charge_car_extended", format!("{}", g.charge_car_extended)),
                    // PR-auto-extended-charge: surface the mode + the
                    // bookkeeping latch the `Auto` arm consults so the
                    // popup explains the effective bool above.
                    factor(
                        "charge_car_extended_mode",
                        format!("{:?}", world.knobs.charge_car_extended_mode),
                    ),
                    factor(
                        "auto_extended_today",
                        format!("{}", world.bookkeeping.auto_extended_today),
                    ),
                    factor("zappi_limit_kwh", format!("{:.2}", g.zappi_limit_kwh)),
                    factor("current_mode", format!("{:?}", i.current_mode)),
                    factor("session_kwh", format!("{:.2}", i.session_kwh)),
                ]
            }
        }
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let target = world
            .zappi_mode
            .target
            .value
            .map_or("—".to_string(), |m| format!("{m:?}"));
        vec![factor("zappi_mode_target", target)]
    }
}

pub(crate) struct EddiModeCore;
impl Core for EddiModeCore {
    fn id(&self) -> CoreId {
        CoreId::EddiMode
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        // No real cross-core reads — `evaluate_eddi_mode` consumes
        // `battery_soc` + the eddi knobs only. The PR-DAG-A
        // `[ZappiMode]` edge was pure linear-chain placeholder.
        &[]
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

    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        let i = build_eddi_mode_input(world);
        let soc = match i.soc_value {
            Some(v) => format!("{v:.2} ({:?})", i.soc_freshness),
            None => format!("— ({:?})", i.soc_freshness),
        };
        vec![
            factor("battery_soc_pct", soc),
            factor("current_mode", format!("{:?}", i.current_mode)),
            factor(
                "last_transition_at",
                i.last_transition_at.map_or("—".to_string(), |_| "set".to_string()),
            ),
            factor("enable_soc_pct", format!("{:.2}", i.knobs.enable_soc)),
            factor("disable_soc_pct", format!("{:.2}", i.knobs.disable_soc)),
            factor("dwell_s", format!("{}", i.knobs.dwell_s)),
        ]
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let target = world
            .eddi_mode
            .target
            .value
            .map_or("—".to_string(), |m| format!("{m:?}"));
        vec![factor("eddi_mode_target", target)]
    }
}

pub(crate) struct WeatherSocCore;
impl Core for WeatherSocCore {
    fn id(&self) -> CoreId {
        CoreId::WeatherSoc
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        // `run_weather_soc` reads `bookkeeping.charge_to_full_required`
        // (written by `Setpoint`) — see `process.rs` cbe-eligibility
        // arms. The PR-DAG-A `[EddiMode]` edge was pure linear-chain
        // placeholder; nothing in WeatherSoc actually reads anything
        // EddiMode produces.
        &[DepEdge {
            from: CoreId::Setpoint,
            fields: &["bookkeeping.charge_to_full_required"],
        }]
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

    /// PR-core-io-popups: surface forecast totals + temperature + the
    /// current planner-knob thresholds. The planner only fires once per
    /// day at 01:55, so most ticks won't produce a fresh
    /// `WeatherSocInput`; rather than reproduce the forecast-fusion
    /// gating here, surface the underlying provider snapshots so the
    /// operator can see what the next 01:55 evaluation will see.
    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        let k = &world.knobs;
        let bk = &world.bookkeeping;
        let temp = fmt_actual_f64(world.sensors.outdoor_temperature);
        let providers = [
            ("solcast", ForecastProvider::Solcast),
            ("forecast_solar", ForecastProvider::ForecastSolar),
            ("open_meteo", ForecastProvider::OpenMeteo),
        ];
        let mut out = vec![
            factor("outdoor_temperature_C", temp),
            factor("charge_to_full_required", format!("{}", bk.charge_to_full_required)),
            factor(
                "winter_temperature_threshold_C",
                format!("{:.2}", k.weathersoc_winter_temperature_threshold),
            ),
            factor(
                "low_energy_threshold_kWh",
                format!("{:.2}", k.weathersoc_low_energy_threshold),
            ),
            factor(
                "ok_energy_threshold_kWh",
                format!("{:.2}", k.weathersoc_ok_energy_threshold),
            ),
            factor(
                "high_energy_threshold_kWh",
                format!("{:.2}", k.weathersoc_high_energy_threshold),
            ),
            factor(
                "too_much_energy_threshold_kWh",
                format!("{:.2}", k.weathersoc_too_much_energy_threshold),
            ),
            factor(
                "forecast_disagreement_strategy",
                format!("{:?}", k.forecast_disagreement_strategy),
            ),
        ];
        for (name, p) in providers {
            let snap = world.typed_sensors.forecast(p);
            let value = match snap {
                None => "—".to_string(),
                Some(s) => format!("today={:.2} kWh, tomorrow={:.2} kWh", s.today_kwh, s.tomorrow_kwh),
            };
            out.push(factor(&format!("forecast_{name}"), value));
        }
        // PR-baseline-forecast: surface the locally-computed baseline
        // last so an operator can see why a fused number exists when all
        // three cloud rows above are "—". The fusion gate is exclusive:
        // baseline contributes ONLY when no cloud snapshot is fresh
        // (see `forecast_fusion::fused_today_kwh`). To make the panel
        // honest about that gate we tag the row "(unused — cloud
        // available)" whenever any cloud snapshot is present, even when
        // baseline itself has values. This avoids the operator
        // misreading a populated baseline row as "this is what
        // weather_soc actually saw".
        let any_cloud_present = world.typed_sensors.forecast_solcast.is_some()
            || world.typed_sensors.forecast_forecast_solar.is_some()
            || world.typed_sensors.forecast_open_meteo.is_some();
        let baseline_value = match world.typed_sensors.forecast(ForecastProvider::Baseline) {
            None => "—".to_string(),
            Some(s) => {
                let core = format!(
                    "today={:.2} kWh, tomorrow={:.2} kWh",
                    s.today_kwh, s.tomorrow_kwh,
                );
                if any_cloud_present {
                    format!("{core} (unused — cloud available)")
                } else {
                    core
                }
            }
        };
        out.push(factor("forecast_baseline", baseline_value));
        out.push(factor(
            "last_run_date",
            bk.last_weather_soc_run_date.map_or("—".to_string(), |d| format!("{d}")),
        ));
        out
    }

    /// We don't keep the last `WeatherSocDecision` around in `World`,
    /// so surface the four knob values the planner steers (which are
    /// the most recent values it (or the operator) wrote) plus the
    /// per-day boolean it stamps on `Bookkeeping`. This is the
    /// "lightweight recomputation" path described in the spec —
    /// approximate but honest.
    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let k = &world.knobs;
        let bk = &world.bookkeeping;
        vec![
            factor("export_soc_threshold", format!("{:.2}", k.export_soc_threshold)),
            factor("discharge_soc_target", format!("{:.2}", k.discharge_soc_target)),
            factor("battery_soc_target", format!("{:.2}", k.battery_soc_target)),
            factor(
                "disable_night_grid_discharge",
                format!("{}", k.disable_night_grid_discharge),
            ),
            factor(
                "charge_battery_extended_today",
                format!("{}", bk.charge_battery_extended_today),
            ),
        ]
    }
}

/// PR-keep-batteries-charged. Reads knobs + bookkeeping + sensors +
/// world.sunrise/sunset, evaluates the pure
/// [`evaluate_ess_state_override`] controller, writes the resulting
/// Decision to `world.decisions.ess_state_override`, and (when the
/// target differs from the proposed value) emits a
/// `WriteDbus(EssState, Int(target))` effect under `writes_enabled`.
///
/// Depends on `Setpoint` because that core writes
/// `bookkeeping.charge_to_full_required`, which gates the 9-vs-10
/// branch.
pub(crate) struct EssStateOverrideCore;
impl Core for EssStateOverrideCore {
    fn id(&self) -> CoreId {
        CoreId::EssStateOverride
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        &[DepEdge {
            from: CoreId::Setpoint,
            fields: &["bookkeeping.charge_to_full_required"],
        }]
    }
    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        let k = &world.knobs;
        let input = EssStateOverrideInput {
            knob_enabled: k.keep_batteries_charged_during_full_charge,
            offset_min: k.sunrise_sunset_offset_min,
            charge_to_full_required: world.bookkeeping.charge_to_full_required,
            sunrise_local: world.sunrise,
            sunset_local: world.sunset,
            sunrise_sunset_updated_at: world.sunrise_sunset_updated_at,
            freshness_threshold: SUNRISE_SUNSET_FRESHNESS,
            now_local: clock.naive(),
        };
        let out = evaluate_ess_state_override(&input);
        world.decisions.ess_state_override = Some(out.decision);
        let target = out.target;
        let now = clock.monotonic();
        let changed = world.ess_state_target.propose_target(
            target,
            crate::Owner::EssStateOverrideController,
            now,
        );
        if !changed {
            return;
        }
        effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
            id: crate::types::ActuatedId::EssStateTarget,
            phase: world.ess_state_target.target.phase,
        }));
        if !world.knobs.writes_enabled {
            #[allow(clippy::cast_possible_truncation)]
            let current_ess_state = if world.sensors.ess_state.is_usable() {
                world.sensors.ess_state.value.map(|v| v as i32)
            } else {
                None
            };
            effects.push(Effect::Log {
                level: LogLevel::Info,
                source: "observer",
                message: format!(
                    "EssState would be set to {target} (current={current_ess_state:?}); suppressed by writes_enabled=false"
                ),
            });
            return;
        }
        effects.push(Effect::WriteDbus {
            target: DbusTarget::EssState,
            value: DbusValue::Int(target),
        });
        world.ess_state_target.mark_commanded(now);
        effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
            id: crate::types::ActuatedId::EssStateTarget,
            phase: world.ess_state_target.target.phase,
        }));
    }

    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        let k = &world.knobs;
        #[allow(clippy::cast_possible_truncation)]
        let current = if world.sensors.ess_state.is_usable() {
            world
                .sensors
                .ess_state
                .value
                .map_or("—".to_string(), |v| (v as i32).to_string())
        } else {
            format!("— ({:?})", world.sensors.ess_state.freshness)
        };
        vec![
            factor(
                "knob_enabled",
                format!("{}", k.keep_batteries_charged_during_full_charge),
            ),
            factor("offset_min", format!("{}", k.sunrise_sunset_offset_min)),
            factor(
                "charge_to_full_required",
                format!("{}", world.bookkeeping.charge_to_full_required),
            ),
            factor("ess_state", current),
            factor(
                "sunrise_local",
                world
                    .sunrise
                    .map_or("—".to_string(), |t| t.to_string()),
            ),
            factor(
                "sunset_local",
                world
                    .sunset
                    .map_or("—".to_string(), |t| t.to_string()),
            ),
            factor(
                "sunrise_sunset_age_s",
                world.sunrise_sunset_updated_at.map_or(
                    "—".to_string(),
                    |at| match std::time::Instant::now().checked_duration_since(at) {
                        Some(d) => format!("{}", d.as_secs()),
                        None => "0".to_string(),
                    },
                ),
            ),
        ]
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let target = match world.decisions.ess_state_override.as_ref() {
            Some(d) => d
                .factors
                .iter()
                .find(|f| f.name == "target")
                .map_or("—".to_string(), |f| f.value.clone()),
            None => "—".to_string(),
        };
        vec![factor("target", target)]
    }
}

/// PR-LG-THINQ-B: proposes three of the four LG ThinQ actuated entities
/// every tick.
///
/// - `lg_dhw_power`: time-window schedule [02:00, 05:00) ∪ [07:00, 08:00).
/// - `lg_dhw_target_c`: constant 60 °C.
/// - `lg_heating_water_target_c`: outdoor-temperature curve — only proposed
///   when `world.sensors.outdoor_temperature` is Fresh.
///
/// `lg_heat_pump_power` (master on/off) is intentionally NOT proposed here;
/// it is operator-only and is mirrored by `apply_knob` in `process.rs`.
///
/// No `depends_on` edges — `evaluate_heat_pump` reads sensors and knobs only.
pub(crate) struct HeatPumpControlCore;
impl Core for HeatPumpControlCore {
    fn id(&self) -> CoreId {
        CoreId::HeatPumpControl
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        &[]
    }
    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        _topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        let now_mono = clock.monotonic();
        let now_local = clock.naive();
        let retry_threshold = Duration::from_secs(u64::from(world.knobs.actuator_retry_s));

        let out = evaluate_heat_pump(
            now_local,
            world.sensors.outdoor_temperature,
            world.lg_heat_pump_power.actual,
        );

        // PR-LG-THINQ-DECISIONS-1: surface the controller's decision
        // factors in the dashboard's Decisions section. Done first so
        // every tick the operator can see WHY the controller produced
        // its current proposals (time window, outdoor-temp bucket,
        // outdoor-sensor freshness).
        world.decisions.heat_pump = Some(out.decision.clone());

        // --- DHW power ---
        if let Some(dhw_power) = out.dhw_power {
            let changed = world
                .lg_dhw_power
                .propose_target(dhw_power, Owner::HeatPumpController, now_mono);
            // PR-LG-THINQ-B-1-D06: gate the propose-side phase publish
            // on `changed`. The write-path publish below stays
            // unconditional so a retry is still surfaced.
            if changed {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::LgDhwPower,
                    phase: world.lg_dhw_power.target.phase,
                }));
            }
            if !world.knobs.writes_enabled {
                if changed {
                    effects.push(Effect::Log {
                        level: LogLevel::Info,
                        source: "observer",
                        message: format!(
                            "lg_dhw_power would be set to {dhw_power}; suppressed by writes_enabled=false"
                        ),
                    });
                }
            } else if changed || world.lg_dhw_power.needs_actuation(now_mono, retry_threshold) {
                effects.push(Effect::CallLgThinq(LgThinqAction::SetDhwPower(dhw_power)));
                world.lg_dhw_power.mark_commanded(now_mono);
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::LgDhwPower,
                    phase: world.lg_dhw_power.target.phase,
                }));
            }
        }

        // --- DHW target temperature (constant 60 °C) ---
        if let Some(dhw_target_c) = out.dhw_target_c {
            let changed = world
                .lg_dhw_target_c
                .propose_target(dhw_target_c, Owner::HeatPumpController, now_mono);
            // PR-LG-THINQ-B-1-D06: gate the propose-side phase publish on `changed`.
            if changed {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::LgDhwTarget,
                    phase: world.lg_dhw_target_c.target.phase,
                }));
            }
            if !world.knobs.writes_enabled {
                if changed {
                    effects.push(Effect::Log {
                        level: LogLevel::Info,
                        source: "observer",
                        message: format!(
                            "lg_dhw_target_c would be set to {dhw_target_c}; suppressed by writes_enabled=false"
                        ),
                    });
                }
            } else if changed || world.lg_dhw_target_c.needs_actuation(now_mono, retry_threshold) {
                effects.push(Effect::CallLgThinq(LgThinqAction::SetDhwTargetC(
                    i64::from(dhw_target_c),
                )));
                world.lg_dhw_target_c.mark_commanded(now_mono);
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::LgDhwTarget,
                    phase: world.lg_dhw_target_c.target.phase,
                }));
            }
        }

        // --- Heating-water target temperature (outdoor-temp curve; None
        //     when outdoor temperature is not Fresh — controller skips) ---
        if let Some(hw_target_c) = out.heating_water_target_c {
            let changed = world
                .lg_heating_water_target_c
                .propose_target(hw_target_c, Owner::HeatPumpController, now_mono);
            // PR-LG-THINQ-B-1-D06: gate the propose-side phase publish on `changed`.
            if changed {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::LgHeatingWaterTarget,
                    phase: world.lg_heating_water_target_c.target.phase,
                }));
            }
            if !world.knobs.writes_enabled {
                if changed {
                    effects.push(Effect::Log {
                        level: LogLevel::Info,
                        source: "observer",
                        message: format!(
                            "lg_heating_water_target_c would be set to {hw_target_c}; suppressed by writes_enabled=false"
                        ),
                    });
                }
            } else if changed
                || world
                    .lg_heating_water_target_c
                    .needs_actuation(now_mono, retry_threshold)
            {
                effects.push(Effect::CallLgThinq(LgThinqAction::SetHeatingWaterTargetC(
                    i64::from(hw_target_c),
                )));
                world.lg_heating_water_target_c.mark_commanded(now_mono);
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::LgHeatingWaterTarget,
                    phase: world.lg_heating_water_target_c.target.phase,
                }));
            }
        }
    }

    fn last_inputs(&self, world: &World) -> Vec<CoreFactor> {
        let outdoor = fmt_actual_f64(world.sensors.outdoor_temperature);
        vec![
            factor("outdoor_temperature_C", outdoor),
        ]
    }

    fn last_outputs(&self, world: &World) -> Vec<CoreFactor> {
        let dhw_power = world
            .lg_dhw_power
            .target
            .value
            .map_or("—".to_string(), |v| v.to_string());
        let dhw_target = world
            .lg_dhw_target_c
            .target
            .value
            .map_or("—".to_string(), |v| format!("{v} °C"));
        let hw_target = world
            .lg_heating_water_target_c
            .target
            .value
            .map_or("—".to_string(), |v| format!("{v} °C"));
        vec![
            factor("lg_dhw_power", dhw_power),
            factor("lg_dhw_target_c", dhw_target),
            factor("lg_heating_water_target_c", hw_target),
        ]
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
    fn depends_on(&self) -> &'static [DepEdge] {
        // Pure ordering edges: the broadcast publishes sensors +
        // bookkeeping written by every actuator + derivation core, so
        // it must run after all of them. No specific field per edge —
        // this is the legitimate empty-fields case that `DepEdge`'s
        // doc-comment mentions.
        &[
            DepEdge { from: CoreId::ZappiActive, fields: &[] },
            DepEdge { from: CoreId::Setpoint, fields: &[] },
            DepEdge { from: CoreId::CurrentLimit, fields: &[] },
            DepEdge { from: CoreId::Schedules, fields: &[] },
            DepEdge { from: CoreId::ZappiMode, fields: &[] },
            DepEdge { from: CoreId::EddiMode, fields: &[] },
            DepEdge { from: CoreId::WeatherSoc, fields: &[] },
            DepEdge { from: CoreId::EssStateOverride, fields: &[] },
            DepEdge { from: CoreId::HeatPumpControl, fields: &[] },
        ]
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
        // PR-AS-C: skip actuated-mirror sensor variants. Their values
        // are surfaced via the dedicated `Actuated` table (published
        // through `PublishPayload::ActuatedPhase`); double-publishing
        // them as plain sensors would clutter HA. The
        // `actuated_id().is_some()` predicate is the single source of
        // truth — any future actuated-mirror SensorId that classifies
        // itself the same way is filtered automatically.
        for &id in SensorId::ALL {
            if id.actuated_id().is_some() {
                continue;
            }
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

        // ----- PR-ZDO-2: Controller-derived observables -----
        // Reads world.zappi_drain_state.latest (populated by run_setpoint and
        // apply_setpoint_safety in PR-ZDO-1). Per-id dedup against
        // world.published_cache.controller_{numeric,bool}.
        //
        // When latest.is_none(): numeric publishes "unavailable" (honest —
        // 0 W would be a real reading); booleans publish false (always-
        // meaningful, false is honest pre-first-tick).
        {
            let snap = world.zappi_drain_state.latest;

            // compensated-w: numeric, freshness-aware.
            // Disabled branch carries a placeholder 0.0 — treat it as Stale
            // (publishes "unavailable") so HA Recorder does not ingest fake zeros.
            let (drain_value, drain_freshness) = match snap {
                None => (0.0, Freshness::Stale),
                Some(s) if s.branch == ZappiDrainBranch::Disabled => (0.0, Freshness::Stale),
                Some(s) => (s.compensated_drain_w, Freshness::Fresh),
            };
            let drain_body = encode_sensor_body(Some(drain_value), drain_freshness);
            let drain_id = ControllerObservableId::ZappiDrainCompensatedW;
            let drain_changed = world
                .published_cache
                .controller_numeric
                .get(&drain_id)
                .is_none_or(|prev| prev != &drain_body);
            if drain_changed {
                world
                    .published_cache
                    .controller_numeric
                    .insert(drain_id, drain_body.clone());
                effects.push(Effect::Publish(PublishPayload::ControllerNumeric {
                    id: drain_id,
                    value: drain_value,
                    freshness: drain_freshness,
                }));
            }

            // tighten-active: bool, always-meaningful.
            let tighten =
                matches!(snap.map(|s| s.branch), Some(ZappiDrainBranch::Tighten));
            let tighten_id = ControllerObservableId::ZappiDrainTightenActive;
            let tighten_changed = world
                .published_cache
                .controller_bool
                .get(&tighten_id)
                .is_none_or(|prev| prev != &tighten);
            if tighten_changed {
                world
                    .published_cache
                    .controller_bool
                    .insert(tighten_id, tighten);
                effects.push(Effect::Publish(PublishPayload::ControllerBool {
                    id: tighten_id,
                    value: tighten,
                }));
            }

            // hard-clamp-active: bool.
            let clamp = snap.is_some_and(|s| s.hard_clamp_engaged);
            let clamp_id = ControllerObservableId::ZappiDrainHardClampActive;
            let clamp_changed = world
                .published_cache
                .controller_bool
                .get(&clamp_id)
                .is_none_or(|prev| prev != &clamp);
            if clamp_changed {
                world.published_cache.controller_bool.insert(clamp_id, clamp);
                effects.push(Effect::Publish(PublishPayload::ControllerBool {
                    id: clamp_id,
                    value: clamp,
                }));
            }
        }

        // ----- weather-SoC active cell -----
        // Mirrors `world.weather_soc_active` (set by run_weather_soc) as a
        // `<bucket>.<temp>` kebab token. None → "unavailable" body so HA
        // greys the entity out when the planner skipped (no fresh temp /
        // forecast) or hasn't run yet on cold boot. Dedup on encoded body.
        {
            let id = ControllerObservableId::WeathersocActiveCell;
            let value: Option<&'static str> = world
                .weather_soc_active
                .map(|(b, t)| crate::weather_soc_addr::active_cell_label(b, t));
            let body = value.unwrap_or("unavailable");
            let changed = world
                .published_cache
                .controller_enum
                .get(&id)
                .is_none_or(|prev| prev.as_str() != body);
            if changed {
                world
                    .published_cache
                    .controller_enum
                    .insert(id, body.to_string());
                effects.push(Effect::Publish(PublishPayload::ControllerEnumName {
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
        Box::new(EssStateOverrideCore),
        Box::new(HeatPumpControlCore),
        Box::new(SensorBroadcastCore),
    ]
}
