use std::time::Duration;

use reqwest::{Client, Method, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auth::Auth;
use crate::error::{Error, Result};
use crate::region::Country;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// The constant body LG expects when registering or deregistering an
/// MQTT client. `SVC202` is the service-code reserved for ThinQ Connect
/// Open API push subscriptions; `device-type=607` is the MQTT-client
/// flavour (versus, e.g., a mobile-app push token). `allowExist=true`
/// makes the call idempotent — re-registering an already-registered
/// client returns success instead of conflict.
const CLIENT_REGISTRATION_BODY: &str =
    r#"{"type":"MQTT","service-code":"SVC202","device-type":"607","allowExist":true}"#;

/// Minimal device record returned by `GET /devices`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceSummary {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    #[serde(rename = "deviceInfo")]
    pub device_info: DeviceInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceInfo {
    /// e.g. `"DEVICE_SYSTEM_BOILER"` for the HM051 hydro kit.
    #[serde(rename = "deviceType")]
    pub device_type: String,
    #[serde(rename = "modelName")]
    pub model_name: String,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub reportable: bool,
}

/// Response from `GET /route` — the MQTT broker hostname for the AWS
/// IoT push channel. LG returns `mqtts://host:port`; we parse the
/// hostname out so the MQTT client can pass it to rustls directly.
#[derive(Debug, Clone, Deserialize)]
pub struct RouteInfo {
    #[serde(rename = "mqttServer")]
    pub mqtt_server: String,
}

impl RouteInfo {
    /// Strip the `mqtts://` scheme and any `:port` suffix, leaving just
    /// the hostname suitable for [`rumqttc::MqttOptions::new`].
    pub fn host(&self) -> &str {
        let s = self
            .mqtt_server
            .strip_prefix("mqtts://")
            .unwrap_or(&self.mqtt_server);
        s.split(':').next().unwrap_or(s)
    }
}

/// Response from `POST /client/certificate`.
#[derive(Debug, Clone, Deserialize)]
pub struct CertificateResponse {
    #[serde(rename = "certificatePem")]
    pub certificate_pem: String,
    pub subscriptions: Vec<String>,
}

/// HTTP client for LG ThinQ Connect Open API.
///
/// One instance per controller. Reuses a single `reqwest::Client`
/// (pooled connections, rustls TLS), so cost-per-request is dominated
/// by network latency to LG, not setup overhead.
#[derive(Debug, Clone)]
pub struct ThinqApi {
    http: Client,
    auth: Auth,
    base_url: Url,
}

impl ThinqApi {
    pub fn new(http: Client, access_token: impl Into<String>, country: &Country) -> Result<Self> {
        let auth = Auth::new(access_token, country.clone());
        Self::with_auth(http, auth, country)
    }

    pub fn with_auth(http: Client, auth: Auth, country: &Country) -> Result<Self> {
        let base_url = Url::parse(&country.region().base_url())
            .map_err(|e| Error::Config(format!("invalid base URL: {e}")))?;
        Ok(Self {
            http,
            auth,
            base_url,
        })
    }

    /// Construct a [`ThinqApi`] pointing at a caller-supplied base URL.
    /// Intended for tests (against a mock server) and HTTP proxy
    /// scenarios. Production code should use [`ThinqApi::new`] so the
    /// region/PAT pair stays consistent.
    pub fn with_base_url(http: Client, auth: Auth, base_url: Url) -> Self {
        Self {
            http,
            auth,
            base_url,
        }
    }

    pub fn auth(&self) -> &Auth {
        &self.auth
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        // ThinQ endpoints are appended without leading slash:
        // `https://api-eic.lgthinq.com/devices`. `Url::join` does the
        // right thing as long as the base ends in `/` — guarantee that.
        let mut base = self.base_url.clone();
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        base.join(path)
            .map_err(|e| Error::Config(format!("bad path {path:?}: {e}")))
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        extra_headers: &[(&str, &str)],
    ) -> Result<Value> {
        let url = self.endpoint(path)?;
        let mut headers = self.auth.headers()?;
        for (k, v) in extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| Error::Config(e.to_string()))?,
                reqwest::header::HeaderValue::from_str(v)
                    .map_err(|e| Error::Config(e.to_string()))?,
            );
        }

        let mut req = self
            .http
            .request(method, url)
            .headers(headers)
            .timeout(DEFAULT_TIMEOUT);
        if let Some(b) = body {
            req = req.json(&b);
        }

        let resp = req.send().await?;
        parse_envelope(resp).await
    }

    pub async fn get_devices(&self) -> Result<Vec<DeviceSummary>> {
        let v = self
            .request_json(Method::GET, "devices", None, &[])
            .await?;
        // LG returns either {"devices": [...]} or the array directly,
        // depending on the API edition. Handle both shapes.
        let arr = v
            .get("devices")
            .cloned()
            .unwrap_or(v);
        serde_json::from_value(arr).map_err(|e| Error::Decode(e.to_string()))
    }

    pub async fn get_device_profile(&self, device_id: &str) -> Result<Value> {
        self.request_json(Method::GET, &format!("devices/{device_id}/profile"), None, &[])
            .await
    }

    pub async fn get_device_state(&self, device_id: &str) -> Result<Value> {
        self.request_json(Method::GET, &format!("devices/{device_id}/state"), None, &[])
            .await
    }

    /// Issue a control command. LG requires the `x-conditional-control:
    /// true` header on every control call — without it the unit will
    /// reject the command if its `remote-control-enabled` flag is set
    /// to "off", which is the default on factory-reset units.
    pub async fn post_device_control(&self, device_id: &str, payload: Value) -> Result<Value> {
        self.request_json(
            Method::POST,
            &format!("devices/{device_id}/control"),
            Some(payload),
            &[("x-conditional-control", "true")],
        )
        .await
    }

    pub async fn get_route(&self) -> Result<RouteInfo> {
        let v = self.request_json(Method::GET, "route", None, &[]).await?;
        serde_json::from_value(v).map_err(|e| Error::Decode(e.to_string()))
    }

    /// Register this client for MQTT push. Idempotent thanks to
    /// `allowExist:true` in the body.
    pub async fn post_client_register(&self) -> Result<()> {
        let body: Value = serde_json::from_str(CLIENT_REGISTRATION_BODY)
            .expect("CLIENT_REGISTRATION_BODY is a static literal");
        self.request_json(Method::POST, "client", Some(body), &[])
            .await?;
        Ok(())
    }

    pub async fn delete_client_register(&self) -> Result<()> {
        let body: Value = serde_json::from_str(CLIENT_REGISTRATION_BODY)
            .expect("CLIENT_REGISTRATION_BODY is a static literal");
        self.request_json(Method::DELETE, "client", Some(body), &[])
            .await?;
        Ok(())
    }

    /// Submit a CSR and receive a signed client certificate + the MQTT
    /// topic this client may subscribe to.
    pub async fn post_client_certificate(&self, csr_inner: &str) -> Result<CertificateResponse> {
        let body = serde_json::json!({
            "service-code": "SVC202",
            "csr": csr_inner,
        });
        let v = self
            .request_json(Method::POST, "client/certificate", Some(body), &[])
            .await?;
        // Two response shapes seen in the wild: {"result": {..}} (older
        // SDK doc) and direct field at top level (newer responses).
        let inner = v.get("result").cloned().unwrap_or(v);
        serde_json::from_value(inner).map_err(|e| Error::Decode(e.to_string()))
    }

    /// Subscribe this client to events for `device_id`. Without this
    /// the MQTT topic stays silent for the device, even though the
    /// connection is healthy. The `expire.timer` field is in hours and
    /// matches the upper bound the SDK uses (4464h = ~186 days).
    pub async fn post_event_subscribe(&self, device_id: &str) -> Result<()> {
        let body = serde_json::json!({
            "expire": {"unit": "HOUR", "timer": 4464_u32}
        });
        self.request_json(
            Method::POST,
            &format!("event/{device_id}/subscribe"),
            Some(body),
            &[],
        )
        .await?;
        Ok(())
    }
}

/// Decode the standard LG response envelope.
///
/// Success: `{"response": <payload>}` or `{"response": null}`.
/// Failure: `{"error": {"code": "1234", "message": "..."}}` with a
/// non-2xx HTTP status.
async fn parse_envelope(resp: Response) -> Result<Value> {
    let status = resp.status();
    let bytes = resp.bytes().await?;
    // Empty body on success (e.g. 204) is fine — return Null.
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Decode(format!("invalid JSON ({status}): {e}")))?;

    if status.is_success() {
        Ok(value.get("response").cloned().unwrap_or(Value::Null))
    } else {
        let err = value.get("error").cloned().unwrap_or_else(|| value.clone());
        let code = err
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("0000")
            .to_string();
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("(no message)")
            .to_string();
        Err(Error::api(code, message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Country;
    use serde_json::json;
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_api(server: &MockServer) -> ThinqApi {
        let http = reqwest::Client::new();
        let auth = Auth::new_with_client_id("tok_abc", Country::new("IE").unwrap(), "cid-1");
        let url = Url::parse(&server.uri()).unwrap();
        ThinqApi::with_base_url(http, auth, url)
    }

    #[tokio::test]
    async fn get_devices_unwraps_response_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/devices"))
            .and(header("authorization", "Bearer tok_abc"))
            .and(header("x-country", "IE"))
            .and(header("x-client-id", "cid-1"))
            .and(header_exists("x-message-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {
                    "devices": [{
                        "deviceId": "dev-1",
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

        let api = make_api(&server);
        let devs = api.get_devices().await.unwrap();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].device_id, "dev-1");
        assert_eq!(devs[0].device_info.model_name, "HM051M.U43");
    }

    #[tokio::test]
    async fn control_sets_conditional_header_and_posts_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/devices/dev-1/control"))
            .and(header("x-conditional-control", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response": {}})))
            .mount(&server)
            .await;

        let api = make_api(&server);
        api.post_device_control("dev-1", json!({"operation": {"boilerOperationMode": "POWER_ON"}}))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn api_error_response_surfaces_code_and_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/devices"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"code": "1103", "message": "token expired"}
            })))
            .mount(&server)
            .await;

        let api = make_api(&server);
        let err = api.get_devices().await.unwrap_err();
        match err {
            Error::Api { code, name, message } => {
                assert_eq!(code, "1103");
                assert_eq!(name, "INVALID_TOKEN");
                assert!(message.contains("token expired"));
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn certificate_response_with_result_wrapper_decodes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/client/certificate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {
                    "result": {
                        "certificatePem": "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n",
                        "subscriptions": ["app/clients/cid-1/push"]
                    }
                }
            })))
            .mount(&server)
            .await;

        let api = make_api(&server);
        let cr = api.post_client_certificate("CSRDATA").await.unwrap();
        assert!(cr.certificate_pem.contains("BEGIN CERTIFICATE"));
        assert_eq!(cr.subscriptions, vec!["app/clients/cid-1/push"]);
    }

    #[tokio::test]
    async fn route_info_strips_mqtts_scheme() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {"mqttServer": "mqtts://a1b2c3.iot.eu-west-1.amazonaws.com:8883"}
            })))
            .mount(&server)
            .await;

        let api = make_api(&server);
        let r = api.get_route().await.unwrap();
        assert_eq!(r.host(), "a1b2c3.iot.eu-west-1.amazonaws.com");
    }
}
