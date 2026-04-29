

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Bookkeeping {
    pub next_full_charge_iso: Option<String>,
    pub above_soc_date_iso: Option<String>,
    pub zappi_active: bool,
    pub charge_to_full_required: bool,
    pub soc_end_of_day_target: f64,
    pub effective_export_soc_threshold: f64,
    pub battery_selected_soc_target: f64,
    pub charge_battery_extended_today: bool,
    pub charge_battery_extended_today_date_iso: Option<String>,
    pub weather_soc_export_soc_threshold: f64,
    pub weather_soc_discharge_soc_target: f64,
    pub weather_soc_battery_soc_target: f64,
    pub weather_soc_disable_night_grid_discharge: bool,
    pub auto_extended_today: bool,
    pub auto_extended_today_date_iso: Option<String>,
}

impl PartialEq for Bookkeeping {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Bookkeeping {}

impl PartialOrd for Bookkeeping {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bookkeeping {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.next_full_charge_iso.cmp(&other.next_full_charge_iso) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.above_soc_date_iso.cmp(&other.above_soc_date_iso) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.zappi_active.cmp(&other.zappi_active) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.charge_to_full_required.cmp(&other.charge_to_full_required) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.soc_end_of_day_target.total_cmp(&other.soc_end_of_day_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.effective_export_soc_threshold.total_cmp(&other.effective_export_soc_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.battery_selected_soc_target.total_cmp(&other.battery_selected_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.charge_battery_extended_today.cmp(&other.charge_battery_extended_today) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.charge_battery_extended_today_date_iso.cmp(&other.charge_battery_extended_today_date_iso) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weather_soc_export_soc_threshold.total_cmp(&other.weather_soc_export_soc_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weather_soc_discharge_soc_target.total_cmp(&other.weather_soc_discharge_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weather_soc_battery_soc_target.total_cmp(&other.weather_soc_battery_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.weather_soc_disable_night_grid_discharge.cmp(&other.weather_soc_disable_night_grid_discharge) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.auto_extended_today.cmp(&other.auto_extended_today) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.auto_extended_today_date_iso.cmp(&other.auto_extended_today_date_iso) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Bookkeeping {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        4
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Bookkeeping {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.next_full_charge_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 1)?;
                    v.encode_ueba(ctx, &mut buffer)?;
                }
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.above_soc_date_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 1)?;
                    v.encode_ueba(ctx, &mut buffer)?;
                }
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.zappi_active.encode_ueba(ctx, &mut buffer)?;
            value.charge_to_full_required.encode_ueba(ctx, &mut buffer)?;
            value.soc_end_of_day_target.encode_ueba(ctx, &mut buffer)?;
            value.effective_export_soc_threshold.encode_ueba(ctx, &mut buffer)?;
            value.battery_selected_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.charge_battery_extended_today.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.charge_battery_extended_today_date_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 1)?;
                    v.encode_ueba(ctx, &mut buffer)?;
                }
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.weather_soc_export_soc_threshold.encode_ueba(ctx, &mut buffer)?;
            value.weather_soc_discharge_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.weather_soc_battery_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.weather_soc_disable_night_grid_discharge.encode_ueba(ctx, &mut buffer)?;
            value.auto_extended_today.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.auto_extended_today_date_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 1)?;
                    v.encode_ueba(ctx, &mut buffer)?;
                }
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            match &value.next_full_charge_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            match &value.above_soc_date_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.zappi_active.encode_ueba(ctx, writer)?;
            value.charge_to_full_required.encode_ueba(ctx, writer)?;
            value.soc_end_of_day_target.encode_ueba(ctx, writer)?;
            value.effective_export_soc_threshold.encode_ueba(ctx, writer)?;
            value.battery_selected_soc_target.encode_ueba(ctx, writer)?;
            value.charge_battery_extended_today.encode_ueba(ctx, writer)?;
            match &value.charge_battery_extended_today_date_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.weather_soc_export_soc_threshold.encode_ueba(ctx, writer)?;
            value.weather_soc_discharge_soc_target.encode_ueba(ctx, writer)?;
            value.weather_soc_battery_soc_target.encode_ueba(ctx, writer)?;
            value.weather_soc_disable_night_grid_discharge.encode_ueba(ctx, writer)?;
            value.auto_extended_today.encode_ueba(ctx, writer)?;
            match &value.auto_extended_today_date_iso {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Bookkeeping {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let next_full_charge_iso = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_string(reader)?) }
        };
        let above_soc_date_iso = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_string(reader)?) }
        };
        let zappi_active = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let charge_to_full_required = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let soc_end_of_day_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let effective_export_soc_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let battery_selected_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let charge_battery_extended_today = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let charge_battery_extended_today_date_iso = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_string(reader)?) }
        };
        let weather_soc_export_soc_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weather_soc_discharge_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weather_soc_battery_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let weather_soc_disable_night_grid_discharge = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let auto_extended_today = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        let auto_extended_today_date_iso = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_string(reader)?) }
        };
        Ok(Bookkeeping {
            next_full_charge_iso,
            above_soc_date_iso,
            zappi_active,
            charge_to_full_required,
            soc_end_of_day_target,
            effective_export_soc_threshold,
            battery_selected_soc_target,
            charge_battery_extended_today,
            charge_battery_extended_today_date_iso,
            weather_soc_export_soc_threshold,
            weather_soc_discharge_soc_target,
            weather_soc_battery_soc_target,
            weather_soc_disable_night_grid_discharge,
            auto_extended_today,
            auto_extended_today_date_iso,
        })
    }
}