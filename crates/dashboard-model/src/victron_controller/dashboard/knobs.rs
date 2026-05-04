use crate::victron_controller::dashboard::charge_battery_extended_mode::ChargeBatteryExtendedMode;
use crate::victron_controller::dashboard::debug_full_charge::DebugFullCharge;
use crate::victron_controller::dashboard::discharge_time::DischargeTime;
use crate::victron_controller::dashboard::extended_charge_mode::ExtendedChargeMode;
use crate::victron_controller::dashboard::forecast_disagreement_strategy::ForecastDisagreementStrategy;
use crate::victron_controller::dashboard::mode::Mode;
use crate::victron_controller::dashboard::weather_soc_table::WeatherSocTable;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Knobs {
    pub force_disable_export: bool,
    pub export_soc_threshold: f64,
    pub discharge_soc_target: f64,
    pub battery_soc_target: f64,
    pub full_charge_discharge_soc_target: f64,
    pub full_charge_export_soc_threshold: f64,
    pub discharge_time: DischargeTime,
    pub debug_full_charge: DebugFullCharge,
    pub pessimism_multiplier_modifier: f64,
    pub disable_night_grid_discharge: bool,
    pub charge_car_boost: bool,
    pub charge_car_extended_mode: ExtendedChargeMode,
    pub zappi_current_target: f64,
    pub zappi_limit: f64,
    pub zappi_emergency_margin: f64,
    pub grid_export_limit_w: i32,
    pub grid_import_limit_w: i32,
    pub allow_battery_to_car: bool,
    pub eddi_enable_soc: f64,
    pub eddi_disable_soc: f64,
    pub eddi_dwell_s: i32,
    pub weathersoc_winter_temperature_threshold: f64,
    pub weathersoc_low_energy_threshold: f64,
    pub weathersoc_ok_energy_threshold: f64,
    pub weathersoc_high_energy_threshold: f64,
    pub weathersoc_too_much_energy_threshold: f64,
    pub writes_enabled: bool,
    pub forecast_disagreement_strategy: ForecastDisagreementStrategy,
    pub charge_battery_extended_mode: ChargeBatteryExtendedMode,
    pub export_soc_threshold_mode: Mode,
    pub discharge_soc_target_mode: Mode,
    pub battery_soc_target_mode: Mode,
    pub disable_night_grid_discharge_mode: Mode,
    pub inverter_safe_discharge_enable: bool,
    pub baseline_winter_start_mm_dd: i32,
    pub baseline_winter_end_mm_dd: i32,
    pub baseline_wh_per_hour_winter: f64,
    pub baseline_wh_per_hour_summer: f64,
    pub keep_batteries_charged_during_full_charge: bool,
    pub sunrise_sunset_offset_min: i32,
    pub full_charge_defer_to_next_sunday: bool,
    pub full_charge_snap_back_max_weekday: i32,
    pub zappi_battery_drain_threshold_w: i32,
    pub zappi_battery_drain_relax_step_w: i32,
    pub zappi_battery_drain_kp: f64,
    pub zappi_battery_drain_target_w: i32,
    pub zappi_battery_drain_hard_clamp_w: i32,
    pub zappi_battery_drain_mppt_probe_w: i32,
    pub actuator_retry_s: i32,
    pub weathersoc_very_sunny_threshold: f64,
    pub weather_soc_table: WeatherSocTable,
}

impl PartialEq for Knobs {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Knobs {}

impl PartialOrd for Knobs {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Knobs {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.force_disable_export.cmp(&other.force_disable_export) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.export_soc_threshold.total_cmp(&other.export_soc_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.discharge_soc_target.total_cmp(&other.discharge_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.battery_soc_target.total_cmp(&other.battery_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.full_charge_discharge_soc_target.total_cmp(&other.full_charge_discharge_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.full_charge_export_soc_threshold.total_cmp(&other.full_charge_export_soc_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.discharge_time.cmp(&other.discharge_time) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.debug_full_charge.cmp(&other.debug_full_charge) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.pessimism_multiplier_modifier.total_cmp(&other.pessimism_multiplier_modifier) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.disable_night_grid_discharge.cmp(&other.disable_night_grid_discharge) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.charge_car_boost.cmp(&other.charge_car_boost) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.charge_car_extended_mode.cmp(&other.charge_car_extended_mode) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_current_target.total_cmp(&other.zappi_current_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_limit.total_cmp(&other.zappi_limit) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_emergency_margin.total_cmp(&other.zappi_emergency_margin) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.grid_export_limit_w.cmp(&other.grid_export_limit_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.grid_import_limit_w.cmp(&other.grid_import_limit_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.allow_battery_to_car.cmp(&other.allow_battery_to_car) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.eddi_enable_soc.total_cmp(&other.eddi_enable_soc) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.eddi_disable_soc.total_cmp(&other.eddi_disable_soc) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.eddi_dwell_s.cmp(&other.eddi_dwell_s) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weathersoc_winter_temperature_threshold.total_cmp(&other.weathersoc_winter_temperature_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weathersoc_low_energy_threshold.total_cmp(&other.weathersoc_low_energy_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weathersoc_ok_energy_threshold.total_cmp(&other.weathersoc_ok_energy_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weathersoc_high_energy_threshold.total_cmp(&other.weathersoc_high_energy_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weathersoc_too_much_energy_threshold.total_cmp(&other.weathersoc_too_much_energy_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.writes_enabled.cmp(&other.writes_enabled) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.forecast_disagreement_strategy.cmp(&other.forecast_disagreement_strategy) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.charge_battery_extended_mode.cmp(&other.charge_battery_extended_mode) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.export_soc_threshold_mode.cmp(&other.export_soc_threshold_mode) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.discharge_soc_target_mode.cmp(&other.discharge_soc_target_mode) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.battery_soc_target_mode.cmp(&other.battery_soc_target_mode) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.disable_night_grid_discharge_mode.cmp(&other.disable_night_grid_discharge_mode) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.inverter_safe_discharge_enable.cmp(&other.inverter_safe_discharge_enable) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.baseline_winter_start_mm_dd.cmp(&other.baseline_winter_start_mm_dd) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.baseline_winter_end_mm_dd.cmp(&other.baseline_winter_end_mm_dd) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.baseline_wh_per_hour_winter.total_cmp(&other.baseline_wh_per_hour_winter) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.baseline_wh_per_hour_summer.total_cmp(&other.baseline_wh_per_hour_summer) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.keep_batteries_charged_during_full_charge.cmp(&other.keep_batteries_charged_during_full_charge) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.sunrise_sunset_offset_min.cmp(&other.sunrise_sunset_offset_min) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.full_charge_defer_to_next_sunday.cmp(&other.full_charge_defer_to_next_sunday) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.full_charge_snap_back_max_weekday.cmp(&other.full_charge_snap_back_max_weekday) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_battery_drain_threshold_w.cmp(&other.zappi_battery_drain_threshold_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_battery_drain_relax_step_w.cmp(&other.zappi_battery_drain_relax_step_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_battery_drain_kp.total_cmp(&other.zappi_battery_drain_kp) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_battery_drain_target_w.cmp(&other.zappi_battery_drain_target_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_battery_drain_hard_clamp_w.cmp(&other.zappi_battery_drain_hard_clamp_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_battery_drain_mppt_probe_w.cmp(&other.zappi_battery_drain_mppt_probe_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.actuator_retry_s.cmp(&other.actuator_retry_s) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weathersoc_very_sunny_threshold.total_cmp(&other.weathersoc_very_sunny_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weather_soc_table.cmp(&other.weather_soc_table) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Knobs {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Knobs {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.force_disable_export.encode_ueba(ctx, &mut buffer)?;
            value.export_soc_threshold.encode_ueba(ctx, &mut buffer)?;
            value.discharge_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.battery_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.full_charge_discharge_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.full_charge_export_soc_threshold.encode_ueba(ctx, &mut buffer)?;
            value.discharge_time.encode_ueba(ctx, &mut buffer)?;
            value.debug_full_charge.encode_ueba(ctx, &mut buffer)?;
            value.pessimism_multiplier_modifier.encode_ueba(ctx, &mut buffer)?;
            value.disable_night_grid_discharge.encode_ueba(ctx, &mut buffer)?;
            value.charge_car_boost.encode_ueba(ctx, &mut buffer)?;
            value.charge_car_extended_mode.encode_ueba(ctx, &mut buffer)?;
            value.zappi_current_target.encode_ueba(ctx, &mut buffer)?;
            value.zappi_limit.encode_ueba(ctx, &mut buffer)?;
            value.zappi_emergency_margin.encode_ueba(ctx, &mut buffer)?;
            value.grid_export_limit_w.encode_ueba(ctx, &mut buffer)?;
            value.grid_import_limit_w.encode_ueba(ctx, &mut buffer)?;
            value.allow_battery_to_car.encode_ueba(ctx, &mut buffer)?;
            value.eddi_enable_soc.encode_ueba(ctx, &mut buffer)?;
            value.eddi_disable_soc.encode_ueba(ctx, &mut buffer)?;
            value.eddi_dwell_s.encode_ueba(ctx, &mut buffer)?;
            value.weathersoc_winter_temperature_threshold.encode_ueba(ctx, &mut buffer)?;
            value.weathersoc_low_energy_threshold.encode_ueba(ctx, &mut buffer)?;
            value.weathersoc_ok_energy_threshold.encode_ueba(ctx, &mut buffer)?;
            value.weathersoc_high_energy_threshold.encode_ueba(ctx, &mut buffer)?;
            value.weathersoc_too_much_energy_threshold.encode_ueba(ctx, &mut buffer)?;
            value.writes_enabled.encode_ueba(ctx, &mut buffer)?;
            value.forecast_disagreement_strategy.encode_ueba(ctx, &mut buffer)?;
            value.charge_battery_extended_mode.encode_ueba(ctx, &mut buffer)?;
            value.export_soc_threshold_mode.encode_ueba(ctx, &mut buffer)?;
            value.discharge_soc_target_mode.encode_ueba(ctx, &mut buffer)?;
            value.battery_soc_target_mode.encode_ueba(ctx, &mut buffer)?;
            value.disable_night_grid_discharge_mode.encode_ueba(ctx, &mut buffer)?;
            value.inverter_safe_discharge_enable.encode_ueba(ctx, &mut buffer)?;
            value.baseline_winter_start_mm_dd.encode_ueba(ctx, &mut buffer)?;
            value.baseline_winter_end_mm_dd.encode_ueba(ctx, &mut buffer)?;
            value.baseline_wh_per_hour_winter.encode_ueba(ctx, &mut buffer)?;
            value.baseline_wh_per_hour_summer.encode_ueba(ctx, &mut buffer)?;
            value.keep_batteries_charged_during_full_charge.encode_ueba(ctx, &mut buffer)?;
            value.sunrise_sunset_offset_min.encode_ueba(ctx, &mut buffer)?;
            value.full_charge_defer_to_next_sunday.encode_ueba(ctx, &mut buffer)?;
            value.full_charge_snap_back_max_weekday.encode_ueba(ctx, &mut buffer)?;
            value.zappi_battery_drain_threshold_w.encode_ueba(ctx, &mut buffer)?;
            value.zappi_battery_drain_relax_step_w.encode_ueba(ctx, &mut buffer)?;
            value.zappi_battery_drain_kp.encode_ueba(ctx, &mut buffer)?;
            value.zappi_battery_drain_target_w.encode_ueba(ctx, &mut buffer)?;
            value.zappi_battery_drain_hard_clamp_w.encode_ueba(ctx, &mut buffer)?;
            value.zappi_battery_drain_mppt_probe_w.encode_ueba(ctx, &mut buffer)?;
            value.actuator_retry_s.encode_ueba(ctx, &mut buffer)?;
            value.weathersoc_very_sunny_threshold.encode_ueba(ctx, &mut buffer)?;
            value.weather_soc_table.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.force_disable_export.encode_ueba(ctx, writer)?;
            value.export_soc_threshold.encode_ueba(ctx, writer)?;
            value.discharge_soc_target.encode_ueba(ctx, writer)?;
            value.battery_soc_target.encode_ueba(ctx, writer)?;
            value.full_charge_discharge_soc_target.encode_ueba(ctx, writer)?;
            value.full_charge_export_soc_threshold.encode_ueba(ctx, writer)?;
            value.discharge_time.encode_ueba(ctx, writer)?;
            value.debug_full_charge.encode_ueba(ctx, writer)?;
            value.pessimism_multiplier_modifier.encode_ueba(ctx, writer)?;
            value.disable_night_grid_discharge.encode_ueba(ctx, writer)?;
            value.charge_car_boost.encode_ueba(ctx, writer)?;
            value.charge_car_extended_mode.encode_ueba(ctx, writer)?;
            value.zappi_current_target.encode_ueba(ctx, writer)?;
            value.zappi_limit.encode_ueba(ctx, writer)?;
            value.zappi_emergency_margin.encode_ueba(ctx, writer)?;
            value.grid_export_limit_w.encode_ueba(ctx, writer)?;
            value.grid_import_limit_w.encode_ueba(ctx, writer)?;
            value.allow_battery_to_car.encode_ueba(ctx, writer)?;
            value.eddi_enable_soc.encode_ueba(ctx, writer)?;
            value.eddi_disable_soc.encode_ueba(ctx, writer)?;
            value.eddi_dwell_s.encode_ueba(ctx, writer)?;
            value.weathersoc_winter_temperature_threshold.encode_ueba(ctx, writer)?;
            value.weathersoc_low_energy_threshold.encode_ueba(ctx, writer)?;
            value.weathersoc_ok_energy_threshold.encode_ueba(ctx, writer)?;
            value.weathersoc_high_energy_threshold.encode_ueba(ctx, writer)?;
            value.weathersoc_too_much_energy_threshold.encode_ueba(ctx, writer)?;
            value.writes_enabled.encode_ueba(ctx, writer)?;
            value.forecast_disagreement_strategy.encode_ueba(ctx, writer)?;
            value.charge_battery_extended_mode.encode_ueba(ctx, writer)?;
            value.export_soc_threshold_mode.encode_ueba(ctx, writer)?;
            value.discharge_soc_target_mode.encode_ueba(ctx, writer)?;
            value.battery_soc_target_mode.encode_ueba(ctx, writer)?;
            value.disable_night_grid_discharge_mode.encode_ueba(ctx, writer)?;
            value.inverter_safe_discharge_enable.encode_ueba(ctx, writer)?;
            value.baseline_winter_start_mm_dd.encode_ueba(ctx, writer)?;
            value.baseline_winter_end_mm_dd.encode_ueba(ctx, writer)?;
            value.baseline_wh_per_hour_winter.encode_ueba(ctx, writer)?;
            value.baseline_wh_per_hour_summer.encode_ueba(ctx, writer)?;
            value.keep_batteries_charged_during_full_charge.encode_ueba(ctx, writer)?;
            value.sunrise_sunset_offset_min.encode_ueba(ctx, writer)?;
            value.full_charge_defer_to_next_sunday.encode_ueba(ctx, writer)?;
            value.full_charge_snap_back_max_weekday.encode_ueba(ctx, writer)?;
            value.zappi_battery_drain_threshold_w.encode_ueba(ctx, writer)?;
            value.zappi_battery_drain_relax_step_w.encode_ueba(ctx, writer)?;
            value.zappi_battery_drain_kp.encode_ueba(ctx, writer)?;
            value.zappi_battery_drain_target_w.encode_ueba(ctx, writer)?;
            value.zappi_battery_drain_hard_clamp_w.encode_ueba(ctx, writer)?;
            value.zappi_battery_drain_mppt_probe_w.encode_ueba(ctx, writer)?;
            value.actuator_retry_s.encode_ueba(ctx, writer)?;
            value.weathersoc_very_sunny_threshold.encode_ueba(ctx, writer)?;
            value.weather_soc_table.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Knobs {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let force_disable_export = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let export_soc_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let discharge_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let battery_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let full_charge_discharge_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let full_charge_export_soc_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let discharge_time = DischargeTime::decode_ueba(ctx, reader)?;
        let debug_full_charge = DebugFullCharge::decode_ueba(ctx, reader)?;
        let pessimism_multiplier_modifier = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let disable_night_grid_discharge = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let charge_car_boost = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let charge_car_extended_mode = ExtendedChargeMode::decode_ueba(ctx, reader)?;
        let zappi_current_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let zappi_limit = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let zappi_emergency_margin = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let grid_export_limit_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let grid_import_limit_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let allow_battery_to_car = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let eddi_enable_soc = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let eddi_disable_soc = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let eddi_dwell_s = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let weathersoc_winter_temperature_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weathersoc_low_energy_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weathersoc_ok_energy_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weathersoc_high_energy_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weathersoc_too_much_energy_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let writes_enabled = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let forecast_disagreement_strategy = ForecastDisagreementStrategy::decode_ueba(ctx, reader)?;
        let charge_battery_extended_mode = ChargeBatteryExtendedMode::decode_ueba(ctx, reader)?;
        let export_soc_threshold_mode = Mode::decode_ueba(ctx, reader)?;
        let discharge_soc_target_mode = Mode::decode_ueba(ctx, reader)?;
        let battery_soc_target_mode = Mode::decode_ueba(ctx, reader)?;
        let disable_night_grid_discharge_mode = Mode::decode_ueba(ctx, reader)?;
        let inverter_safe_discharge_enable = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let baseline_winter_start_mm_dd = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let baseline_winter_end_mm_dd = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let baseline_wh_per_hour_winter = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let baseline_wh_per_hour_summer = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let keep_batteries_charged_during_full_charge = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let sunrise_sunset_offset_min = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let full_charge_defer_to_next_sunday = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let full_charge_snap_back_max_weekday = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let zappi_battery_drain_threshold_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let zappi_battery_drain_relax_step_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let zappi_battery_drain_kp = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let zappi_battery_drain_target_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let zappi_battery_drain_hard_clamp_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let zappi_battery_drain_mppt_probe_w = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let actuator_retry_s = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let weathersoc_very_sunny_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weather_soc_table = WeatherSocTable::decode_ueba(ctx, reader)?;
        Ok(Knobs {
            force_disable_export,
            export_soc_threshold,
            discharge_soc_target,
            battery_soc_target,
            full_charge_discharge_soc_target,
            full_charge_export_soc_threshold,
            discharge_time,
            debug_full_charge,
            pessimism_multiplier_modifier,
            disable_night_grid_discharge,
            charge_car_boost,
            charge_car_extended_mode,
            zappi_current_target,
            zappi_limit,
            zappi_emergency_margin,
            grid_export_limit_w,
            grid_import_limit_w,
            allow_battery_to_car,
            eddi_enable_soc,
            eddi_disable_soc,
            eddi_dwell_s,
            weathersoc_winter_temperature_threshold,
            weathersoc_low_energy_threshold,
            weathersoc_ok_energy_threshold,
            weathersoc_high_energy_threshold,
            weathersoc_too_much_energy_threshold,
            writes_enabled,
            forecast_disagreement_strategy,
            charge_battery_extended_mode,
            export_soc_threshold_mode,
            discharge_soc_target_mode,
            battery_soc_target_mode,
            disable_night_grid_discharge_mode,
            inverter_safe_discharge_enable,
            baseline_winter_start_mm_dd,
            baseline_winter_end_mm_dd,
            baseline_wh_per_hour_winter,
            baseline_wh_per_hour_summer,
            keep_batteries_charged_during_full_charge,
            sunrise_sunset_offset_min,
            full_charge_defer_to_next_sunday,
            full_charge_snap_back_max_weekday,
            zappi_battery_drain_threshold_w,
            zappi_battery_drain_relax_step_w,
            zappi_battery_drain_kp,
            zappi_battery_drain_target_w,
            zappi_battery_drain_hard_clamp_w,
            zappi_battery_drain_mppt_probe_w,
            actuator_retry_s,
            weathersoc_very_sunny_threshold,
            weather_soc_table,
        })
    }
}