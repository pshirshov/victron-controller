# Clarifications: weather-soc forecast-temperature input

**Context:** You want the weather-soc column selection (Cold vs Warm) driven by forecasted temperature instead of the current local sensor reading, with the local sensor demoted to fallback. Today the controller reads `world.sensors.outdoor_temperature` instantaneously (`crates/core/src/process.rs:2461`) and compares it to `winter_temperature_threshold`. Before I sketch code, the items below need decisions because they each shape a different layer (data source, aggregation, freshness, UX).

**Key facts I confirmed:**
- Open-Meteo fetch (`crates/shell/src/forecast/open_meteo.rs`) currently asks **only** for `global_tilted_irradiance` — no temperature in the response. So the "temperature forecast" source has to be added; it isn't a free byproduct of the existing call.
- `world.sunrise` / `world.sunset` (NaiveDateTime) are already present in core (`world.rs:662–663`), so a "daylight hours" window is trivially computable inside `process.rs`.
- `ControllerParams` has the `freshness_forecast: Duration = 12h` precedent (`topology.rs:54`) and weather-soc already uses it for solar fusion.
- The widget (`web/src/render.ts:1929–1963`) does **not** currently display the temperature input that drove the column.

**How to answer:** Write your response on the `Answer:` line under each question. Leave a question blank if you want to skip it. You can answer in any order; reference questions by ID (e.g. `Q3`) in chat if convenient.

---

## Q1: Where should forecasted temperature come from?

**Suggestions:**
- **Extend the existing Open-Meteo plane fetch** to additionally request `hourly=temperature_2m` — same provider, same cadence, single extra field, no new HTTP client. (recommended — lowest blast radius, reuses `ForecastSnapshot` plumbing.)
- **New dedicated Open-Meteo "weather" fetcher** separate from the irradiance per-plane fetcher, with its own cadence/freshness — cleaner separation of concerns but doubles the wiring (config section, scheduler spawn, world slot).
- **Reuse the already-existing Open-Meteo current-weather fetcher** — but that's instantaneous, not a daylight average; doesn't satisfy the "average during light hours" requirement. Listed only to rule out.

Answer: extend existing fetch

---

## Q2: What window defines "light hours"?

**Suggestions:**
- **Sunrise → sunset for today** using the existing `world.sunrise` / `world.sunset` — matches the literal phrasing, varies seasonally, no new config.  (recommended)
- **Fixed window (e.g. 09:00–17:00 local)** — simpler, deterministic, but wrong at solstices; not recommended.
- **Solar-weighted average** (weight each hour by forecast irradiance) — biases toward the temperature when the panels actually produce; theoretically more correct for "what regime are the panels in" but adds complexity and couples the temperature pipeline to the irradiance pipeline.

Answer: sunrise to sunset

---

## Q3: Which aggregation function?

**Suggestions:**
- **Arithmetic mean** of hourly forecast temperatures across the window. (recommended — matches your phrasing)
- **Min** across the window — biases the column toward Cold; useful if the operator's intent is "if any part of the day is cold, act cold".
- **Mean of (min, max)** — the meteorological "daily mean"; close to arithmetic mean for a smooth diurnal curve.

Answer: arithmetic mean

---

## Q4: When the forecast is stale/missing, what's the fallback?

**Suggestions:**
- **Fall back to the instantaneous local sensor** (= current behavior), and surface the source on the dashboard so the operator can see which input was used. (recommended)
- **Fall back to local sensor only if the sensor itself is fresh; otherwise refuse to evaluate (skip the controller tick)** — safer, but means a stale forecast + stale sensor parks weather-soc at its last commanded targets indefinitely. Worth it only if you'd rather freeze than guess.
- **Hold previous decision** — keep the last-chosen column until either input recovers; needs persistence across restarts (we don't have that for transient state) so probably not viable.

Answer: fall back to the local sensor. Keep in mind that we may have several defined sensors (currently we have two) - we should use the most recent reading

---

## Q5: How fresh is "fresh enough" for the temperature forecast?

**Suggestions:**
- **Reuse `freshness_forecast` (12 h)** — matches solar; one knob to think about. (recommended)
- **New separate `freshness_temperature_forecast` knob** — temperature forecasts age more slowly than irradiance forecasts; could be longer (e.g. 24 h). Adds a knob that nobody will ever tune.

Answer: reuse freshness_forecast

---

## Q6: Today's daylight average vs tomorrow's?

**Context:** weather-soc decides "today's regime". If the controller evaluates at 06:00 the daylight hours haven't happened yet — the forecast is a true prediction. If it evaluates at 20:00, "today's daylight" is in the past; the forecast values are essentially historical (still from the forecast model, not measured). Does the controller want to switch to *tomorrow*'s forecast at some point in the day?

**Suggestions:**
- **Always today** — simplest, stable behavior; column may flip at midnight when "today" rolls over. (recommended for first cut)
- **Today until sunset, then tomorrow** — forward-looking always; column may flip at sunset, which the operator might find surprising.
- **Rolling 24h forward** — drop the "daylight" framing, average the next 24 h hourly forecast temperature regardless of sun. Simpler, less semantically aligned with "column = today's regime".

Answer: always today

---

## Q7: Should the dashboard show the chosen temperature + its source?

**Suggestions:**
- **Yes, add a small line above/beside the table** showing e.g. `Temperature: 4.2 °C (forecast, daylight avg)` or `Temperature: 6.1 °C (sensor, fallback)`. Required IMO for operator transparency — without it you can't tell at a glance why a column was picked. (recommended)
- **No, keep the widget unchanged** — minimum diff, but reintroduces the "why is the cold column lit?" mystery.
- **Yes, but defer to a follow-up PR** — land the controller change first, ship the UI separately.

Answer: go recommended

---

## Q8: Should the new behavior be gated by config, or replace the old behavior outright?

**Suggestions:**
- **Replace outright** — single deployment, single client, no third-party consumers; project convention is to refactor freely. (recommended)
- **Gate behind a config flag (`use_forecast_temperature: bool`)** defaulting to true, with the sensor-only path retained as an escape hatch. Adds a knob nobody will flip.

Answer: replace outright

---
