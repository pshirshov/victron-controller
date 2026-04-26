use crate::victron_controller::dashboard::v0_2_0::actual_f64::ActualF64;
use crate::victron_controller::dashboard::v0_2_0::owner::Owner;
use crate::victron_controller::dashboard::v0_2_0::target_phase::TargetPhase;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActuatedF64 {
    pub target_value: Option<f64>,
    pub target_owner: Owner,
    pub target_phase: TargetPhase,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub target_since_epoch_ms: i64,
    pub actual: ActualF64,
}

impl PartialEq for ActuatedF64 {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ActuatedF64 {}

impl PartialOrd for ActuatedF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ActuatedF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.target_value, &other.target_value) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.target_owner.cmp(&other.target_owner) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.target_phase.cmp(&other.target_phase) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.target_since_epoch_ms.cmp(&other.target_since_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.actual.cmp(&other.actual) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ActuatedF64 {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        2
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ActuatedF64 {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.target_value {
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
                value.actual.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            match &value.target_value {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.target_owner.encode_ueba(ctx, writer)?;
            value.target_phase.encode_ueba(ctx, writer)?;
            value.target_since_epoch_ms.encode_ueba(ctx, writer)?;
            value.actual.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ActuatedF64 {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let target_value = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        let target_owner = Owner::decode_ueba(ctx, reader)?;
        let target_phase = TargetPhase::decode_ueba(ctx, reader)?;
        let target_since_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let actual = ActualF64::decode_ueba(ctx, reader)?;
        Ok(ActuatedF64 {
            target_value,
            target_owner,
            target_phase,
            target_since_epoch_ms,
            actual,
        })
    }
}