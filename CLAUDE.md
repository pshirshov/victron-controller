# Project notes for Claude

## Deployment topology — no baboon backward compat needed

Shell (Rust on the GX) and dashboard (TypeScript bundle served by the
same binary) deploy together as one unit. The baboon wire format has a
single client (the operator's browser); no third-party integrations
read it. So:

- Edit `models/dashboard.baboon` in place — additive or breaking, both
  fine. Run `scripts/regen-baboon.sh`. Fix compile errors in the shell.
- Don't spend effort hand-implementing
  `convert__<type>__from__0_X_0(...)` migration stubs; baboon
  auto-emits them with `todo!()` bodies and they're never called at
  runtime.
- Don't bump to a new version block for routine changes — extending the
  current version's `data` blocks is the path of least resistance.
- Reconsider if the project ever ships the dashboard separately or
  exposes the wire format to a third-party client.

## Adding a new knob — full registration checklist

When adding a knob, every layer below must be touched. Skipping one
produces silent failure modes (renders as "Other" with no controls,
silent retained-MQTT drops on out-of-range values, missing HA entity,
config-section field silently ignored, etc.). The order below is the
order I recommend doing it in; each step is a hard requirement.

1. **Baboon model** — `models/dashboard.baboon`. Add the field to the
   `Knobs` block. Run `scripts/regen-baboon.sh` to regenerate
   `crates/dashboard-model/` and `web/src/model/`. Additive at the end
   is fine within an existing version (single client, no wire-format
   migration needed).
2. **Core knobs struct** — `crates/core/src/knobs.rs`: add the field +
   default in `safe_defaults()`. Update the `safe_defaults_match_spec_7`
   test if the value is non-trivial.
3. **Core enum** — `crates/core/src/types.rs`: add the matching
   `KnobId::*` variant.
4. **Core apply_knob** — `crates/core/src/process.rs`: add the
   `(KnobId, KnobValue)` arm so retained-MQTT writes land on the field.
5. **Shell MQTT serialize** — `crates/shell/src/mqtt/serialize.rs`,
   *four* sites: `knob_name` (KnobId → dotted), `knob_id_from_name`
   (dotted → KnobId), `knob_range` (numeric ranges; bool/enum knobs
   take the `return None` branch), `parse_knob_value` (bool/float/u32
   shape).
6. **Shell HA discovery** — `crates/shell/src/mqtt/discovery.rs`
   `knob_schemas`: add a `switch` (bool), `select` (enum), or
   `number_knob` (numeric with step + unit) entry.
7. **Shell config defaults** — `crates/shell/src/config.rs`
   `KnobsDefaultsConfig`: add `Option<…>` field + `set!(…)` line in
   `apply_to`. This is what lets the operator seed a non-default at
   boot via `[knobs]` in `config.toml`. Document it in
   `config.example.toml` (commented out at the safe-default value).
8. **Shell dashboard convert** — `crates/shell/src/dashboard/convert.rs`
   *two* sites: `knobs_to_model` (Knobs → wire model field), and
   `knob_id_from_name` (the dashboard's own snake_case → KnobId map,
   parallel to the MQTT-layer one).
9. **Web display-name table** — `web/src/displayNames.ts` `DISPLAY_NAMES`:
   add `<snake_case>: "<dotted-name>"`. **Without this entry the
   dashboard falls through to the "Other" group with no controls** —
   the snake_case canonical from `snap.knobs` never resolves to the
   dotted KNOB_SPEC key. This is the easiest step to forget.
10. **Web KNOB_SPEC** — `web/src/knobs.ts`: add an entry keyed by the
    dotted name (matching step 9) with `kind`, range, `default`,
    `category` (`"operator"` for daily-use knobs, `"config"` for
    install-time), `group` (visual section in the table).
11. **Web descriptions** — `web/src/descriptions.ts`: add the dotted
    name → human-readable explanation. Surfaces in the entity inspector
    popup.

After all layers: `cargo test --workspace`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cd web && ./node_modules/.bin/tsc
--noEmit -p .`, then reload the dashboard and confirm the knob shows
in the right group with working controls.

## Adding a new actuator (DBus write target)

**Default to full TASS Actuated tracking, not bare `Effect::WriteDbus`.**
The dashboard's actuated table and "scheduled actions" surface are
load-bearing operator UX — an actuator written via direct WriteDbus
without an `Actuated<T>` slot is invisible there, which is a UX
regression even when the underlying actuation works. The ESS-state
override learned this the hard way (was originally direct WriteDbus,
later promoted to full Actuated).

The full registration:

1. **Baboon model** — `models/dashboard.baboon`: extend `data Actuated`
   with the new entry (`ActuatedI32` / `ActuatedF64` / `ActuatedSchedule`
   / `ActuatedEnumName`). Run `scripts/regen-baboon.sh`.
2. **DbusTarget** — `crates/core/src/types.rs`: add `DbusTarget::*`
   variant + `ActuatedId::*` variant.
3. **Owner** — if the actuator is driven by a new controller, add
   `Owner::*` in `crates/core/src/owner.rs` AND mirror it in
   `models/dashboard.baboon` `enum Owner` (override the import via
   `without { Owner }` and redefine).
4. **Writer** — `crates/shell/src/dbus/writer.rs` `resolve()`: map the
   `DbusTarget` to `(service, path)`. Add to
   `resolve_covers_every_target` test.
5. **World slot** — `crates/core/src/world.rs`: add
   `pub <name>: Actuated<T>` field + `Actuated::new(now)` in
   `fresh_boot`.
6. **Controller core** — emit `propose_target` + `mark_commanded` (gated
   on `writes_enabled`), follow `set_grid_setpoint` in `process.rs` as
   the canonical template. Always publish `ActuatedPhase` after both
   `propose_target` (so observer mode shows intent) and
   `mark_commanded` (so live mode shows Commanded).
7. **Readback hook** — `crates/core/src/process.rs` `apply_sensor_reading`:
   feed the readback sensor into `world.<entity>.actual.on_reading +
   confirm_if` with an appropriate equality predicate (exact for
   discrete states, tolerance for floats). Either via the post-hook
   `actuated_id()` route (when there's a dedicated `*Actual` SensorId)
   or directly inside the primary sensor's match arm (when the sensor
   doubles as readback for an actuator).
8. **Dashboard convert** — `crates/shell/src/dashboard/convert.rs`:
   add to `WorldActuatedRefs`, `world_actuated`, and the `actuated:`
   block of the snapshot. Add the `Owner` mapping if step 3 applied.
9. **MQTT actuated_name** — `crates/shell/src/mqtt/serialize.rs`
   `actuated_name`: dotted name for the new `ActuatedId`.
10. **Scheduled actions** — if the actuator fires on a predictable
    cadence (sunrise/sunset, time-of-day), add an entry-emitter to
    `crates/shell/src/dashboard/convert_schedule.rs` so the dashboard's
    "Scheduled actions" section surfaces the next fire. Otherwise the
    operator has no forward-looking visibility.

The "no Scheduled-actions row" + "no Actuated row" combination is the
exact symptom that made the user notice the under-registered ESS-state
actuator. If you can answer "when will this fire?" and "what's its
phase right now?" from the spec, both surfaces should show it.

## Coordinates

`[location]` (`crates/shell/src/config.rs` `LocationConfig`) is the
single source of truth for site latitude/longitude. Every coord-driven
scheduler (Forecast.Solar, Open-Meteo, Open-Meteo current-weather,
baseline forecast, sunrise/sunset) reads from `cfg.location.*`. Per-
provider sections own their own `cadence` / `enabled` / `planes` /
`system_efficiency` etc. but **not** lat/lon. When adding a new
coord-consuming scheduler, gate its spawn on
`cfg.location.is_configured()` and read coords from there.
