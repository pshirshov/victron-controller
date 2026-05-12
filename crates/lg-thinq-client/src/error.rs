use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors surfaced by the LG ThinQ client.
///
/// The split between [`Error::Api`] (LG-side rejection with a code) and
/// [`Error::Http`] / [`Error::Decode`] (transport / parsing failure) is
/// load-bearing for the controller: an `Api { code: "1103", .. }`
/// ("invalid token") is permanent until the PAT is rotated, while an
/// `Http` timeout is transient and should be retried.
#[derive(Debug, Error)]
pub enum Error {
    #[error("LG ThinQ API error: {code} ({name}): {message}")]
    Api {
        code: String,
        name: &'static str,
        message: String,
    },

    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("response decode error: {0}")]
    Decode(String),

    #[error("MQTT error: {0}")]
    Mqtt(String),

    #[error("certificate / key error: {0}")]
    Cert(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unsupported country code: {0}")]
    UnsupportedCountry(String),

    #[error("configuration error: {0}")]
    Config(String),
}

impl Error {
    /// Build an [`Error::Api`] from the raw `(code, message)` pair LG
    /// returns. The `name` field maps the numeric code to its symbolic
    /// constant (e.g. "1103" → `INVALID_TOKEN`) for easier log triage.
    pub fn api(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let name = api_error_name(&code);
        Self::Api {
            code,
            name,
            message: message.into(),
        }
    }
}

/// Maps an LG ThinQ Connect error code to its symbolic name.
///
/// Source: the `ThinQAPIErrorCodes` class in the official Python SDK
/// (`pythinqconnect/thinq_api.py`). Only the codes the controller is
/// likely to encounter at runtime are listed; unknown codes fall
/// through to `"UNKNOWN_ERROR"` and the caller still has the raw code.
fn api_error_name(code: &str) -> &'static str {
    match code {
        "1000" => "BAD_REQUEST",
        "1101" => "MISSING_PARAMETERS",
        "1102" => "UNACCEPTABLE_PARAMETERS",
        "1103" => "INVALID_TOKEN",
        "1104" => "INVALID_MESSAGE_ID",
        "1205" | "1213" => "NOT_REGISTERED_DEVICE",
        "1218" => "INVALID_TOKEN_AGAIN",
        "1219" => "NOT_SUPPORTED_MODEL",
        "1220" => "NOT_SUPPORTED_FEATURE",
        "1222" => "NOT_CONNECTED_DEVICE",
        "1223" => "INVALID_STATUS_DEVICE",
        "1301" => "INVALID_SERVICE_KEY",
        "1302" => "NOT_FOUND_TOKEN",
        "1304" => "NOT_ACCEPTABLE_TERMS",
        "1305" => "NOT_ALLOWED_API",
        "1306" => "EXCEEDED_API_CALLS",
        "1307" => "NOT_SUPPORTED_COUNTRY",
        "1308" => "NO_CONTROL_AUTHORITY",
        "2000" => "INTERNAL_SERVER_ERROR",
        "2207" => "INVALID_COMMAND_ERROR",
        "2208" => "FAIL_DEVICE_CONTROL",
        "2209" => "DEVICE_RESPONSE_DELAY",
        "2210" => "RETRY_REQUEST",
        "2301" => "COMMAND_NOT_SUPPORTED_IN_REMOTE_OFF",
        "2302" => "COMMAND_NOT_SUPPORTED_IN_STATE",
        "2304" => "COMMAND_NOT_SUPPORTED_IN_POWER_OFF",
        _ => "UNKNOWN_ERROR",
    }
}
