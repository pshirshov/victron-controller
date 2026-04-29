use crate::victron_controller::dashboard::actual_f64::ActualF64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Sensors {
    pub battery_soc: ActualF64,
    pub battery_soh: ActualF64,
    pub battery_installed_capacity: ActualF64,
    pub battery_dc_power: ActualF64,
    pub mppt_power_0: ActualF64,
    pub mppt_power_1: ActualF64,
    pub soltaro_power: ActualF64,
    pub power_consumption: ActualF64,
    pub grid_power: ActualF64,
    pub grid_voltage: ActualF64,
    pub grid_current: ActualF64,
    pub consumption_current: ActualF64,
    pub offgrid_power: ActualF64,
    pub offgrid_current: ActualF64,
    pub vebus_input_current: ActualF64,
    pub evcharger_ac_power: ActualF64,
    pub evcharger_ac_current: ActualF64,
    pub ess_state: ActualF64,
    pub outdoor_temperature: ActualF64,
    pub session_kwh: ActualF64,
    pub ev_soc: ActualF64,
    pub ev_charge_target: ActualF64,
    pub heat_pump_power: ActualF64,
    pub cooker_power: ActualF64,
    pub mppt_0_operation_mode: ActualF64,
    pub mppt_1_operation_mode: ActualF64,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for Sensors {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        26
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Sensors {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.battery_soc.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.battery_soh.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.battery_installed_capacity.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.battery_dc_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.mppt_power_0.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.mppt_power_1.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.soltaro_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.power_consumption.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.grid_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.grid_voltage.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.grid_current.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.consumption_current.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.offgrid_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.offgrid_current.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.vebus_input_current.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.evcharger_ac_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.evcharger_ac_current.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.ess_state.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.outdoor_temperature.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.session_kwh.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.ev_soc.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.ev_charge_target.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.heat_pump_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.cooker_power.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.mppt_0_operation_mode.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.mppt_1_operation_mode.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.battery_soc.encode_ueba(ctx, writer)?;
            value.battery_soh.encode_ueba(ctx, writer)?;
            value.battery_installed_capacity.encode_ueba(ctx, writer)?;
            value.battery_dc_power.encode_ueba(ctx, writer)?;
            value.mppt_power_0.encode_ueba(ctx, writer)?;
            value.mppt_power_1.encode_ueba(ctx, writer)?;
            value.soltaro_power.encode_ueba(ctx, writer)?;
            value.power_consumption.encode_ueba(ctx, writer)?;
            value.grid_power.encode_ueba(ctx, writer)?;
            value.grid_voltage.encode_ueba(ctx, writer)?;
            value.grid_current.encode_ueba(ctx, writer)?;
            value.consumption_current.encode_ueba(ctx, writer)?;
            value.offgrid_power.encode_ueba(ctx, writer)?;
            value.offgrid_current.encode_ueba(ctx, writer)?;
            value.vebus_input_current.encode_ueba(ctx, writer)?;
            value.evcharger_ac_power.encode_ueba(ctx, writer)?;
            value.evcharger_ac_current.encode_ueba(ctx, writer)?;
            value.ess_state.encode_ueba(ctx, writer)?;
            value.outdoor_temperature.encode_ueba(ctx, writer)?;
            value.session_kwh.encode_ueba(ctx, writer)?;
            value.ev_soc.encode_ueba(ctx, writer)?;
            value.ev_charge_target.encode_ueba(ctx, writer)?;
            value.heat_pump_power.encode_ueba(ctx, writer)?;
            value.cooker_power.encode_ueba(ctx, writer)?;
            value.mppt_0_operation_mode.encode_ueba(ctx, writer)?;
            value.mppt_1_operation_mode.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Sensors {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let battery_soc = ActualF64::decode_ueba(ctx, reader)?;
        let battery_soh = ActualF64::decode_ueba(ctx, reader)?;
        let battery_installed_capacity = ActualF64::decode_ueba(ctx, reader)?;
        let battery_dc_power = ActualF64::decode_ueba(ctx, reader)?;
        let mppt_power_0 = ActualF64::decode_ueba(ctx, reader)?;
        let mppt_power_1 = ActualF64::decode_ueba(ctx, reader)?;
        let soltaro_power = ActualF64::decode_ueba(ctx, reader)?;
        let power_consumption = ActualF64::decode_ueba(ctx, reader)?;
        let grid_power = ActualF64::decode_ueba(ctx, reader)?;
        let grid_voltage = ActualF64::decode_ueba(ctx, reader)?;
        let grid_current = ActualF64::decode_ueba(ctx, reader)?;
        let consumption_current = ActualF64::decode_ueba(ctx, reader)?;
        let offgrid_power = ActualF64::decode_ueba(ctx, reader)?;
        let offgrid_current = ActualF64::decode_ueba(ctx, reader)?;
        let vebus_input_current = ActualF64::decode_ueba(ctx, reader)?;
        let evcharger_ac_power = ActualF64::decode_ueba(ctx, reader)?;
        let evcharger_ac_current = ActualF64::decode_ueba(ctx, reader)?;
        let ess_state = ActualF64::decode_ueba(ctx, reader)?;
        let outdoor_temperature = ActualF64::decode_ueba(ctx, reader)?;
        let session_kwh = ActualF64::decode_ueba(ctx, reader)?;
        let ev_soc = ActualF64::decode_ueba(ctx, reader)?;
        let ev_charge_target = ActualF64::decode_ueba(ctx, reader)?;
        let heat_pump_power = ActualF64::decode_ueba(ctx, reader)?;
        let cooker_power = ActualF64::decode_ueba(ctx, reader)?;
        let mppt_0_operation_mode = ActualF64::decode_ueba(ctx, reader)?;
        let mppt_1_operation_mode = ActualF64::decode_ueba(ctx, reader)?;
        Ok(Sensors {
            battery_soc,
            battery_soh,
            battery_installed_capacity,
            battery_dc_power,
            mppt_power_0,
            mppt_power_1,
            soltaro_power,
            power_consumption,
            grid_power,
            grid_voltage,
            grid_current,
            consumption_current,
            offgrid_power,
            offgrid_current,
            vebus_input_current,
            evcharger_ac_power,
            evcharger_ac_current,
            ess_state,
            outdoor_temperature,
            session_kwh,
            ev_soc,
            ev_charge_target,
            heat_pump_power,
            cooker_power,
            mppt_0_operation_mode,
            mppt_1_operation_mode,
        })
    }
}