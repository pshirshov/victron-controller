//! PR-HEATING-CURVE-1: cell-addressing vocabulary for the 5×2 heating-water
//! curve lookup table. Mirrors `weather_soc_addr` — one module owns the
//! sibling enums (`RowIndex` / `CellField`) plus the kebab-case wire
//! tokens used by every plumbing layer (MQTT topic-tail, dashboard
//! display name, HA discovery `unique_id` segment, TS knob-name
//! generator). Keeping them together here means a future schema change
//! lands in one file rather than fanning out across `types.rs`,
//! `serialize.rs`, `discovery.rs`, `convert.rs`, `knobs.ts`, and
//! `displayNames.ts` simultaneously.

/// Five buckets along the outdoor-temperature axis. Each bucket carries
/// an `outdoor_max_c` upper bound (inclusive) and a `water_target_c`
/// target temperature. Buckets are evaluated in ascending order; the
/// first bucket where `outdoor_c <= outdoor_max_c` wins. The last
/// bucket's `outdoor_max_c` is a high sentinel acting as the
/// catch-all anchor so any outdoor temperature above the (N-1)th
/// threshold falls into row 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowIndex {
    Row0,
    Row1,
    Row2,
    Row3,
    Row4,
}

impl RowIndex {
    pub const ALL: &'static [RowIndex] = &[
        RowIndex::Row0,
        RowIndex::Row1,
        RowIndex::Row2,
        RowIndex::Row3,
        RowIndex::Row4,
    ];

    /// Kebab-case wire token used by all plumbing layers
    /// (`heating.curve.<row>.<field>`).
    #[must_use]
    pub const fn kebab(self) -> &'static str {
        match self {
            Self::Row0 => "row-0",
            Self::Row1 => "row-1",
            Self::Row2 => "row-2",
            Self::Row3 => "row-3",
            Self::Row4 => "row-4",
        }
    }

    /// Inverse of [`Self::kebab`].
    #[must_use]
    pub fn from_kebab(s: &str) -> Option<Self> {
        Some(match s {
            "row-0" => Self::Row0,
            "row-1" => Self::Row1,
            "row-2" => Self::Row2,
            "row-3" => Self::Row3,
            "row-4" => Self::Row4,
            _ => return None,
        })
    }
}

/// Two operator-tunable fields per row. Both are floats — `OutdoorMaxC`
/// in °C (outdoor-temperature threshold), `WaterTargetC` in °C
/// (heating-loop setpoint). Stored as f64 in the cell struct; the
/// controller rounds the water target down to i32 at the actuator
/// boundary (matches `LgHeatingWaterTargetC`'s wire type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellField {
    OutdoorMaxC,
    WaterTargetC,
}

impl CellField {
    pub const ALL: &'static [CellField] = &[CellField::OutdoorMaxC, CellField::WaterTargetC];

    #[must_use]
    pub const fn kebab(self) -> &'static str {
        match self {
            Self::OutdoorMaxC => "outdoor-max-c",
            Self::WaterTargetC => "water-target-c",
        }
    }

    #[must_use]
    pub fn from_kebab(s: &str) -> Option<Self> {
        Some(match s {
            "outdoor-max-c" => Self::OutdoorMaxC,
            "water-target-c" => Self::WaterTargetC,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_index_kebab_round_trip() {
        for &r in RowIndex::ALL {
            assert_eq!(RowIndex::from_kebab(r.kebab()), Some(r));
        }
        assert_eq!(RowIndex::from_kebab("nope"), None);
    }

    #[test]
    fn cell_field_kebab_round_trip() {
        for &f in CellField::ALL {
            assert_eq!(CellField::from_kebab(f.kebab()), Some(f));
        }
        assert_eq!(CellField::from_kebab("nope"), None);
    }

    #[test]
    fn cartesian_product_is_10() {
        let total = RowIndex::ALL.len() * CellField::ALL.len();
        assert_eq!(total, 10);
    }
}
