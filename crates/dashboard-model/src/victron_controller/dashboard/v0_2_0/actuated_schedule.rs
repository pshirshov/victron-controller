use crate::victron_controller::dashboard::v0_2_0::freshness::Freshness;
use crate::victron_controller::dashboard::v0_2_0::owner::Owner;
use crate::victron_controller::dashboard::v0_2_0::schedule_spec::ScheduleSpec;
use crate::victron_controller::dashboard::v0_2_0::target_phase::TargetPhase;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct ActuatedSchedule {
    pub target: Option<ScheduleSpec>,
    pub target_owner: Owner,
    pub target_phase: TargetPhase,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub target_since_epoch_ms: i64,
    pub actual: Option<ScheduleSpec>,
    pub actual_freshness: Freshness,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub actual_since_epoch_ms: i64,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for ActuatedSchedule {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        2
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ActuatedSchedule {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.target {
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
            value.target_owner.encode_ueba(ctx, &mut buffer)?;
            value.target_phase.encode_ueba(ctx, &mut buffer)?;
            value.target_since_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.actual {
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
            value.actual_freshness.encode_ueba(ctx, &mut buffer)?;
            value.actual_since_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            match &value.target {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.target_owner.encode_ueba(ctx, writer)?;
            value.target_phase.encode_ueba(ctx, writer)?;
            value.target_since_epoch_ms.encode_ueba(ctx, writer)?;
            match &value.actual {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.actual_freshness.encode_ueba(ctx, writer)?;
            value.actual_since_epoch_ms.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ActuatedSchedule {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let target = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(ScheduleSpec::decode_ueba(ctx, reader)?) }
        };
        let target_owner = Owner::decode_ueba(ctx, reader)?;
        let target_phase = TargetPhase::decode_ueba(ctx, reader)?;
        let target_since_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let actual = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(ScheduleSpec::decode_ueba(ctx, reader)?) }
        };
        let actual_freshness = Freshness::decode_ueba(ctx, reader)?;
        let actual_since_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        Ok(ActuatedSchedule {
            target,
            target_owner,
            target_phase,
            target_since_epoch_ms,
            actual,
            actual_freshness,
            actual_since_epoch_ms,
        })
    }
}