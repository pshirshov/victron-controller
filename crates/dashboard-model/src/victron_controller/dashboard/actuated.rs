use crate::victron_controller::dashboard::actuated_enum_name::ActuatedEnumName;
use crate::victron_controller::dashboard::actuated_f64::ActuatedF64;
use crate::victron_controller::dashboard::actuated_i32::ActuatedI32;
use crate::victron_controller::dashboard::actuated_schedule::ActuatedSchedule;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Actuated {
    pub grid_setpoint: ActuatedI32,
    pub input_current_limit: ActuatedF64,
    pub zappi_mode: ActuatedEnumName,
    pub eddi_mode: ActuatedEnumName,
    pub schedule_0: ActuatedSchedule,
    pub schedule_1: ActuatedSchedule,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for Actuated {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        6
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Actuated {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.grid_setpoint.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.input_current_limit.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.zappi_mode.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.eddi_mode.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.schedule_0.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.schedule_1.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.grid_setpoint.encode_ueba(ctx, writer)?;
            value.input_current_limit.encode_ueba(ctx, writer)?;
            value.zappi_mode.encode_ueba(ctx, writer)?;
            value.eddi_mode.encode_ueba(ctx, writer)?;
            value.schedule_0.encode_ueba(ctx, writer)?;
            value.schedule_1.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Actuated {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let grid_setpoint = ActuatedI32::decode_ueba(ctx, reader)?;
        let input_current_limit = ActuatedF64::decode_ueba(ctx, reader)?;
        let zappi_mode = ActuatedEnumName::decode_ueba(ctx, reader)?;
        let eddi_mode = ActuatedEnumName::decode_ueba(ctx, reader)?;
        let schedule_0 = ActuatedSchedule::decode_ueba(ctx, reader)?;
        let schedule_1 = ActuatedSchedule::decode_ueba(ctx, reader)?;
        Ok(Actuated {
            grid_setpoint,
            input_current_limit,
            zappi_mode,
            eddi_mode,
            schedule_0,
            schedule_1,
        })
    }
}