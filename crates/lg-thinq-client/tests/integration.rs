//! End-to-end exercise of the public surface against a mock LG
//! backend. Covers: list devices → fetch state → decode → issue
//! control command → provision cert bundle → re-read on warm restart.
//!
//! Live integration against the real LG API would need a valid PAT and
//! a registered HM051; that test lives in the operator's runbook, not
//! the automated suite.

use serde_json::json;
use victron_controller_lg_thinq_client::api::ThinqApi;
use victron_controller_lg_thinq_client::auth::Auth;
use victron_controller_lg_thinq_client::heat_pump::{HeatPumpControl, HeatPumpState};
use victron_controller_lg_thinq_client::provision::provision;
use victron_controller_lg_thinq_client::region::Country;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_api(server: &MockServer) -> ThinqApi {
    let http = reqwest::Client::new();
    let auth = Auth::new_with_client_id("PAT_TOKEN", Country::new("IE").unwrap(), "cid-integ");
    let url = reqwest::Url::parse(&server.uri()).unwrap();
    ThinqApi::with_base_url(http, auth, url)
}

#[tokio::test]
async fn end_to_end_state_read_control_write_and_cert_provisioning() {
    let server = MockServer::start().await;

    // 1. Device listing.
    Mock::given(method("GET"))
        .and(path("/devices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "devices": [{
                    "deviceId": "hp-1",
                    "deviceInfo": {
                        "deviceType": "DEVICE_SYSTEM_BOILER",
                        "modelName": "HM051M.U43",
                        "alias": "Heat Pump",
                        "reportable": true
                    }
                }]
            }
        })))
        .mount(&server)
        .await;

    // 2. State fetch.
    Mock::given(method("GET"))
        .and(path("/devices/hp-1/state"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "boilerJobMode": {"currentJobMode": "AUTO"},
                "operation": {
                    "boilerOperationMode": "POWER_ON",
                    "hotWaterMode": "ON"
                },
                "hotWaterTemperatureInUnits": [
                    {"unit": "C", "currentTemperature": 47.0, "targetTemperature": 48}
                ],
                "roomTemperatureInUnits": [
                    {"unit": "C",
                     "outWaterCurrentTemperature": 34.0,
                     "waterHeatTargetTemperature": 35}
                ]
            }
        })))
        .mount(&server)
        .await;

    // 3. Control: heating off. We assert the exact body LG sees.
    Mock::given(method("POST"))
        .and(path("/devices/hp-1/control"))
        .and(body_partial_json(json!({
            "operation": {"boilerOperationMode": "POWER_OFF"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response": {}})))
        .mount(&server)
        .await;

    // 4. Cert provisioning leg.
    Mock::given(method("POST"))
        .and(path("/client"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response": {}})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/client/certificate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "result": {
                    "certificatePem": "-----BEGIN CERTIFICATE-----\nMIIBhTCB7w==\n-----END CERTIFICATE-----\n",
                    "subscriptions": ["app/clients/cid-integ/push"]
                }
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/route"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {"mqttServer": "mqtts://broker.example.com:8883"}
        })))
        .mount(&server)
        .await;

    let api = make_api(&server);

    // --- run the exercise ---
    let devices = api.get_devices().await.unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_info.model_name, "HM051M.U43");

    let state_json = api.get_device_state(&devices[0].device_id).await.unwrap();
    let state = HeatPumpState::from_json(&state_json).unwrap();
    assert!(state.heating_enabled);
    assert!(state.dhw_enabled);
    assert_eq!(state.dhw_target_c, Some(48.0));
    assert_eq!(state.heating_water_target_c, Some(35.0));

    api.post_device_control(&devices[0].device_id, HeatPumpControl::set_heating_power(false))
        .await
        .unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let bundle = provision(&api, tmp.path(), "cid-integ").await.unwrap();
    assert_eq!(bundle.subscription_topic, "app/clients/cid-integ/push");

    // Warm restart: should not re-issue a cert.
    let bundle2 = provision(&api, tmp.path(), "cid-integ").await.unwrap();
    assert_eq!(bundle2.subscription_topic, bundle.subscription_topic);
    assert_eq!(bundle2.client_cert_pem, bundle.client_cert_pem);
}
