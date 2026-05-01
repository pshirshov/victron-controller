# M-EDDI-SENSORS — Eddi parser fix + typed-sensor surfacing + raw-response capture

## Origin

Field observation 2026-05-01: dashboard reports `eddi.mode.target =
Stopped, EddiController, Confirmed, actual=Stopped` while HASS's separate
myenergi integration shows the Eddi in `Normal` and the device is
physically diverting. Two distinct issues fall out:

1. **Parser bug**: `parse_eddi` (`crates/shell/src/myenergi/types.rs:147`)
   maps only `sta=1` to `Normal`, every other status to `Stopped`. The
   myenergi `sta` field on Eddi is the *operational status*, not the
   *mode*. Documented values:

   - `0` / `6` — Stopped
   - `1` — Paused (mode is Normal, no surplus right now)
   - `3` — Diverting (mode is Normal, dumping power)
   - `4` — Boost (mode is Normal, manual / scheduled boost)
   - `5` — Hot / max-temp reached (mode is Normal)

   So a Diverting Eddi (sta=3) is reported by us as `Stopped`, the
   `confirm_if(target==actual)` predicate then matches the controller's
   safety-direction `Stopped` target, and the actuated row falsely
   surfaces as `Confirmed Stopped`. Heuristic-from-docs fix: invert the
   mapping (`sta ∈ {0, 6}` → Stopped, everything else → Normal).

2. **Visibility gap**: `world.typed_sensors.eddi_mode` and
   `world.typed_sensors.zappi_state` are non-`Actual<f64>` readings the
   controllers consume (EddiController reads `current_mode`, ZappiController
   reads zmo/sta/pst), but the dashboard's `#sensors-table` only enumerates
   the f64 scalar block. The actuated table's "actual" column is the only
   surface — and it's contaminated by the parser bug above. A separate
   sensor row shows what the device *reports*, independent of what the
   controller *wants*.

3. **Diagnostic feedback**: with no captured raw API body, parser-mapping
   bugs are hard to verify against reality. In-memory retention of the
   last `cgi-jstatus-{Z,E}` body, surfaced in the entity inspector popup
   on the relevant sensor row, lets the operator click a sensor name and
   see the exact JSON that produced the displayed value.

## Scope

Scoped per user direction in the conversation:

- Forecasts are out of scope for the sensors-list extension (they have
  their own dashboard section already).
- DBus values are out of scope for the raw-capture work — only "values
  which we read from the live systems and require parsing" need raw
  retention. That narrows the raw payload to the two JSON-shaped reads:
  zappi + eddi.
- Storage is in-memory only. Last value only. Cleared on poll failure.
- Single client (operator's browser); no baboon backward-compat work.
  Edit `models/dashboard.baboon` in place; regen; fix compile errors.

## Acceptance criteria

1. `parse_eddi(sta=3)` → `Some(EddiMode::Normal)`. Test added.
2. `parse_eddi(sta=4)` → `Some(EddiMode::Normal)`. Test added.
3. `parse_eddi(sta=5)` → `Some(EddiMode::Normal)`. Test added.
4. `parse_eddi(sta=0)` → `Some(EddiMode::Stopped)`. Test added.
5. `parse_eddi(sta=6)` → `Some(EddiMode::Stopped)`. Test added.
6. `parse_eddi(sta=1)` → `Some(EddiMode::Normal)` (Paused = Normal mode).
   Existing test `parse_eddi_sta_1_is_normal` repurposed; comment notes
   this is "paused under Normal mode" not "explicit Normal indicator".
7. Existing `parse_eddi_sta_0_is_stopped` and
   `parse_eddi_unknown_sta_is_stopped_safe_default` updated: `sta=0` →
   Stopped (kept), `sta=99` → Normal (changed; tighten the mapping
   semantically: only known-Stopped codes mean Stopped, anything unknown
   defaults to Normal — this is the *opposite* safe direction at the
   parser layer, but the EddiController already enforces the
   `safe-default Stopped on Stale/Unknown freshness` contract one layer
   up, so a Normal report from a bogus sta value gets overridden the
   moment freshness decays). Document this in the parser comment.

   **Open call**: whether to default unknown sta values to Stopped
   (preserves current safe direction at the parser layer; risks more
   false-Stopped on firmware variants) or Normal (matches the
   docs-driven expectation that *any* operational status implies the
   mode is Normal). Decision: Normal, per the docs-driven mapping. The
   controller's freshness-driven safety net is the actual safety layer.

8. `WorldSnapshot.typed_sensors` (new wire-model block) carries:

   - `eddi_mode: TypedSensorEnum { value: opt[str], freshness, since_epoch_ms, raw_json: opt[str] }`
   - `zappi: TypedSensorZappi { mode: opt[str], status: opt[str], plug_state: opt[str], freshness, since_epoch_ms, raw_json: opt[str] }`

   The ZappiState struct surfaces three string fields rather than
   collapsing into one composite display value — the renderer can choose
   to render one row "zappi" with a composite display, but the wire
   format keeps the three fields addressable for the popup.

9. `renderSensors` in `web/src/render.ts` emits two new rows
   (`eddi.mode` and `zappi`) into `#sensors-table`, sorted alphabetically
   alongside the f64 rows by display name. Each row shows the parsed
   value (formatted), freshness with `since` epoch, and the raw_json is
   accessible via the existing inspector popup (item 11).

10. `web/src/displayNames.ts` and `web/src/descriptions.ts` carry the
    two new sensor entries (`eddi.mode`, `zappi`) so the entity inspector
    has prose to render.

11. The entity inspector popup, when opened on a sensor that has
    `raw_json` populated, shows a **Raw response** collapsible panel
    containing pretty-printed JSON in a `<pre>` and a copy-to-clipboard
    button. When `raw_json` is `None` (poll never succeeded, or the
    sensor has no raw form), the panel is omitted entirely (no empty
    section, no "no data" placeholder — silent absence). Parser tests
    + tsc verify both branches.

12. `Poller::poll_once` populates `raw_json` on success and emits
    `raw_json: None` on parser-success-but-no-body or on poll error. The
    raw body is pretty-printed (`serde_json::to_string_pretty`) so the
    popup `<pre>` reads naturally. Parser cost: one allocation per
    poll-cycle per device; acceptable at the 30-60 s cadence.

13. Verification:

    - `cargo test --workspace` — all green, including the new parser
      tests and any new typed-sensor convert tests.
    - `cargo clippy --workspace --all-targets -- -D warnings` — no
      warnings.
    - `cd web && ./node_modules/.bin/tsc --noEmit -p .` — no errors.

## Plan layers (analogous to the "Adding a new knob" checklist)

A new typed-sensor wire field touches:

1. **Baboon model** — `models/dashboard.baboon`: new `data
   TypedSensors`, `data TypedSensorEnum`, `data TypedSensorZappi` plus
   a `typed_sensors: TypedSensors` field on `WorldSnapshot`. Run
   `scripts/regen-baboon.sh`.
2. **Core typed reading** — `crates/core/src/types.rs`
   `TypedReading::Eddi` and `::Zappi` gain `raw_json: Option<String>`.
   `Event::TypedSensor` carries the field through.
3. **Core apply** — `crates/core/src/process.rs::apply_typed_reading`
   already routes Eddi/Zappi into `world.typed_sensors`; needs to also
   stamp the raw_json onto a sibling field (extend `Actual<T>` is the
   wrong shape — raw_json travels alongside `value`/`freshness`/`since`
   but isn't part of the freshness lifecycle. Cleanest: hold it as
   `Option<String>` on a sibling field, e.g.
   `world.typed_sensors.eddi_raw_json`, populated from the typed
   reading; cleared on freshness decay or on `reset_to_unset`.
   Alternative: extend `Actual<T>` with a `raw: Option<String>` field —
   touches every Actual call-site, more invasive). **Decision: sibling
   field on `TypedSensors`.** Less ripple; keeps `Actual<T>` focused on
   value/freshness/since.
4. **Shell poller** — `crates/shell/src/myenergi/mod.rs::poll_once`:
   stamp the pretty-printed body onto the `TypedReading::{Eddi,Zappi}`
   payload it sends. On poll error, no event is sent (existing
   behaviour); on parse failure, send the event with `raw_json =
   Some(body)` (so the popup shows what we couldn't parse) and the
   parsed value as None. **Refinement**: today the poller emits
   nothing on parse failure (`parse_eddi` returns None and the match
   arm is `Ok(Some(mode)) => …`). Keep that behaviour for now;
   raw-on-parse-failure is a future hardening. Scope this PR to the
   common path: parse success → emit with raw_json=Some(body).
5. **Shell convert** — `crates/shell/src/dashboard/convert.rs`:
   construct `WorldSnapshot.typed_sensors` from `world.typed_sensors`.
   Map `Actual<EddiMode>` and `Actual<ZappiState>` plus the sibling
   raw_json fields onto the wire types.
6. **Web render** — `web/src/render.ts::renderSensors`: two new rows
   sourced from `snap.typed_sensors`; same KeyedRow shape as the f64
   rows (composite display value column for zappi).
7. **Web display names + descriptions** — `web/src/displayNames.ts`
   `DISPLAY_NAMES` adds `eddi.mode` and `zappi`. `web/src/descriptions.ts`
   adds prose for both.
8. **Web inspector popup** — `web/src/entityInspector.ts` (or wherever
   the click-handler lives — confirm at execute time): render the
   "Raw response" panel from the looked-up `raw_json`.

## Risks and assumptions

- **Heuristic mapping for `sta`**. Documented values are not
  authoritative until verified against a real capture. The popup work
  in this PR is the verification mechanism — once deployed, the user
  can click `eddi.mode` and see what `sta` value was actually present
  when mode was Normal. If the heuristic is wrong on the user's
  firmware, follow-up PR to refine.
- **`Actual<EddiMode>` and `Actual<ZappiState>` already exist** in
  `world.typed_sensors`. The work is plumbing, not new state.
- **Freshness threshold for typed sensors**: the existing
  `world.typed_sensors.eddi_mode.tick(at, ...)` decay logic stays as-is.
  raw_json sibling field follows the same `since`/`reset_to_unset`
  lifecycle to avoid stale raw bodies hanging around after the parsed
  value goes Stale. (Or it doesn't — if the goal is "see the last raw
  body the operator can paste into a bug report", surviving past
  freshness decay is *useful*. Decision: raw_json survives freshness
  decay, cleared only on `reset_to_unset`. Document this in the comment.

## Single PR vs split

Tight coupling (B depends on the wire format introduced for C; D
depends on the wire format for both; A is conceptually independent but
only one file + tests). Ship as **one PR**: `PR-EDDI-SENSORS-1`. The
full registration-checklist is the load-bearing risk; bundling reduces
the chance of a half-shipped intermediate state.
