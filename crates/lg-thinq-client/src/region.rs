use crate::error::{Error, Result};

/// LG ThinQ Connect serves three regional clusters. The country code
/// chosen at PAT issuance determines which cluster a token can talk to;
/// pointing the client at the wrong region returns `NOT_SUPPORTED_COUNTRY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainPrefix {
    /// Korea + Asia/Pacific
    Kic,
    /// Americas
    Aic,
    /// Europe, Middle East, Africa
    Eic,
}

impl DomainPrefix {
    /// Lowercase prefix used in the API hostname.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kic => "kic",
            Self::Aic => "aic",
            Self::Eic => "eic",
        }
    }

    pub fn base_url(self) -> String {
        format!("https://api-{}.lgthinq.com", self.as_str())
    }
}

/// ISO-3166-1 alpha-2 country code accepted by ThinQ Connect.
///
/// Stored as the raw two-letter string so we don't have to enumerate
/// the ~120 supported countries — we only need the country → region
/// mapping, and that's a small switch over country-code prefixes plus
/// a handful of exceptions. Compared to porting the full Python table
/// this is one screen instead of 350 lines, and keeps unknown codes
/// surfaceable as a clean error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country(String);

impl Country {
    pub fn new(code: &str) -> Result<Self> {
        let code = code.trim().to_ascii_uppercase();
        if code.len() != 2 || !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(Error::UnsupportedCountry(code));
        }
        // Validate region resolves; if it does, we accept it.
        Self::resolve_region(&code)?;
        Ok(Self(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn region(&self) -> DomainPrefix {
        // `new()` already validated this; unwrap is sound.
        Self::resolve_region(&self.0).expect("validated at construction")
    }

    fn resolve_region(code: &str) -> Result<DomainPrefix> {
        // Layout copied from the official SDK's
        // `country.SUPPORTED_COUNTRIES`. The three buckets are disjoint;
        // we use a per-code match rather than carrying the full lists.
        // Source-of-truth check: bump this if a future LG release
        // reassigns a code (rare).
        Ok(match code {
            // Americas (AIC)
            "AG" | "AR" | "BB" | "BO" | "BR" | "BS" | "BZ" | "CA" | "CL"
            | "CO" | "CR" | "CU" | "DM" | "DO" | "EC" | "GD" | "GT" | "GY"
            | "HN" | "HT" | "JM" | "KN" | "LC" | "MX" | "NI" | "PA" | "PE"
            | "PR" | "PY" | "SR" | "SV" | "TT" | "US" | "UY" | "VC" | "VE"
            | "AW" => DomainPrefix::Aic,

            // Korea + Asia/Pacific (KIC)
            "AU" | "BD" | "CN" | "HK" | "ID" | "IN" | "JP" | "KH" | "KR"
            | "LA" | "LK" | "MM" | "MY" | "NP" | "NZ" | "PH" | "PK" | "SG"
            | "TH" | "TW" | "VN" => DomainPrefix::Kic,

            // Everything else LG supports — EMEA — defaults to EIC.
            // Unknown codes are rejected so we don't silently pick the
            // wrong region.
            "AE" | "AF" | "AL" | "AM" | "AO" | "AT" | "AZ" | "BA" | "BE"
            | "BF" | "BG" | "BH" | "BJ" | "BY" | "CD" | "CF" | "CG" | "CH"
            | "CI" | "CM" | "CV" | "CY" | "CZ" | "DE" | "DJ" | "DK" | "DZ"
            | "EE" | "EG" | "ES" | "ET" | "FI" | "FR" | "GA" | "GB" | "GE"
            | "GH" | "GM" | "GN" | "GQ" | "GR" | "HR" | "HU" | "IE" | "IL"
            | "IQ" | "IR" | "IS" | "IT" | "JO" | "KE" | "KG" | "KW" | "KZ"
            | "LB" | "LR" | "LT" | "LU" | "LV" | "LY" | "MA" | "MD" | "ME"
            | "MK" | "ML" | "MR" | "MT" | "MU" | "MW" | "NE" | "NG" | "NL"
            | "NO" | "OM" | "PL" | "PS" | "PT" | "QA" | "RO" | "RS" | "RU"
            | "RW" | "SA" | "SD" | "SE" | "SI" | "SK" | "SL" | "SN" | "SO"
            | "ST" | "SY" | "TD" | "TG" | "TN" | "TR" | "TZ" | "UA" | "UG"
            | "UZ" | "XK" | "YE" | "ZA" | "ZM" => DomainPrefix::Eic,

            other => return Err(Error::UnsupportedCountry(other.to_string())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ireland_is_eic() {
        let c = Country::new("IE").unwrap();
        assert_eq!(c.region(), DomainPrefix::Eic);
        assert_eq!(c.region().base_url(), "https://api-eic.lgthinq.com");
    }

    #[test]
    fn lowercase_accepted_and_normalised() {
        let c = Country::new("ie").unwrap();
        assert_eq!(c.as_str(), "IE");
    }

    #[test]
    fn unknown_country_is_rejected() {
        assert!(matches!(
            Country::new("ZZ"),
            Err(Error::UnsupportedCountry(_))
        ));
    }

    #[test]
    fn malformed_country_is_rejected() {
        assert!(Country::new("USA").is_err());
        assert!(Country::new("1").is_err());
        assert!(Country::new("").is_err());
    }

    #[test]
    fn region_spot_checks() {
        assert_eq!(Country::new("US").unwrap().region(), DomainPrefix::Aic);
        assert_eq!(Country::new("KR").unwrap().region(), DomainPrefix::Kic);
        assert_eq!(Country::new("DE").unwrap().region(), DomainPrefix::Eic);
        assert_eq!(Country::new("JP").unwrap().region(), DomainPrefix::Kic);
    }
}
