use crate::victron_controller::dashboard::actuated::Actuated;
use crate::victron_controller::dashboard::bookkeeping::Bookkeeping;
use crate::victron_controller::dashboard::forecasts::Forecasts;
use crate::victron_controller::dashboard::knobs::Knobs;
use crate::victron_controller::dashboard::sensors::Sensors;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct WorldSnapshot {
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub captured_at_epoch_ms: i64,
    pub captured_at_naive_iso: String,
    pub sensors: Sensors,
    pub actuated: Actuated,
    pub knobs: Knobs,
    pub bookkeeping: Bookkeeping,
    pub forecasts: Forecasts,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for WorldSnapshot {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        5
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WorldSnapshot {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.captured_at_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.captured_at_naive_iso.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.sensors.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.actuated.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.knobs.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.bookkeeping.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.forecasts.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.captured_at_epoch_ms.encode_ueba(ctx, writer)?;
            value.captured_at_naive_iso.encode_ueba(ctx, writer)?;
            value.sensors.encode_ueba(ctx, writer)?;
            value.actuated.encode_ueba(ctx, writer)?;
            value.knobs.encode_ueba(ctx, writer)?;
            value.bookkeeping.encode_ueba(ctx, writer)?;
            value.forecasts.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WorldSnapshot {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let captured_at_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let captured_at_naive_iso = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let sensors = Sensors::decode_ueba(ctx, reader)?;
        let actuated = Actuated::decode_ueba(ctx, reader)?;
        let knobs = Knobs::decode_ueba(ctx, reader)?;
        let bookkeeping = Bookkeeping::decode_ueba(ctx, reader)?;
        let forecasts = Forecasts::decode_ueba(ctx, reader)?;
        Ok(WorldSnapshot {
            captured_at_epoch_ms,
            captured_at_naive_iso,
            sensors,
            actuated,
            knobs,
            bookkeeping,
            forecasts,
        })
    }
}