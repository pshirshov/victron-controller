//! Client-side certificate provisioning bits: RSA-2048 keypair
//! generation and CSR construction matching what LG expects on
//! `POST /client/certificate`.
//!
//! The flow this supports:
//!
//!   1. Generate a fresh RSA-2048 keypair locally (PKCS#8 PEM out).
//!   2. Build a CSR with CN=lg_thinq, signed by that key.
//!   3. Strip the PEM armour and intra-line whitespace to produce the
//!      inner base64 blob LG expects in the request body.
//!
//! The Python SDK uses pyOpenSSL for the same flow. We use the
//! RustCrypto `rsa` crate for keygen and rcgen for the CSR; both go
//! through `ring`, matching the rest of the rustls stack already
//! pulled into the workspace.

use rcgen::{CertificateParams, DnType, KeyPair, SignatureAlgorithm};
use rsa::RsaPrivateKey;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};

use crate::error::{Error, Result};

/// Production RSA key length. LG accepts >= 2048-bit; the controller
/// provisions once per device flash, so the ~1-2 s generation cost on
/// the GX is amortised across the lifetime of the deployment.
pub const PRODUCTION_KEY_BITS: usize = 2048;

/// CSR subject. The Python SDK hard-codes `CN=lg_thinq`; LG keys
/// certificate issuance off the (Bearer-authenticated) request, not
/// the CSR subject, so the CN itself is cosmetic but must be present.
const CSR_COMMON_NAME: &str = "lg_thinq";

/// An RSA-2048 keypair plus its PEM serialisation. Keeping the PEM
/// bytes alongside the in-memory key avoids re-serialising on every
/// persistence write.
pub struct LgKeypair {
    /// PKCS#8 PEM-encoded private key. Safe to write to disk.
    pub private_key_pem: String,
    /// rcgen KeyPair already configured for RSA-SHA512 signing —
    /// reusable for both CSR generation and (later) any private-key-
    /// holder operations.
    pub key_pair: KeyPair,
}

impl std::fmt::Debug for LgKeypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print private key material.
        f.debug_struct("LgKeypair")
            .field("private_key_pem", &"<redacted>")
            .field("key_pair", &"<rcgen KeyPair>")
            .finish()
    }
}

/// Generate an RSA-2048 keypair suitable for the LG CSR flow.
pub fn generate_keypair() -> Result<LgKeypair> {
    generate_keypair_with_size(PRODUCTION_KEY_BITS)
}

/// Test/utility variant that lets the caller pick the key size. Tests
/// use 1024-bit to keep `cargo test` fast; production always uses
/// [`PRODUCTION_KEY_BITS`].
pub fn generate_keypair_with_size(bits: usize) -> Result<LgKeypair> {
    let mut rng = rand::rngs::OsRng;
    let rsa_key = RsaPrivateKey::new(&mut rng, bits)
        .map_err(|e| Error::Cert(format!("RSA keygen failed: {e}")))?;

    let private_key_pem = rsa_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| Error::Cert(format!("PKCS#8 encode failed: {e}")))?
        .to_string();

    let key_pair = KeyPair::from_pem_and_sign_algo(&private_key_pem, rsa_sha512())
        .map_err(|e| Error::Cert(format!("rcgen could not load RSA key: {e}")))?;

    Ok(LgKeypair {
        private_key_pem,
        key_pair,
    })
}

/// Load an existing PKCS#8-PEM private key (from disk) back into the
/// rcgen wrapper. Used on warm restarts when a cached cert bundle is
/// already present.
pub fn load_keypair(private_key_pem: String) -> Result<LgKeypair> {
    let key_pair = KeyPair::from_pem_and_sign_algo(&private_key_pem, rsa_sha512())
        .map_err(|e| Error::Cert(format!("rcgen could not load cached RSA key: {e}")))?;
    Ok(LgKeypair {
        private_key_pem,
        key_pair,
    })
}

/// Build a PEM-encoded CSR for the given key.
pub fn build_csr_pem(kp: &LgKeypair) -> Result<String> {
    let mut params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| Error::Cert(format!("CSR params: {e}")))?;
    params
        .distinguished_name
        .push(DnType::CommonName, CSR_COMMON_NAME);

    let csr = params
        .serialize_request(&kp.key_pair)
        .map_err(|e| Error::Cert(format!("CSR serialize: {e}")))?;
    csr.pem().map_err(|e| Error::Cert(format!("CSR pem: {e}")))
}

/// Strip the `-----BEGIN/END CERTIFICATE REQUEST-----` armour and any
/// whitespace from a CSR PEM, returning just the inner base64 blob.
/// This is what LG's `/client/certificate` endpoint expects in the
/// `csr` JSON field.
pub fn csr_inner_base64(csr_pem: &str) -> Result<String> {
    let parsed = pem::parse(csr_pem.as_bytes())
        .map_err(|e| Error::Cert(format!("CSR is not valid PEM: {e}")))?;
    if parsed.tag() != "CERTIFICATE REQUEST" {
        return Err(Error::Cert(format!(
            "expected PEM tag CERTIFICATE REQUEST, got {:?}",
            parsed.tag()
        )));
    }
    // Re-encode the inner DER as base64 with no line breaks. We can't
    // just regex-strip the PEM body because rcgen wraps at 64 cols and
    // LG rejects unwrapped vs. wrapped inconsistently across regions.
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(parsed.contents()))
}

fn rsa_sha512() -> &'static SignatureAlgorithm {
    &rcgen::PKCS_RSA_SHA512
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    /// Cache one freshly-generated 2048-bit key across all tests so we
    /// pay the keygen cost (~0.5–1 s in debug) exactly once per
    /// `cargo test` run instead of N times.
    ///
    /// `ring` rejects sub-2048 RSA keys outright, so we can't drop
    /// the size for tests.
    fn shared_test_pem() -> &'static str {
        static PEM: OnceLock<String> = OnceLock::new();
        PEM.get_or_init(|| {
            generate_keypair_with_size(PRODUCTION_KEY_BITS)
                .expect("keygen")
                .private_key_pem
        })
    }

    fn shared_test_kp() -> LgKeypair {
        load_keypair(shared_test_pem().to_string()).expect("load")
    }

    #[test]
    fn generated_keypair_is_pkcs8_pem() {
        let pem = shared_test_pem();
        assert!(pem.contains("BEGIN PRIVATE KEY"));
        assert!(pem.contains("END PRIVATE KEY"));
        // Re-parse must succeed.
        let _ = load_keypair(pem.to_string()).unwrap();
    }

    #[test]
    fn csr_contains_common_name_and_round_trips() {
        let kp = shared_test_kp();
        let csr_pem = build_csr_pem(&kp).unwrap();

        assert!(csr_pem.contains("BEGIN CERTIFICATE REQUEST"));
        assert!(csr_pem.contains("END CERTIFICATE REQUEST"));

        // Parse back the PEM and verify it's a CERTIFICATE REQUEST
        // block with non-empty DER body.
        let parsed = pem::parse(csr_pem.as_bytes()).unwrap();
        assert_eq!(parsed.tag(), "CERTIFICATE REQUEST");
        assert!(!parsed.contents().is_empty());
    }

    #[test]
    fn csr_inner_base64_strips_armour_and_whitespace() {
        let kp = shared_test_kp();
        let csr_pem = build_csr_pem(&kp).unwrap();
        let inner = csr_inner_base64(&csr_pem).unwrap();

        assert!(!inner.contains('\n'));
        assert!(!inner.contains('\r'));
        assert!(!inner.contains(' '));
        assert!(!inner.contains("BEGIN"));
        assert!(!inner.contains("END"));

        // Inner must be valid base64 and decode to non-empty DER.
        use base64::Engine;
        let der = base64::engine::general_purpose::STANDARD
            .decode(&inner)
            .unwrap();
        assert!(!der.is_empty());
        // DER starts with SEQUENCE tag 0x30 for a CSR.
        assert_eq!(der[0], 0x30);
    }

    #[test]
    fn csr_inner_rejects_non_csr_pem() {
        // Encode some non-CSR PEM and ensure we reject it.
        let other = pem::Pem::new("CERTIFICATE", vec![1, 2, 3]);
        let pem_str = pem::encode(&other);
        assert!(csr_inner_base64(&pem_str).is_err());
    }
}
