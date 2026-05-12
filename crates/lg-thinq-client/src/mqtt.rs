//! AWS IoT Core MQTT push subscriber.
//!
//! Once [`provision::provision`](crate::provision::provision) hands us
//! a [`CertBundle`](crate::CertBundle), this module connects to the
//! `mqttServer` LG returned, subscribes to the per-client topic, and
//! streams decoded push events to a tokio channel.
//!
//! Reconnect handling is delegated to rumqttc's event loop; the caller
//! sees a single long-lived `PushReceiver` regardless of the
//! underlying connection's churn.

use std::sync::Arc;
use std::time::Duration;

use rumqttc::{
    AsyncClient, Event, EventLoop, Incoming, MqttOptions, Packet, QoS, TlsConfiguration,
    Transport,
};
use rustls::{ClientConfig, RootCertStore};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::CertBundle;
use crate::error::{Error, Result};

/// Standard AWS IoT Core mTLS port. The endpoints LG hands out are
/// regular MQTT-over-TLS on 8883; we don't need to opt into the
/// ALPN-on-443 variant for this client.
const AWS_IOT_MQTT_PORT: u16 = 8883;

/// Match the Python SDK exactly. LG's broker drops idle connections
/// fast; a shorter keep-alive keeps the channel warm against NAT
/// timeouts.
const KEEP_ALIVE: Duration = Duration::from_secs(6);

/// Decoded LG push event. The wire format is JSON; the standard
/// envelope carries a `deviceId`, optional `deviceType`, and either a
/// `report` (state delta) or `push` (notification) body.
///
/// Unknown fields are preserved in `raw` so the caller can inspect
/// them — LG occasionally adds fields without a schema bump.
#[derive(Debug, Clone, Deserialize)]
pub struct PushEvent {
    #[serde(rename = "deviceId")]
    pub device_id: String,
    #[serde(rename = "deviceType")]
    #[serde(default)]
    pub device_type: Option<String>,
    /// State delta. For a heat pump this contains
    /// `operation`/`hotWaterTemperatureInUnits`/...
    #[serde(default)]
    pub report: Option<Value>,
    /// Notification push (e.g. error events).
    #[serde(default)]
    pub push: Option<Value>,
    /// The original JSON, retained for callers that want to inspect
    /// fields the typed struct doesn't expose.
    #[serde(skip)]
    pub raw: Option<Value>,
}

impl PushEvent {
    /// Parse a raw MQTT payload (bytes-of-UTF-8-JSON) into a typed
    /// event. The `raw` field is populated with the original Value to
    /// preserve forward compatibility.
    pub fn parse(payload: &[u8]) -> Result<Self> {
        let value: Value = serde_json::from_slice(payload)
            .map_err(|e| Error::Decode(format!("MQTT payload not JSON: {e}")))?;
        let mut event: PushEvent = serde_json::from_value(value.clone())
            .map_err(|e| Error::Decode(format!("MQTT payload missing deviceId: {e}")))?;
        event.raw = Some(value);
        Ok(event)
    }
}

/// Receiver side of the push event stream.
#[derive(Debug)]
pub struct PushReceiver {
    rx: mpsc::Receiver<PushEvent>,
}

impl PushReceiver {
    pub async fn recv(&mut self) -> Option<PushEvent> {
        self.rx.recv().await
    }
}

/// Spawn the MQTT subscriber. The returned [`PushReceiver`] yields
/// decoded events; the underlying tokio task lives until the channel
/// is dropped or the broker connection errors fatally.
///
/// Channel capacity is small because the events are state deltas —
/// queue depth above ~16 typically means the consumer is asleep, in
/// which case backpressure is preferable to memory bloat.
pub async fn spawn_subscriber(bundle: &CertBundle) -> Result<PushReceiver> {
    let tls = build_tls_config(bundle)?;
    let mut opts = MqttOptions::new(
        bundle.client_id.clone(),
        bundle.mqtt_server.clone(),
        AWS_IOT_MQTT_PORT,
    );
    opts.set_transport(Transport::Tls(TlsConfiguration::Rustls(Arc::new(tls))));
    opts.set_keep_alive(KEEP_ALIVE);
    opts.set_clean_session(false);
    // AWS IoT Core enforces 128 KiB on inbound packets; matching here
    // keeps a malformed broker from blowing up the buffer.
    opts.set_max_packet_size(128 * 1024, 128 * 1024);

    let (client, eventloop) = AsyncClient::new(opts, 16);
    client
        .subscribe(&bundle.subscription_topic, QoS::AtLeastOnce)
        .await
        .map_err(|e| Error::Mqtt(format!("subscribe: {e}")))?;

    let (tx, rx) = mpsc::channel::<PushEvent>(16);
    let topic = bundle.subscription_topic.clone();
    tokio::spawn(run_eventloop(eventloop, tx, topic));

    Ok(PushReceiver { rx })
}

async fn run_eventloop(
    mut eventloop: EventLoop,
    tx: mpsc::Sender<PushEvent>,
    topic: String,
) {
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(pkt))) => {
                if pkt.topic != topic {
                    // rumqttc only delivers what we subscribed to, but
                    // shared connections (or future subscriptions)
                    // could shift this — be explicit.
                    continue;
                }
                match PushEvent::parse(&pkt.payload) {
                    Ok(ev) => {
                        if tx.send(ev).await.is_err() {
                            tracing::debug!(
                                target: "lg_thinq::mqtt",
                                "push consumer dropped; shutting down event loop"
                            );
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "lg_thinq::mqtt",
                            "ignoring un-decodable push payload: {e}"
                        );
                    }
                }
            }
            Ok(Event::Incoming(Packet::Disconnect)) => {
                tracing::info!(target: "lg_thinq::mqtt", "broker sent Disconnect");
            }
            Ok(_other) => {
                // Pings, suback, etc. — uninteresting at this layer.
            }
            Err(e) => {
                // rumqttc surfaces transport errors here; it will
                // attempt to reconnect on the next `poll()`, so we
                // just log and continue. A short sleep avoids tight-
                // looping during sustained outages.
                tracing::warn!(
                    target: "lg_thinq::mqtt",
                    "MQTT eventloop error (will reconnect): {e}"
                );
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

fn build_tls_config(bundle: &CertBundle) -> Result<ClientConfig> {
    // Root CA.
    let mut root_store = RootCertStore::empty();
    let mut ca_reader = std::io::BufReader::new(bundle.root_ca_pem.as_bytes());
    for cert in rustls_pemfile::certs(&mut ca_reader) {
        let cert: CertificateDer<'static> =
            cert.map_err(|e| Error::Cert(format!("root CA parse: {e}")))?;
        root_store
            .add(cert)
            .map_err(|e| Error::Cert(format!("root CA add: {e}")))?;
    }
    if root_store.is_empty() {
        return Err(Error::Cert("no root CA certificates in bundle".into()));
    }

    // Client cert chain.
    let mut cert_reader = std::io::BufReader::new(bundle.client_cert_pem.as_bytes());
    let client_certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| Error::Cert(format!("client cert parse: {e}")))?;
    if client_certs.is_empty() {
        return Err(Error::Cert("no client certificates in bundle".into()));
    }

    // PKCS#8 private key (RSA or otherwise — rsa 0.9 writes PKCS#8).
    let mut key_reader = std::io::BufReader::new(bundle.keypair.private_key_pem.as_bytes());
    let key: PrivatePkcs8KeyDer<'static> = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
        .next()
        .ok_or_else(|| Error::Cert("no PKCS#8 key in bundle".into()))?
        .map_err(|e| Error::Cert(format!("private key parse: {e}")))?;

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, PrivateKeyDer::Pkcs8(key))
        .map_err(|e| Error::Cert(format!("rustls client auth: {e}")))?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_standard_report_envelope() {
        let payload = json!({
            "deviceId": "abc-123",
            "deviceType": "DEVICE_SYSTEM_BOILER",
            "report": {
                "operation": {"boilerOperationMode": "POWER_ON", "hotWaterMode": "ON"},
                "hotWaterTemperatureInUnits": [
                    {"unit": "C", "currentTemperature": 48.0, "targetTemperature": 50}
                ]
            }
        })
        .to_string();

        let ev = PushEvent::parse(payload.as_bytes()).unwrap();
        assert_eq!(ev.device_id, "abc-123");
        assert_eq!(ev.device_type.as_deref(), Some("DEVICE_SYSTEM_BOILER"));
        assert!(ev.report.is_some());
        assert!(ev.push.is_none());
        // raw must be populated for forward-compatibility.
        let raw = ev.raw.as_ref().unwrap();
        assert!(raw.get("deviceId").is_some());
    }

    #[test]
    fn parses_notification_envelope() {
        let payload = json!({
            "deviceId": "abc-123",
            "push": {"code": "FILTER_REPLACE_NEEDED", "message": "Replace filter"}
        })
        .to_string();

        let ev = PushEvent::parse(payload.as_bytes()).unwrap();
        assert_eq!(ev.device_id, "abc-123");
        assert!(ev.push.is_some());
        assert!(ev.report.is_none());
    }

    #[test]
    fn rejects_payload_without_device_id() {
        let payload = br#"{"report": {"operation": {}}}"#;
        let err = PushEvent::parse(payload).unwrap_err();
        assert!(matches!(err, Error::Decode(_)));
    }

    #[test]
    fn rejects_non_json_payload() {
        let err = PushEvent::parse(b"not json").unwrap_err();
        assert!(matches!(err, Error::Decode(_)));
    }

    #[test]
    fn preserves_unknown_fields_in_raw() {
        let payload = json!({
            "deviceId": "abc-123",
            "report": {"x": 1},
            "futureField": {"foo": "bar"}
        })
        .to_string();
        let ev = PushEvent::parse(payload.as_bytes()).unwrap();
        let raw = ev.raw.unwrap();
        assert_eq!(raw["futureField"]["foo"], json!("bar"));
    }
}
