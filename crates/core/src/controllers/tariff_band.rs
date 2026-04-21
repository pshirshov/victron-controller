//! Tariff-band classification. Direct port of the TS version in
//! `legacy/setpoint-node-red-ts/src/index.ts`.
//!
//! Bands (all local time):
//!
//! - **Peak**       17:00–19:00 (Day)
//! - **Day**        08:00–17:00 and 19:00–23:00 (Day)
//! - **NightStart** 23:00–02:00 (Night)
//! - **Boost**      02:00–05:00 (Night)
//! - **NightExtended** 05:00–08:00 (Night)

use chrono::{NaiveDateTime, Timelike};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TariffBandKind {
    Day,
    Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TariffBandSubKind {
    Day,
    Peak,
    NightStart,
    Boost,
    NightExtended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TariffBand {
    pub kind: TariffBandKind,
    pub subkind: TariffBandSubKind,
}

impl TariffBand {
    pub const PEAK: Self = Self {
        kind: TariffBandKind::Day,
        subkind: TariffBandSubKind::Peak,
    };
    pub const DAY: Self = Self {
        kind: TariffBandKind::Day,
        subkind: TariffBandSubKind::Day,
    };
    pub const NIGHT_START: Self = Self {
        kind: TariffBandKind::Night,
        subkind: TariffBandSubKind::NightStart,
    };
    pub const BOOST: Self = Self {
        kind: TariffBandKind::Night,
        subkind: TariffBandSubKind::Boost,
    };
    pub const NIGHT_EXTENDED: Self = Self {
        kind: TariffBandKind::Night,
        subkind: TariffBandSubKind::NightExtended,
    };
}

/// Classify a local-time moment into a tariff band.
///
/// Mirrors the TS `tariff_band(now: Date)` function. The branching order
/// matches the TS version; the last (extended-night) branch is the
/// unconditional fall-through that catches 05:00–08:00.
#[must_use]
pub fn tariff_band(now: NaiveDateTime) -> TariffBand {
    let h = now.hour();

    let is_peak = (17..19).contains(&h);
    if is_peak {
        return TariffBand::PEAK;
    }

    let is_day = (8..23).contains(&h);
    if is_day {
        return TariffBand::DAY;
    }

    let is_boost = (2..5).contains(&h);
    if is_boost {
        return TariffBand::BOOST;
    }

    let is_night = !(2..23).contains(&h);
    if is_night {
        return TariffBand::NIGHT_START;
    }

    // Fall-through: 05:00–08:00
    TariffBand::NIGHT_EXTENDED
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn peak_17_to_19() {
        assert_eq!(tariff_band(at(17, 0)), TariffBand::PEAK);
        assert_eq!(tariff_band(at(18, 59)), TariffBand::PEAK);
    }

    #[test]
    fn day_08_to_17_and_19_to_23() {
        assert_eq!(tariff_band(at(8, 0)), TariffBand::DAY);
        assert_eq!(tariff_band(at(12, 0)), TariffBand::DAY);
        assert_eq!(tariff_band(at(16, 59)), TariffBand::DAY);
        assert_eq!(tariff_band(at(19, 0)), TariffBand::DAY);
        assert_eq!(tariff_band(at(22, 59)), TariffBand::DAY);
    }

    #[test]
    fn night_start_23_to_02() {
        assert_eq!(tariff_band(at(23, 0)), TariffBand::NIGHT_START);
        assert_eq!(tariff_band(at(23, 59)), TariffBand::NIGHT_START);
        assert_eq!(tariff_band(at(0, 0)), TariffBand::NIGHT_START);
        assert_eq!(tariff_band(at(1, 59)), TariffBand::NIGHT_START);
    }

    #[test]
    fn boost_02_to_05() {
        assert_eq!(tariff_band(at(2, 0)), TariffBand::BOOST);
        assert_eq!(tariff_band(at(4, 59)), TariffBand::BOOST);
    }

    #[test]
    fn night_extended_05_to_08() {
        assert_eq!(tariff_band(at(5, 0)), TariffBand::NIGHT_EXTENDED);
        assert_eq!(tariff_band(at(6, 30)), TariffBand::NIGHT_EXTENDED);
        assert_eq!(tariff_band(at(7, 59)), TariffBand::NIGHT_EXTENDED);
    }

    #[test]
    fn band_kind_matches_subkind() {
        assert_eq!(TariffBand::PEAK.kind, TariffBandKind::Day);
        assert_eq!(TariffBand::DAY.kind, TariffBandKind::Day);
        assert_eq!(TariffBand::NIGHT_START.kind, TariffBandKind::Night);
        assert_eq!(TariffBand::BOOST.kind, TariffBandKind::Night);
        assert_eq!(TariffBand::NIGHT_EXTENDED.kind, TariffBandKind::Night);
    }
}
