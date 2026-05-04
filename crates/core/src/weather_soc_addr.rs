//! PR-WSOC-EDIT-1: cell-addressing vocabulary for the 6×2 weather-SoC
//! lookup table. One module owns the three sibling enums
//! (`EnergyBucket` / `TempCol` / `CellField`) plus the kebab-case wire
//! tokens used by every plumbing layer (MQTT topic-tail, dashboard
//! display name, HA discovery `unique_id` segment, TS knob-name
//! generator). Keeping them together here means a future schema change
//! lands in one file rather than fanning out across `types.rs`,
//! `serialize.rs`, `discovery.rs`, `convert.rs`, `knobs.ts`, and
//! `displayNames.ts` simultaneously.
//!
//! `EnergyBucket` was originally defined in
//! `crates/core/src/controllers/weather_soc.rs`; PR-WSOC-EDIT-1 moves
//! it here and re-exports from that module so existing call sites don't
//! break.

/// Six energy buckets along the kWh axis. Boundary semantics match the
/// cascade: `<=` at the top of each band, `>` at the bottom.
///
/// - VerySunny: `today_energy > very_sunny_threshold`
/// - Sunny: `(too_much, very_sunny_threshold]`
/// - Mid: `(high, too_much]`
/// - Low: `(ok, high]`
/// - Dim: `(low, ok]`
/// - VeryDim: `<= low`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnergyBucket {
    VerySunny,
    Sunny,
    Mid,
    Low,
    Dim,
    VeryDim,
}

impl EnergyBucket {
    /// Cartesian-product enumeration source for downstream layers
    /// (apply_knob loop, all_knob_publish_payloads loop, MQTT discovery
    /// schema enumeration, TS KNOB_SPEC generator).
    pub const ALL: &'static [EnergyBucket] = &[
        EnergyBucket::VerySunny,
        EnergyBucket::Sunny,
        EnergyBucket::Mid,
        EnergyBucket::Low,
        EnergyBucket::Dim,
        EnergyBucket::VeryDim,
    ];

    /// Display label used by `weather_soc.rs` decision summaries.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::VerySunny => "VerySunny",
            Self::Sunny => "Sunny",
            Self::Mid => "Mid",
            Self::Low => "Low",
            Self::Dim => "Dim",
            Self::VeryDim => "VeryDim",
        }
    }

    /// Kebab-case wire token. Single source of truth used by:
    ///   - MQTT topic-tail (`weathersoc.table.<bucket>.<temp>.<field>`)
    ///   - HA discovery `unique_id` segment
    ///   - dashboard convert layer
    ///   - TS knob-name generator
    #[must_use]
    pub const fn kebab(self) -> &'static str {
        match self {
            Self::VerySunny => "very-sunny",
            Self::Sunny => "sunny",
            Self::Mid => "mid",
            Self::Low => "low",
            Self::Dim => "dim",
            Self::VeryDim => "very-dim",
        }
    }

    /// Inverse of [`Self::kebab`].
    #[must_use]
    pub fn from_kebab(s: &str) -> Option<Self> {
        Some(match s {
            "very-sunny" => Self::VerySunny,
            "sunny" => Self::Sunny,
            "mid" => Self::Mid,
            "low" => Self::Low,
            "dim" => Self::Dim,
            "very-dim" => Self::VeryDim,
            _ => return None,
        })
    }
}

/// Two temperature columns: warm (`today_temp > winter_threshold`) and
/// cold (`<= winter_threshold` — boundary at threshold counts as cold).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TempCol {
    Warm,
    Cold,
}

impl TempCol {
    pub const ALL: &'static [TempCol] = &[TempCol::Warm, TempCol::Cold];

    #[must_use]
    pub const fn kebab(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Cold => "cold",
        }
    }

    #[must_use]
    pub fn from_kebab(s: &str) -> Option<Self> {
        Some(match s {
            "warm" => Self::Warm,
            "cold" => Self::Cold,
            _ => return None,
        })
    }
}

/// Four operator-tunable fields per cell. Three are floats (%); the
/// fourth (`Extended`) is a bool that drives the derived
/// `disable_night_grid_discharge` output before the stacked override
/// applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellField {
    ExportSocThreshold,
    BatterySocTarget,
    DischargeSocTarget,
    Extended,
}

impl CellField {
    pub const ALL: &'static [CellField] = &[
        CellField::ExportSocThreshold,
        CellField::BatterySocTarget,
        CellField::DischargeSocTarget,
        CellField::Extended,
    ];

    #[must_use]
    pub const fn kebab(self) -> &'static str {
        match self {
            Self::ExportSocThreshold => "export-soc-threshold",
            Self::BatterySocTarget => "battery-soc-target",
            Self::DischargeSocTarget => "discharge-soc-target",
            Self::Extended => "extended",
        }
    }

    #[must_use]
    pub fn from_kebab(s: &str) -> Option<Self> {
        Some(match s {
            "export-soc-threshold" => Self::ExportSocThreshold,
            "battery-soc-target" => Self::BatterySocTarget,
            "discharge-soc-target" => Self::DischargeSocTarget,
            "extended" => Self::Extended,
            _ => return None,
        })
    }

    /// True if the field carries a float (%) value; false if bool.
    #[must_use]
    pub const fn is_float(self) -> bool {
        matches!(
            self,
            Self::ExportSocThreshold | Self::BatterySocTarget | Self::DischargeSocTarget
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_bucket_kebab_round_trip() {
        for &b in EnergyBucket::ALL {
            assert_eq!(EnergyBucket::from_kebab(b.kebab()), Some(b));
        }
        assert_eq!(EnergyBucket::from_kebab("nope"), None);
    }

    #[test]
    fn temp_col_kebab_round_trip() {
        for &t in TempCol::ALL {
            assert_eq!(TempCol::from_kebab(t.kebab()), Some(t));
        }
        assert_eq!(TempCol::from_kebab("nope"), None);
    }

    #[test]
    fn cell_field_kebab_round_trip() {
        for &f in CellField::ALL {
            assert_eq!(CellField::from_kebab(f.kebab()), Some(f));
        }
        assert_eq!(CellField::from_kebab("nope"), None);
    }

    #[test]
    fn cell_field_is_float_matches_kind() {
        assert!(CellField::ExportSocThreshold.is_float());
        assert!(CellField::BatterySocTarget.is_float());
        assert!(CellField::DischargeSocTarget.is_float());
        assert!(!CellField::Extended.is_float());
    }

    #[test]
    fn cartesian_product_is_48() {
        let total = EnergyBucket::ALL.len() * TempCol::ALL.len() * CellField::ALL.len();
        assert_eq!(total, 48);
    }
}
