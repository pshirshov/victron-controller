# victron-controller-lg-thinq-client

Minimal LG ThinQ Connect Open-API client, scoped to what the
victron-controller needs to drive the LG Therma V hydro kit
(`HM051M.U43`, presented in the API as `DEVICE_SYSTEM_BOILER`).

Not a general-purpose ThinQ SDK port — it intentionally supports one
device family, one region per configuration, and exactly the four
actuators the operator drives from the dashboard:

- heating master power on / off (`boilerOperationMode`)
- DHW circuit on / off (`hotWaterMode`)
- DHW target temperature, Celsius
- heating-loop water target temperature, Celsius

State readback covers DHW current/target, heating-loop water
current/target, room-air current, and the current job mode.

## Auth

Authentication is a Personal Access Token. To obtain one:

1. Sign in at <https://thinq.developer.lge.com/> with the same LG
   account the heat pump is registered against.
2. Create an application, enable the ThinQ Connect product.
3. Issue a PAT scoped to that application.

The PAT, country code (ISO-3166-1 alpha-2 — e.g. `IE`, `DE`, `NL`),
and a stable per-controller client-id are everything the HTTP side
needs.

The MQTT-push side additionally requires a client certificate that
LG issues against a CSR — handled automatically by
[`provision`](src/provision.rs); the resulting key/cert/CA bundle is
cached on disk and re-used on every restart. If any file goes missing
or fails to parse (e.g. flash wipe after a Venus OS upgrade) the next
`provision()` call self-heals.

## Example

```rust,no_run
use victron_controller_lg_thinq_client::{
    api::ThinqApi,
    heat_pump::{HeatPumpControl, HeatPumpState},
    provision::provision,
    region::Country,
};
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let http = reqwest::Client::new();
    let api = ThinqApi::new(http, "<PAT>", &Country::new("IE")?)?;

    // List devices, find the heat pump.
    let devices = api.get_devices().await?;
    let hp = devices
        .into_iter()
        .find(|d| d.device_info.device_type == "DEVICE_SYSTEM_BOILER")
        .expect("no heat pump on this account");

    // Read state.
    let raw = api.get_device_state(&hp.device_id).await?;
    let state = HeatPumpState::from_json(&raw)?;
    println!("DHW target: {:?}°C", state.dhw_target_c);

    // Turn DHW off.
    api.post_device_control(
        &hp.device_id,
        HeatPumpControl::set_dhw_power(false),
    )
    .await?;

    // Provision MQTT push (idempotent — cached after first call).
    let _bundle = provision(
        &api,
        Path::new("/data/var/lib/victron-controller/lg-thinq/"),
        "controller-1",
    )
    .await?;

    Ok(())
}
```

## Crypto stack

Pure-Rust throughout — no OpenSSL runtime dependency. RSA-2048
keygen via the RustCrypto `rsa` crate; CSR construction via `rcgen`
backed by `ring`; TLS via `rustls` 0.22 (matched to `rumqttc` 0.24).

## Test scope

- HTTP request shapes (headers, body, query) are mocked via
  `wiremock`.
- The cert/CSR flow is exercised end-to-end against the mock,
  including the persistent cache and re-provision-on-corruption
  branches.
- MQTT push event decoding is unit-tested against fixture payloads.
  Live AWS-IoT mTLS handshake is not exercised in the automated
  suite — that's verified manually against a registered device.
