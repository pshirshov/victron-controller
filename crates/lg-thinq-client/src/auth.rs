use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::region::Country;

/// The hard-coded ThinQ Connect API key. This is **not** a secret — it
/// is the same string shipped in every official SDK build and acts as a
/// product identifier rather than an authentication credential. The
/// real auth credential is the per-account Personal Access Token.
///
/// Source: `pythinqconnect/const.py::API_KEY`.
const API_KEY: &str = "v6GFvkweNo7DK7yD3ylIZ9w52aKBU0eJ7wLXkSR3";

/// Header names. Pre-parsed once.
fn header_name(s: &'static str) -> HeaderName {
    HeaderName::from_static(s)
}

/// Authentication state for a long-lived client. The PAT and client-id
/// are stable across requests; `x-message-id` is regenerated per call.
#[derive(Debug, Clone)]
pub struct Auth {
    access_token: String,
    country: Country,
    client_id: String,
}

impl Auth {
    /// Build an [`Auth`] from a PAT, country code, and a stable client
    /// id. The client id should be persisted across restarts so the LG
    /// backend can correlate registrations; if not supplied via
    /// [`Auth::new_with_client_id`], a fresh UUID-v4 is generated.
    pub fn new(access_token: impl Into<String>, country: Country) -> Self {
        Self::new_with_client_id(access_token, country, Uuid::new_v4().to_string())
    }

    pub fn new_with_client_id(
        access_token: impl Into<String>,
        country: Country,
        client_id: impl Into<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            country,
            client_id: client_id.into(),
        }
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn country(&self) -> &Country {
        &self.country
    }

    /// Build the per-request header set. Includes a fresh message-id.
    ///
    /// Extra headers (e.g. `x-conditional-control: true` for control
    /// commands) can be merged on top of the result.
    pub fn headers(&self) -> Result<HeaderMap> {
        let mut h = HeaderMap::with_capacity(7);

        let bearer = format!("Bearer {}", self.access_token);
        h.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(&bearer).map_err(|e| Error::Config(e.to_string()))?,
        );

        h.insert(
            header_name("x-country"),
            HeaderValue::from_str(self.country.as_str())
                .map_err(|e| Error::Config(e.to_string()))?,
        );

        h.insert(
            header_name("x-message-id"),
            HeaderValue::from_str(&generate_message_id())
                .map_err(|e| Error::Config(e.to_string()))?,
        );

        h.insert(
            header_name("x-client-id"),
            HeaderValue::from_str(&self.client_id).map_err(|e| Error::Config(e.to_string()))?,
        );

        h.insert(
            header_name("x-api-key"),
            HeaderValue::from_static(API_KEY),
        );

        h.insert(
            header_name("x-service-phase"),
            HeaderValue::from_static("OP"),
        );

        Ok(h)
    }
}

/// LG's `x-message-id` format: take a fresh UUID-v4 (16 bytes), encode
/// the bytes with URL-safe base64, drop the trailing `==` padding.
/// Result is a stable 22-char ASCII string.
///
/// Mirrors `ThinQApi._generate_message_id` in the Python SDK.
pub fn generate_message_id() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_id_is_22_chars_url_safe_no_padding() {
        for _ in 0..100 {
            let id = generate_message_id();
            assert_eq!(id.len(), 22, "id was: {id}");
            assert!(!id.contains('='));
            assert!(!id.contains('+'));
            assert!(!id.contains('/'));
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "id had non-base64url char: {id}"
            );
        }
    }

    #[test]
    fn message_ids_are_unique_across_calls() {
        let a = generate_message_id();
        let b = generate_message_id();
        assert_ne!(a, b);
    }

    #[test]
    fn headers_contain_required_fields_and_bearer_prefix() {
        let auth = Auth::new_with_client_id(
            "tok_abc",
            Country::new("IE").unwrap(),
            "stable-client-id",
        );
        let h = auth.headers().unwrap();

        assert_eq!(
            h.get("authorization").unwrap().to_str().unwrap(),
            "Bearer tok_abc"
        );
        assert_eq!(h.get("x-country").unwrap().to_str().unwrap(), "IE");
        assert_eq!(
            h.get("x-client-id").unwrap().to_str().unwrap(),
            "stable-client-id"
        );
        assert_eq!(h.get("x-api-key").unwrap().to_str().unwrap(), API_KEY);
        assert_eq!(h.get("x-service-phase").unwrap().to_str().unwrap(), "OP");
        assert_eq!(h.get("x-message-id").unwrap().to_str().unwrap().len(), 22);
    }

    #[test]
    fn each_headers_call_rotates_message_id() {
        let auth = Auth::new("tok", Country::new("IE").unwrap());
        let a = auth.headers().unwrap();
        let b = auth.headers().unwrap();
        assert_ne!(
            a.get("x-message-id").unwrap(),
            b.get("x-message-id").unwrap()
        );
    }
}
