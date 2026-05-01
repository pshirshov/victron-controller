use crate::victron_controller::dashboard::typed_sensor_enum::TypedSensorEnum;
use crate::victron_controller::dashboard::typed_sensor_string::TypedSensorString;
use crate::victron_controller::dashboard::typed_sensor_zappi::TypedSensorZappi;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct TypedSensors {
    pub eddi_mode: TypedSensorEnum,
    pub zappi: TypedSensorZappi,
    pub timezone: TypedSensorString,
    pub sunrise: TypedSensorString,
    pub sunset: TypedSensorString,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for TypedSensors {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        5
    }
}

impl crate::baboon_runtime::BaboonBinEncode for TypedSensors {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
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
                value.zappi.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.timezone.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.sunrise.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.sunset.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.eddi_mode.encode_ueba(ctx, writer)?;
            value.zappi.encode_ueba(ctx, writer)?;
            value.timezone.encode_ueba(ctx, writer)?;
            value.sunrise.encode_ueba(ctx, writer)?;
            value.sunset.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for TypedSensors {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let eddi_mode = TypedSensorEnum::decode_ueba(ctx, reader)?;
        let zappi = TypedSensorZappi::decode_ueba(ctx, reader)?;
        let timezone = TypedSensorString::decode_ueba(ctx, reader)?;
        let sunrise = TypedSensorString::decode_ueba(ctx, reader)?;
        let sunset = TypedSensorString::decode_ueba(ctx, reader)?;
        Ok(TypedSensors {
            eddi_mode,
            zappi,
            timezone,
            sunrise,
            sunset,
        })
    }
}