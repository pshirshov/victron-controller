//! PR-schedule-section: build the dashboard's `ScheduledActions` wire
//! payload from the live `World`.
//!
//! Forward-looking controller actions sorted by `next_fire_epoch_ms`.
//! Sources for v1:
//!
//! - **Eddi tariff windows** — four daily edges from
//!   `eddi_schedule_override` (02:00 → Normal, 05:00 → Stopped,
//!   07:00 → Normal, 08:00 → Stopped).
//! - **`schedule_0` / `schedule_1`** — when `days == DAYS_ENABLED (7)`,
//!   surfaced as one daily entry per slot.
//! - **`next_full_charge`** — one-shot at the bookkeeping field's value.
//! - **`weather_soc` planner** — daily 01:55 re-evaluation.
//! - **`zappi.mode`** — three daily edges (02:00, 05:00, 08:00) whose
//!   labels reflect the current `charge_car_boost` /
//!   `effective_charge_car_extended` state.
//!
//! Pure compute: callable from tests with a hand-built `World` and a
//! fixed `now_ms`. TZ discipline mirrors `convert_soc_chart` —
//! `world.timezone` parsed with `Tz::from_str`, UTC fallback, DST
//! ambiguity resolved by picking the early mapping, DST-skip dates
//! dropped.

use std::str::FromStr;

use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

use victron_controller_core::controllers::schedules::{DAYS_ENABLED, ScheduleSpec};
use victron_controller_core::process::effective_charge_car_extended;
use victron_controller_core::tass::Actuated;
use victron_controller_core::world::World;

use victron_controller_dashboard_model::victron_controller::dashboard::scheduled_action::ScheduledAction as WireAction;
use victron_controller_dashboard_model::victron_controller::dashboard::scheduled_actions::ScheduledActions as WireActions;

const DAY_MS: i64 = 86_400_000;

/// The four eddi tariff edges (hour-of-day, "→ <mode>" label fragment).
/// Matches `eddi_schedule_override`: 02:00 starts Normal (boost), 05:00
/// starts Stopped (rest), 07:00 starts Normal (last cheap-rate hour),
/// 08:00 starts Stopped (day rate).
const EDDI_EDGES: &[(u32, &str)] = &[
    (2, "→ Normal"),
    (5, "→ Stopped"),
    (7, "→ Normal"),
    (8, "→ Stopped"),
];

/// Build the `ScheduledActions` wire payload sorted by `next_fire_epoch_ms`.
#[must_use]
pub fn compute_scheduled_actions(world: &World, now_ms: i64) -> WireActions {
    let tz = Tz::from_str(&world.timezone).unwrap_or(chrono_tz::UTC);
    let now_local: DateTime<Tz> = match Utc.timestamp_millis_opt(now_ms).single() {
        Some(dt) => dt.with_timezone(&tz),
        None => return WireActions { entries: Vec::new() },
    };

    let mut entries: Vec<WireAction> = Vec::new();
    entries.extend(eddi_tariff_actions(now_local));
    entries.extend(schedule_actions(world, now_local));
    entries.extend(next_full_charge_action(world, tz, now_ms));
    entries.extend(weather_soc_action(now_local));
    entries.extend(zappi_actions(world, now_local));
    entries.extend(keep_batteries_charged_actions(world, tz, now_ms));

    entries.sort_by_key(|e| e.next_fire_epoch_ms);
    WireActions { entries }
}

/// Surface the daytime ESS-state-9 override window edges. Emitted only
/// when the operator knob is on AND today is a full-charge day
/// (`bookkeeping.charge_to_full_required`) AND `world.sunrise` /
/// `world.sunset` are present. Skipping the entries on non-full-charge
/// days matches the controller's actual behaviour: outside a
/// full-charge day, the controller writes 10 every tick — the window
/// edges are no-ops, so surfacing them as "scheduled actions" misleads
/// the operator about what's coming.
fn keep_batteries_charged_actions(
    world: &World,
    tz: Tz,
    now_ms: i64,
) -> Vec<WireAction> {
    if !world.knobs.keep_batteries_charged_during_full_charge {
        return Vec::new();
    }
    if !world.bookkeeping.charge_to_full_required {
        return Vec::new();
    }
    let (Some(sunrise_local), Some(sunset_local)) = (world.sunrise, world.sunset) else {
        return Vec::new();
    };
    let offset = ChronoDuration::minutes(i64::from(world.knobs.sunrise_sunset_offset_min));
    let open = sunrise_local + offset;
    let close = sunset_local - offset;
    if close <= open {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut emit = |label: String, source: String, local: chrono::NaiveDateTime| {
        let dt = match tz.from_local_datetime(&local) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(early, _late) => early,
            chrono::LocalResult::None => return,
        };
        let mut next_fire_epoch_ms = dt.timestamp_millis();
        // Walk forward to tomorrow if today's edge has already passed —
        // sunrise/sunset shift slightly day-to-day; using +1 day as an
        // approximation is fine for the dashboard surface (the
        // sunrise/sunset scheduler reseeds every 15 min so the figure
        // stays close).
        if next_fire_epoch_ms < now_ms {
            next_fire_epoch_ms += DAY_MS;
        }
        out.push(WireAction {
            label,
            source,
            next_fire_epoch_ms,
            period_ms: Some(DAY_MS),
        });
    };
    emit(
        format!(
            "ESS → KeepBatteriesCharged ({:02}:{:02})",
            open.time().hour(),
            open.time().minute()
        ),
        "ess.state.override.open".to_string(),
        open,
    );
    emit(
        format!(
            "ESS → Optimized ({:02}:{:02})",
            close.time().hour(),
            close.time().minute()
        ),
        "ess.state.override.close".to_string(),
        close,
    );
    out
}

/// Resolve a local-clock `(date, h, m)` to its UTC epoch-ms in `tz`,
/// disambiguating DST as `convert_soc_chart::expand_schedule_windows`
/// does (Single → use; Ambiguous → pick early; None → skip).
fn local_hm_to_epoch_ms(tz: Tz, date: NaiveDate, h: u32, m: u32) -> Option<i64> {
    let local = date.and_hms_opt(h, m, 0)?;
    let dt = match tz.from_local_datetime(&local) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(early, _late) => early,
        chrono::LocalResult::None => return None,
    };
    Some(dt.timestamp_millis())
}

/// Find the next epoch-ms at which local `HH:MM` happens at-or-after
/// `now_local`. Walks forward day-by-day; skips DST-non-existent dates.
/// Bounded to 7 days to cover any realistic gap (DST happens once per
/// year in practice).
fn next_local_hm(now_local: DateTime<Tz>, h: u32, m: u32) -> Option<i64> {
    let tz = now_local.timezone();
    let now_ms = now_local.timestamp_millis();
    let base_date = now_local.date_naive();
    for day_offset in 0..=7 {
        let date = base_date + ChronoDuration::days(day_offset);
        let Some(candidate_ms) = local_hm_to_epoch_ms(tz, date, h, m) else {
            continue;
        };
        if candidate_ms >= now_ms {
            return Some(candidate_ms);
        }
    }
    None
}

/// Eddi tariff edges — one entry per edge, period = 1 day.
fn eddi_tariff_actions(now_local: DateTime<Tz>) -> Vec<WireAction> {
    EDDI_EDGES
        .iter()
        .filter_map(|(hour, suffix)| {
            let next_fire_epoch_ms = next_local_hm(now_local, *hour, 0)?;
            Some(WireAction {
                label: format!("Eddi {suffix}"),
                source: "eddi.tariff".to_string(),
                next_fire_epoch_ms,
                period_ms: Some(DAY_MS),
            })
        })
        .collect()
}

/// Zappi mode edges — three predictable daily transitions (02:00 boost
/// start, 05:00 NightExtended start, 08:00 day-rate stop). Labels reflect
/// the current knob state so the dashboard surfaces what *will* happen.
fn zappi_actions(world: &World, now_local: DateTime<Tz>) -> Vec<WireAction> {
    let edges: [(u32, String); 3] = [
        (
            2,
            format!(
                "Zappi 02:00 → {}",
                if world.knobs.charge_car_boost { "Fast" } else { "Off" }
            ),
        ),
        (
            5,
            format!(
                "Zappi 05:00 → {}",
                if effective_charge_car_extended(world) { "Fast" } else { "Off" }
            ),
        ),
        (8, "Zappi 08:00 → Off".to_string()),
    ];
    edges
        .into_iter()
        .filter_map(|(hour, label)| {
            let next_fire_epoch_ms = next_local_hm(now_local, hour, 0)?;
            Some(WireAction {
                label,
                source: "zappi.mode".to_string(),
                next_fire_epoch_ms,
                period_ms: Some(DAY_MS),
            })
        })
        .collect()
}

/// ESS schedules 0 and 1 — one entry per enabled (`days == 7`) slot.
fn schedule_actions(world: &World, now_local: DateTime<Tz>) -> Vec<WireAction> {
    let mut out = Vec::new();
    for (idx, slot) in [(0_usize, &world.schedule_0), (1_usize, &world.schedule_1)] {
        if let Some(action) = schedule_action_for_slot(idx, slot, now_local) {
            out.push(action);
        }
    }
    out
}

fn schedule_action_for_slot(
    idx: usize,
    slot: &Actuated<ScheduleSpec>,
    now_local: DateTime<Tz>,
) -> Option<WireAction> {
    let spec = slot.target.value.or(slot.actual.value)?;
    if spec.days != DAYS_ENABLED {
        return None;
    }
    if spec.duration_s <= 0 {
        return None;
    }
    let (start_h, start_m) = secs_to_hm(spec.start_s);
    let next_fire_epoch_ms = next_local_hm(now_local, start_h, start_m)?;
    let (end_h, end_m) = secs_to_hm(spec.start_s.saturating_add(spec.duration_s));
    let label = format!(
        "Schedule {idx}: {:02}:{:02}–{:02}:{:02} soc={}%",
        start_h, start_m, end_h, end_m, spec.soc as i32,
    );
    Some(WireAction {
        label,
        source: format!("schedule.{idx}"),
        next_fire_epoch_ms,
        period_ms: Some(DAY_MS),
    })
}

/// `next_full_charge` is a `NaiveDateTime` — interpret it as local
/// (matches `setpoint`'s `clock.naive()` post-PR-tz-from-victron).
/// One-shot: emit only when in the future.
fn next_full_charge_action(world: &World, tz: Tz, now_ms: i64) -> Option<WireAction> {
    let nfc = world.bookkeeping.next_full_charge?;
    let dt = match tz.from_local_datetime(&nfc) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(early, _late) => early,
        chrono::LocalResult::None => return None,
    };
    let next_fire_epoch_ms = dt.timestamp_millis();
    if next_fire_epoch_ms < now_ms {
        return None;
    }
    let day = match dt.weekday() {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    };
    let label = format!("Full charge: {} {:02}:{:02}", day, dt.hour(), dt.minute());
    Some(WireAction {
        label,
        source: "next_full_charge".to_string(),
        next_fire_epoch_ms,
        period_ms: None,
    })
}

/// Weather-SoC planner re-evaluates daily at 01:55 local.
fn weather_soc_action(now_local: DateTime<Tz>) -> Vec<WireAction> {
    let Some(next_fire_epoch_ms) = next_local_hm(now_local, 1, 55) else {
        return Vec::new();
    };
    vec![WireAction {
        label: "Weather-SoC re-evaluate".to_string(),
        source: "weather_soc".to_string(),
        next_fire_epoch_ms,
        period_ms: Some(DAY_MS),
    }]
}

fn secs_to_hm(secs: i32) -> (u32, u32) {
    let secs = secs.max(0);
    let total_minutes = (secs / 60) as u32;
    // Wrap at 24h so a duration that pushes an end past midnight still
    // displays as a wall-clock HH:MM rather than 25:00 / 26:00.
    let total_minutes = total_minutes % (24 * 60);
    (total_minutes / 60, total_minutes % 60)
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use victron_controller_core::Owner;
    use victron_controller_core::controllers::schedules::{DAYS_DISABLED, DAYS_ENABLED};
    use victron_controller_core::tass::TargetPhase;

    /// Build a `now_ms` corresponding to local-time `YYYY-MM-DD HH:MM:00`
    /// in the given TZ. Picks the early mapping on DST ambiguity.
    fn local_to_epoch_ms(tz: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
        let local = NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, 0)
            .unwrap();
        let dt = match tz.from_local_datetime(&local) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(early, _late) => early,
            chrono::LocalResult::None => panic!("test fixture used a DST-non-existent local time"),
        };
        dt.timestamp_millis()
    }

    fn dt_local(tz: Tz, now_ms: i64) -> DateTime<Tz> {
        Utc.timestamp_millis_opt(now_ms)
            .single()
            .unwrap()
            .with_timezone(&tz)
    }

    fn world_with_tz(tz: &str) -> World {
        let mut w = World::fresh_boot(Instant::now());
        w.timezone = tz.to_string();
        w
    }

    fn install_schedule(w: &mut World, slot: usize, spec: ScheduleSpec) {
        let target = match slot {
            0 => &mut w.schedule_0,
            1 => &mut w.schedule_1,
            _ => panic!("invalid slot"),
        };
        target.target.value = Some(spec);
        target.target.owner = Owner::ScheduleController;
        target.target.phase = TargetPhase::Confirmed;
    }

    // ----- eddi tariff edges -----------------------------------------

    #[test]
    fn eddi_tariff_actions_emits_four_entries_per_day() {
        let tz = chrono_tz::UTC;
        // Noon UTC on a non-DST day.
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = eddi_tariff_actions(now_local);
        assert_eq!(actions.len(), 4, "expected 4 eddi edges, got {actions:?}");
        // All within the next 24 h.
        for a in &actions {
            let dt = a.next_fire_epoch_ms - now_ms;
            assert!(
                dt > 0 && dt <= DAY_MS,
                "next_fire {} not in (now, now+24h]",
                a.next_fire_epoch_ms
            );
            assert_eq!(a.period_ms, Some(DAY_MS));
            assert_eq!(a.source, "eddi.tariff");
        }
    }

    #[test]
    fn eddi_tariff_actions_today_vs_tomorrow_boundary() {
        let tz = chrono_tz::UTC;
        // 06:00 — boost edge (02:00) and last cheap-rate boost edge
        // (07:00) bracket "now" differently.
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 6, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = eddi_tariff_actions(now_local);
        // Build a label-keyed lookup. Multiple "→ Normal" entries exist
        // (02:00 boost, 07:00 last-cheap), but 07:00 must come first
        // chronologically since it's only one hour away.
        let next_normal = actions
            .iter()
            .filter(|a| a.label == "Eddi → Normal")
            .min_by_key(|a| a.next_fire_epoch_ms)
            .unwrap();
        // 07:00 today.
        assert_eq!(
            next_normal.next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 4, 26, 7, 0)
        );
        let next_stopped = actions
            .iter()
            .filter(|a| a.label == "Eddi → Stopped")
            .min_by_key(|a| a.next_fire_epoch_ms)
            .unwrap();
        // 08:00 today.
        assert_eq!(
            next_stopped.next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 4, 26, 8, 0)
        );
        // The 02:00-Normal boost edge must roll to tomorrow.
        let later_normal = actions
            .iter()
            .filter(|a| a.label == "Eddi → Normal")
            .max_by_key(|a| a.next_fire_epoch_ms)
            .unwrap();
        assert_eq!(
            later_normal.next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 4, 27, 2, 0)
        );
    }

    #[test]
    fn eddi_tariff_actions_dst_spring_forward_skips_day() {
        // Europe/London 2026-03-29: clocks jump from 01:00 → 02:00 BST.
        // → 02:00 local on 2026-03-29 doesn't exist. The 02:00 edge
        // must roll to 2026-03-30.
        let tz: Tz = chrono_tz::Europe::London;
        // "now" sits inside the missing hour conceptually — pick 02:30
        // local on the day before so we definitely sit before the
        // missing 02:00 edge.
        // Use 2026-03-29 03:00 local (post-jump) as "now".
        let now_ms = local_to_epoch_ms(tz, 2026, 3, 29, 3, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = eddi_tariff_actions(now_local);
        // Pick the next "→ Normal" boost (02:00). It must NOT be the
        // 02:00 of today (which doesn't exist) — must roll to 03-30.
        let next_normal = actions
            .iter()
            .filter(|a| a.label == "Eddi → Normal")
            .min_by_key(|a| a.next_fire_epoch_ms)
            .unwrap();
        // 07:00 today is the closer "Normal" edge (it exists).
        assert_eq!(
            next_normal.next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 3, 29, 7, 0),
            "next Normal should be 07:00 today (the 02:00 edge would have been earlier but can't fire on a skipped local time)"
        );
        // The boost-window edge (the *other* Normal — 02:00) must
        // appear in the list and be on 03-30 (the next day where 02:00
        // exists).
        let later_normal = actions
            .iter()
            .filter(|a| a.label == "Eddi → Normal")
            .max_by_key(|a| a.next_fire_epoch_ms)
            .unwrap();
        assert_eq!(
            later_normal.next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 3, 30, 2, 0),
            "02:00 edge must roll to 03-30 since 03-29 02:00 doesn't exist"
        );
    }

    // ----- schedules --------------------------------------------------

    #[test]
    fn schedule_actions_skips_disabled() {
        let tz = chrono_tz::UTC;
        let mut w = world_with_tz("Etc/UTC");
        install_schedule(
            &mut w,
            0,
            ScheduleSpec {
                start_s: 2 * 3600,
                duration_s: 3 * 3600,
                discharge: 0,
                soc: 80.0,
                days: DAYS_DISABLED,
            },
        );
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = schedule_actions(&w, now_local);
        assert!(
            actions.is_empty(),
            "disabled schedule should not produce an entry: {actions:?}"
        );
    }

    #[test]
    fn schedule_actions_emits_enabled() {
        let tz = chrono_tz::UTC;
        let mut w = world_with_tz("Etc/UTC");
        install_schedule(
            &mut w,
            0,
            ScheduleSpec {
                start_s: 2 * 3600,
                duration_s: 3 * 3600,
                discharge: 0,
                soc: 80.0,
                days: DAYS_ENABLED,
            },
        );
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = schedule_actions(&w, now_local);
        assert_eq!(actions.len(), 1, "expected one enabled schedule, got {actions:?}");
        let a = &actions[0];
        assert_eq!(a.source, "schedule.0");
        assert_eq!(a.label, "Schedule 0: 02:00–05:00 soc=80%");
        assert_eq!(a.period_ms, Some(DAY_MS));
        // Next fire is 02:00 tomorrow (we're past today's 02:00 at noon).
        assert_eq!(
            a.next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 4, 27, 2, 0)
        );
    }

    // ----- next_full_charge ------------------------------------------

    #[test]
    fn next_full_charge_skips_when_past() {
        let tz = chrono_tz::UTC;
        let mut w = world_with_tz("Etc/UTC");
        // Past local time.
        w.bookkeeping.next_full_charge = Some(
            NaiveDate::from_ymd_opt(2026, 4, 25)
                .unwrap()
                .and_hms_opt(17, 0, 0)
                .unwrap(),
        );
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let action = next_full_charge_action(&w, tz, now_ms);
        assert!(action.is_none(), "past next_full_charge must not surface");
    }

    #[test]
    fn next_full_charge_emits_when_future() {
        let tz = chrono_tz::UTC;
        let mut w = world_with_tz("Etc/UTC");
        let nfc_local = NaiveDate::from_ymd_opt(2026, 4, 28)
            .unwrap()
            .and_hms_opt(17, 0, 0)
            .unwrap();
        w.bookkeeping.next_full_charge = Some(nfc_local);
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let action = next_full_charge_action(&w, tz, now_ms).expect("should emit");
        assert_eq!(action.source, "next_full_charge");
        assert_eq!(action.period_ms, None);
        let expected_ms = local_to_epoch_ms(tz, 2026, 4, 28, 17, 0);
        assert_eq!(action.next_fire_epoch_ms, expected_ms);
        assert!(
            action.label.contains("17:00"),
            "expected label to contain 17:00, got {}",
            action.label
        );
    }

    // ----- weather_soc -----------------------------------------------

    #[test]
    fn weather_soc_emits_daily_0155() {
        let tz = chrono_tz::UTC;
        // Before today's 01:55 → should fire today.
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 1, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = weather_soc_action(now_local);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].source, "weather_soc");
        assert_eq!(actions[0].period_ms, Some(DAY_MS));
        assert_eq!(
            actions[0].next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 4, 26, 1, 55)
        );

        // After today's 01:55 → should fire tomorrow.
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let actions = weather_soc_action(now_local);
        assert_eq!(
            actions[0].next_fire_epoch_ms,
            local_to_epoch_ms(tz, 2026, 4, 27, 1, 55)
        );
    }

    // ----- top-level sort --------------------------------------------

    #[test]
    fn compute_scheduled_actions_sorts_ascending() {
        let tz = chrono_tz::UTC;
        let mut w = world_with_tz("Etc/UTC");
        install_schedule(
            &mut w,
            0,
            ScheduleSpec {
                start_s: 2 * 3600,
                duration_s: 3 * 3600,
                discharge: 0,
                soc: 80.0,
                days: DAYS_ENABLED,
            },
        );
        install_schedule(
            &mut w,
            1,
            ScheduleSpec {
                start_s: 5 * 3600,
                duration_s: 3 * 3600,
                discharge: 0,
                soc: 90.0,
                days: DAYS_ENABLED,
            },
        );
        // next_full_charge a few days out so it slots near the end.
        w.bookkeeping.next_full_charge = Some(
            NaiveDate::from_ymd_opt(2026, 4, 28)
                .unwrap()
                .and_hms_opt(17, 0, 0)
                .unwrap(),
        );
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let actions = compute_scheduled_actions(&w, now_ms);
        assert!(!actions.entries.is_empty(), "expected entries");
        // Sorted ascending.
        for w in actions.entries.windows(2) {
            assert!(
                w[0].next_fire_epoch_ms <= w[1].next_fire_epoch_ms,
                "entries not sorted ascending: {:?}",
                actions.entries
            );
        }
        // Ensure all four sources are present.
        let sources: std::collections::HashSet<&str> =
            actions.entries.iter().map(|a| a.source.as_str()).collect();
        assert!(sources.contains("eddi.tariff"), "missing eddi.tariff in {sources:?}");
        assert!(sources.contains("schedule.0"));
        assert!(sources.contains("schedule.1"));
        assert!(sources.contains("next_full_charge"));
        assert!(sources.contains("weather_soc"));
        assert!(sources.contains("zappi.mode"));
    }

    // ----- zappi mode edges ------------------------------------------

    #[test]
    fn zappi_actions_emits_three_daily_edges() {
        use victron_controller_core::knobs::ExtendedChargeMode;
        let tz = chrono_tz::UTC;
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let mut w = world_with_tz("Etc/UTC");
        // `Knobs::safe_defaults` sets boost=true and extended_mode=Auto;
        // pin both to the all-Off branches for a deterministic label
        // assertion.
        w.knobs.charge_car_boost = false;
        w.knobs.charge_car_extended_mode = ExtendedChargeMode::Disabled;
        let actions = zappi_actions(&w, now_local);
        assert_eq!(actions.len(), 3, "expected 3 zappi edges, got {actions:?}");
        for a in &actions {
            assert_eq!(a.source, "zappi.mode");
            assert_eq!(a.period_ms, Some(DAY_MS));
            let dt = a.next_fire_epoch_ms - now_ms;
            assert!(
                dt > 0 && dt <= DAY_MS,
                "next_fire {} not in (now, now+24h]",
                a.next_fire_epoch_ms
            );
        }
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert!(labels.contains(&"Zappi 02:00 → Off"), "labels: {labels:?}");
        assert!(labels.contains(&"Zappi 05:00 → Off"), "labels: {labels:?}");
        assert!(labels.contains(&"Zappi 08:00 → Off"), "labels: {labels:?}");
    }

    #[test]
    fn zappi_actions_label_reflects_knob_state() {
        use victron_controller_core::knobs::ExtendedChargeMode;
        let tz = chrono_tz::UTC;
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let mut w = world_with_tz("Etc/UTC");
        w.knobs.charge_car_boost = true;
        w.knobs.charge_car_extended_mode = ExtendedChargeMode::Forced;
        let actions = zappi_actions(&w, now_local);
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert!(labels.contains(&"Zappi 02:00 → Fast"), "labels: {labels:?}");
        assert!(labels.contains(&"Zappi 05:00 → Fast"), "labels: {labels:?}");
        assert!(labels.contains(&"Zappi 08:00 → Off"), "labels: {labels:?}");
    }

    /// `ExtendedChargeMode::Auto` is the production default: the 05:00
    /// label must track `bookkeeping.auto_extended_today` (the latch the
    /// daily 04:30 evaluator writes). Pinning `Disabled` / `Forced`
    /// short-circuits this path, so it needs its own coverage.
    #[test]
    fn zappi_actions_label_auto_mode_tracks_bookkeeping() {
        use victron_controller_core::knobs::ExtendedChargeMode;
        let tz = chrono_tz::UTC;
        let now_ms = local_to_epoch_ms(tz, 2026, 4, 26, 12, 0);
        let now_local = dt_local(tz, now_ms);
        let mut w = world_with_tz("Etc/UTC");
        w.knobs.charge_car_extended_mode = ExtendedChargeMode::Auto;

        w.bookkeeping.auto_extended_today = true;
        let actions = zappi_actions(&w, now_local);
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert!(
            labels.contains(&"Zappi 05:00 → Fast"),
            "Auto + auto_extended_today=true should yield Fast at 05:00; labels: {labels:?}"
        );

        w.bookkeeping.auto_extended_today = false;
        let actions = zappi_actions(&w, now_local);
        let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
        assert!(
            labels.contains(&"Zappi 05:00 → Off"),
            "Auto + auto_extended_today=false should yield Off at 05:00; labels: {labels:?}"
        );
    }
}
