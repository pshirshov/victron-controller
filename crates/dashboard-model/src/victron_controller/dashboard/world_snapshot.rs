use crate::victron_controller::dashboard::actuated::Actuated;
use crate::victron_controller::dashboard::bookkeeping::Bookkeeping;
use crate::victron_controller::dashboard::cores_state::CoresState;
use crate::victron_controller::dashboard::decisions::Decisions;
use crate::victron_controller::dashboard::forecasts::Forecasts;
use crate::victron_controller::dashboard::knobs::Knobs;
use crate::victron_controller::dashboard::sensor_meta::SensorMeta;
use crate::victron_controller::dashboard::sensors::Sensors;
use crate::victron_controller::dashboard::timers::Timers;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct WorldSnapshot {
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub captured_at_epoch_ms: i64,
    pub captured_at_naive_iso: String,
    pub sensors: Sensors,
    pub sensors_meta: BTreeMap<String, SensorMeta>,
    pub actuated: Actuated,
    pub knobs: Knobs,
    pub bookkeeping: Bookkeeping,
    pub forecasts: Forecasts,
    pub decisions: Decisions,
    pub cores_state: CoresState,
    pub timers: Timers,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for WorldSnapshot {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        9
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
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.sensors_meta.len() as i32)?;
            for (k, v) in (value.sensors_meta).iter() {
                k.encode_ueba(ctx, &mut buffer)?;
                v.encode_ueba(ctx, &mut buffer)?;
            }
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
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.decisions.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.cores_state.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.timers.encode_ueba(ctx, &mut buffer)?;
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
            crate::baboon_runtime::bin_tools::write_i32(writer, value.sensors_meta.len() as i32)?;
            for (k, v) in (value.sensors_meta).iter() {
                k.encode_ueba(ctx, writer)?;
                v.encode_ueba(ctx, writer)?;
            }
            value.actuated.encode_ueba(ctx, writer)?;
            value.knobs.encode_ueba(ctx, writer)?;
            value.bookkeeping.encode_ueba(ctx, writer)?;
            value.forecasts.encode_ueba(ctx, writer)?;
            value.decisions.encode_ueba(ctx, writer)?;
            value.cores_state.encode_ueba(ctx, writer)?;
            value.timers.encode_ueba(ctx, writer)?;
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
        let sensors_meta = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| {
                let k = crate::baboon_runtime::bin_tools::read_string(reader)?;
                let v = SensorMeta::decode_ueba(ctx, reader)?;
                Ok((k, v))
            }).collect::<Result<std::collections::BTreeMap<_, _>, Box<dyn std::error::Error>>>()?
        };
        let actuated = Actuated::decode_ueba(ctx, reader)?;
        let knobs = Knobs::decode_ueba(ctx, reader)?;
        let bookkeeping = Bookkeeping::decode_ueba(ctx, reader)?;
        let forecasts = Forecasts::decode_ueba(ctx, reader)?;
        let decisions = Decisions::decode_ueba(ctx, reader)?;
        let cores_state = CoresState::decode_ueba(ctx, reader)?;
        let timers = Timers::decode_ueba(ctx, reader)?;
        Ok(WorldSnapshot {
            captured_at_epoch_ms,
            captured_at_naive_iso,
            sensors,
            sensors_meta,
            actuated,
            knobs,
            bookkeeping,
            forecasts,
            decisions,
            cores_state,
            timers,
        })
    }
}