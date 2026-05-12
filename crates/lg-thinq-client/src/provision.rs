//! Cert-bundle provisioning + on-disk caching.
//!
//! The MQTT push side of LG ThinQ Connect requires mutual-TLS against
//! AWS IoT Core, with a per-client cert that LG issues against a CSR
//! we submit. This module runs that handshake at most once per device
//! flash; result lands in a directory the caller picks (typically
//! `/data/var/lib/victron-controller/lg-thinq/`).
//!
//! On warm restart we try to load the cached bundle from disk and use
//! it verbatim. If any file is missing or the parser rejects it we
//! re-provision from scratch — the GX flash can be wiped during
//! firmware updates and the controller should self-heal.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::api::ThinqApi;
use crate::cert::{LgKeypair, build_csr_pem, csr_inner_base64, generate_keypair, load_keypair};
use crate::error::{Error, Result};

/// Amazon Root CA 1, the trust anchor for AWS IoT Core endpoints LG
/// puts in `mqttServer`. Valid until 2038-01-17. Embedded to avoid a
/// blocking network round-trip at provision time on a possibly-offline
/// GX.
///
/// Source: <https://www.amazontrust.com/repository/AmazonRootCA1.pem>
const AMAZON_ROOT_CA_1_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIDQTCCAimgAwIBAgITBmyfz5m/jAo54vB4ikPmljZbyjANBgkqhkiG9w0BAQsF
ADA5MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6
b24gUm9vdCBDQSAxMB4XDTE1MDUyNjAwMDAwMFoXDTM4MDExNzAwMDAwMFowOTEL
MAkGA1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJv
b3QgQ0EgMTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALJ4gHHKeNXj
ca9HgFB0fW7Y14h29Jlo91ghYPl0hAEvrAIthtOgQ3pOsqTQNroBvo3bSMgHFzZM
9O6II8c+6zf1tRn4SWiw3te5djgdYZ6k/oI2peVKVuRF4fn9tBb6dNqcmzU5L/qw
IFAGbHrQgLKm+a/sRxmPUDgH3KKHOVj4utWp+UhnMJbulHheb4mjUcAwhmahRWa6
VOujw5H5SNz/0egwLX0tdHA114gk957EWW67c4cX8jJGKLhD+rcdqsq08p8kDi1L
93FcXmn/6pUCyziKrlA4b9v7LWIbxcceVOF34GfID5yHI9Y/QCB/IIDEgEw+OyQm
jgSubJrIqg0CAwEAAaNCMEAwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMC
AYYwHQYDVR0OBBYEFIQYzIU07LwMlJQuCFmcx7IQTgoIMA0GCSqGSIb3DQEBCwUA
A4IBAQCY8jdaQZChGsV2USggNiMOruYou6r4lK5IpDB/G/wkjUu0yKGX9rbxenDI
U5PMCCjjmCXPI6T53iHTfIUJrU6adTrCC2qJeHZERxhlbI1Bjjt/msv0tadQ1wUs
N+gDS63pYaACbvXy8MWy7Vu33PqUXHeeE6V/Uq2V8viTO96LXFvKWlJbYK8U90vv
o/ufQJVtMVT8QtPHRh8jrdkPSHCa2XV4cdFyQzR1bldZwgJcJmApzyMZFo6IQ6XU
5MsI+yMRQ+hDKXJioaldXgjUkK642M4UwtBV8ob2xJNDd2ZhwLnoQdeXeGADbkpy
rqXRfboQnoZsG4q5WTP468SQvvG5
-----END CERTIFICATE-----
";

/// Files persisted under the bundle directory.
mod filenames {
    pub const CLIENT_ID: &str = "client_id.txt";
    pub const PRIVATE_KEY: &str = "private_key.pem";
    pub const CLIENT_CERT: &str = "client_cert.pem";
    pub const ROOT_CA: &str = "root_ca.pem";
    pub const METADATA: &str = "metadata.json";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMetadata {
    subscription_topic: String,
    mqtt_server: String,
}

/// Everything the MQTT push client needs to connect to AWS IoT Core
/// as this LG account's push subscriber.
pub struct CertBundle {
    pub client_id: String,
    pub mqtt_server: String,
    pub subscription_topic: String,
    pub root_ca_pem: String,
    pub client_cert_pem: String,
    pub keypair: LgKeypair,
}

impl std::fmt::Debug for CertBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertBundle")
            .field("client_id", &self.client_id)
            .field("mqtt_server", &self.mqtt_server)
            .field("subscription_topic", &self.subscription_topic)
            .field("root_ca_pem", &"<embedded>")
            .field("client_cert_pem", &"<redacted>")
            .field("keypair", &self.keypair)
            .finish()
    }
}

/// Load the cached bundle if present and parseable; otherwise run the
/// full provisioning handshake against LG and persist the result.
///
/// `client_id` is a stable identifier the LG backend uses to correlate
/// registrations across reconnects. If a `client_id.txt` already exists
/// in `dir`, it's reused; otherwise the value passed in here is used
/// and persisted.
pub async fn provision(api: &ThinqApi, dir: &Path, client_id: &str) -> Result<CertBundle> {
    if let Some(bundle) = try_load_cached(dir).await? {
        tracing::debug!(target: "lg_thinq::provision", "reusing cached cert bundle");
        return Ok(bundle);
    }

    tracing::info!(
        target: "lg_thinq::provision",
        "provisioning a fresh LG ThinQ MQTT cert bundle (this happens once per flash)"
    );
    let bundle = fresh_provision(api, client_id).await?;
    persist(dir, &bundle).await?;
    Ok(bundle)
}

/// Force a re-provision, ignoring any cached bundle. Useful when the
/// MQTT layer signals the cert was rejected (e.g. revoked at the LG
/// side after a long offline period).
pub async fn reprovision(api: &ThinqApi, dir: &Path, client_id: &str) -> Result<CertBundle> {
    // Best-effort cleanup of stale files. Ignore errors — the next
    // write will overwrite them and there's no recovery to do here.
    let _ = remove_file_if_exists(&dir.join(filenames::CLIENT_CERT)).await;
    let _ = remove_file_if_exists(&dir.join(filenames::PRIVATE_KEY)).await;
    let _ = remove_file_if_exists(&dir.join(filenames::ROOT_CA)).await;
    let _ = remove_file_if_exists(&dir.join(filenames::METADATA)).await;

    let bundle = fresh_provision(api, client_id).await?;
    persist(dir, &bundle).await?;
    Ok(bundle)
}

async fn try_load_cached(dir: &Path) -> Result<Option<CertBundle>> {
    let client_id_path = dir.join(filenames::CLIENT_ID);
    let priv_path = dir.join(filenames::PRIVATE_KEY);
    let cert_path = dir.join(filenames::CLIENT_CERT);
    let ca_path = dir.join(filenames::ROOT_CA);
    let meta_path = dir.join(filenames::METADATA);

    for p in [&client_id_path, &priv_path, &cert_path, &ca_path, &meta_path] {
        if !tokio::fs::try_exists(p).await? {
            return Ok(None);
        }
    }

    // Any single parse failure → invalidate the whole bundle. We don't
    // try to recover partial state because the four pieces have to
    // agree (the cert was signed against this keypair, etc.).
    let client_id = fs::read_to_string(&client_id_path).await?.trim().to_string();
    let priv_pem = fs::read_to_string(&priv_path).await?;
    let cert_pem = fs::read_to_string(&cert_path).await?;
    let ca_pem = fs::read_to_string(&ca_path).await?;
    let meta_raw = fs::read_to_string(&meta_path).await?;

    let keypair = match load_keypair(priv_pem) {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!(target: "lg_thinq::provision",
                "cached private key did not parse; will re-provision: {e}");
            return Ok(None);
        }
    };
    let metadata: PersistedMetadata = match serde_json::from_str(&meta_raw) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(target: "lg_thinq::provision",
                "cached metadata did not parse; will re-provision: {e}");
            return Ok(None);
        }
    };
    // Sanity-check the cert PEM is at least *a* PEM block. We can't
    // verify the signature without rebuilding LG's trust chain, but
    // catching truncated/empty files here means we'll re-provision
    // instead of failing at mTLS handshake.
    if pem::parse(cert_pem.as_bytes()).is_err() {
        tracing::warn!(target: "lg_thinq::provision",
            "cached client cert is not valid PEM; will re-provision");
        return Ok(None);
    }

    Ok(Some(CertBundle {
        client_id,
        mqtt_server: metadata.mqtt_server,
        subscription_topic: metadata.subscription_topic,
        root_ca_pem: ca_pem,
        client_cert_pem: cert_pem,
        keypair,
    }))
}

async fn fresh_provision(api: &ThinqApi, client_id: &str) -> Result<CertBundle> {
    // 1. Register this client_id for MQTT push. Idempotent.
    api.post_client_register().await?;

    // 2. Local keygen + CSR.
    let keypair = generate_keypair()?;
    let csr_pem = build_csr_pem(&keypair)?;
    let csr_inner = csr_inner_base64(&csr_pem)?;

    // 3. Submit CSR; receive signed cert + subscription topic.
    let cert_resp = api.post_client_certificate(&csr_inner).await?;
    let subscription_topic = cert_resp
        .subscriptions
        .into_iter()
        .next()
        .ok_or_else(|| Error::Cert("LG returned no subscription topics".into()))?;

    // 4. Fetch the MQTT endpoint hostname.
    let route = api.get_route().await?;

    Ok(CertBundle {
        client_id: client_id.to_string(),
        mqtt_server: route.host().to_string(),
        subscription_topic,
        root_ca_pem: AMAZON_ROOT_CA_1_PEM.to_string(),
        client_cert_pem: cert_resp.certificate_pem,
        keypair,
    })
}

async fn persist(dir: &Path, bundle: &CertBundle) -> Result<()> {
    fs::create_dir_all(dir).await?;
    atomic_write(&dir.join(filenames::CLIENT_ID), bundle.client_id.as_bytes()).await?;
    atomic_write(
        &dir.join(filenames::PRIVATE_KEY),
        bundle.keypair.private_key_pem.as_bytes(),
    )
    .await?;
    atomic_write(
        &dir.join(filenames::CLIENT_CERT),
        bundle.client_cert_pem.as_bytes(),
    )
    .await?;
    atomic_write(&dir.join(filenames::ROOT_CA), bundle.root_ca_pem.as_bytes()).await?;
    let metadata = PersistedMetadata {
        subscription_topic: bundle.subscription_topic.clone(),
        mqtt_server: bundle.mqtt_server.clone(),
    };
    let meta_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| Error::Cert(format!("metadata encode: {e}")))?;
    atomic_write(&dir.join(filenames::METADATA), meta_json.as_bytes()).await?;
    Ok(())
}

/// Write a file by staging at `<path>.tmp` then renaming. The rename
/// is atomic on Linux when both names live on the same filesystem,
/// which is the only case the GX deployment uses.
async fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let tmp: PathBuf = {
        let mut p = path.as_os_str().to_owned();
        p.push(".tmp");
        PathBuf::from(p)
    };
    fs::write(&tmp, data).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

async fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Auth;
    use crate::region::Country;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_api(server: &MockServer) -> ThinqApi {
        let http = reqwest::Client::new();
        let auth = Auth::new_with_client_id("tok", Country::new("IE").unwrap(), "cid-1");
        let url = reqwest::Url::parse(&server.uri()).unwrap();
        ThinqApi::with_base_url(http, auth, url)
    }

    async fn mock_full_provision_flow(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/client"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"response": {}})))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/client/certificate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {
                    "result": {
                        "certificatePem": "-----BEGIN CERTIFICATE-----\nMIIBhTCB7w==\n-----END CERTIFICATE-----\n",
                        "subscriptions": ["app/clients/cid-1/push"]
                    }
                }
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {"mqttServer": "mqtts://broker.example.com:8883"}
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn fresh_provision_writes_all_files() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mock_full_provision_flow(&server).await;

        let api = make_api(&server);
        let bundle = provision(&api, tmp.path(), "cid-1").await.unwrap();

        assert_eq!(bundle.client_id, "cid-1");
        assert_eq!(bundle.mqtt_server, "broker.example.com");
        assert_eq!(bundle.subscription_topic, "app/clients/cid-1/push");
        assert!(bundle.root_ca_pem.contains("BEGIN CERTIFICATE"));

        for f in [
            filenames::CLIENT_ID,
            filenames::PRIVATE_KEY,
            filenames::CLIENT_CERT,
            filenames::ROOT_CA,
            filenames::METADATA,
        ] {
            let p = tmp.path().join(f);
            assert!(
                tokio::fs::try_exists(&p).await.unwrap(),
                "missing persisted file: {p:?}"
            );
        }
    }

    #[tokio::test]
    async fn second_provision_uses_cache_and_does_not_hit_network() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mock_full_provision_flow(&server).await;

        let api = make_api(&server);
        let _ = provision(&api, tmp.path(), "cid-1").await.unwrap();

        // Tear the mock server down; if `provision` tries to call out
        // again we'll get a network error.
        drop(server);

        let bundle = provision(&api, tmp.path(), "cid-1").await.unwrap();
        assert_eq!(bundle.subscription_topic, "app/clients/cid-1/push");
    }

    #[tokio::test]
    async fn corrupt_private_key_triggers_reprovision() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mock_full_provision_flow(&server).await;

        let api = make_api(&server);
        let _ = provision(&api, tmp.path(), "cid-1").await.unwrap();

        // Corrupt the cached private key. Next `provision()` must
        // notice and re-run the full flow (so the mock server has to
        // stay up).
        tokio::fs::write(tmp.path().join(filenames::PRIVATE_KEY), b"garbage")
            .await
            .unwrap();

        let bundle = provision(&api, tmp.path(), "cid-1").await.unwrap();
        assert_eq!(bundle.client_id, "cid-1");
    }

    #[tokio::test]
    async fn missing_metadata_triggers_reprovision() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mock_full_provision_flow(&server).await;

        let api = make_api(&server);
        let _ = provision(&api, tmp.path(), "cid-1").await.unwrap();
        tokio::fs::remove_file(tmp.path().join(filenames::METADATA))
            .await
            .unwrap();

        // Will need to hit the network again; the mocks are still
        // mounted.
        let _ = provision(&api, tmp.path(), "cid-1").await.unwrap();
    }

    #[tokio::test]
    async fn reprovision_overwrites_cached_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        mock_full_provision_flow(&server).await;

        let api = make_api(&server);
        let first = provision(&api, tmp.path(), "cid-1").await.unwrap();
        let second = reprovision(&api, tmp.path(), "cid-1").await.unwrap();

        // The cert from the second call must have been written, even
        // though the first call's files were present.
        assert_eq!(second.client_id, first.client_id);
        let on_disk = tokio::fs::read_to_string(tmp.path().join(filenames::CLIENT_CERT))
            .await
            .unwrap();
        assert_eq!(on_disk, second.client_cert_pem);
    }
}
