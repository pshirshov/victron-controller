use crate::victron_controller::dashboard::zappi_drain_branch::ZappiDrainBranch;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZappiDrainSample {
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub captured_at_epoch_ms: i64,
    pub compensated_drain_w: f64,
    pub branch: ZappiDrainBranch,
    pub hard_clamp_engaged: bool,
}

impl PartialEq for ZappiDrainSample {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ZappiDrainSample {}

impl PartialOrd for ZappiDrainSample {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ZappiDrainSample {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.captured_at_epoch_ms.cmp(&other.captured_at_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.compensated_drain_w.total_cmp(&other.compensated_drain_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.branch.cmp(&other.branch) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.hard_clamp_engaged.cmp(&other.hard_clamp_engaged) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ZappiDrainSample {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ZappiDrainSample {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.captured_at_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            value.compensated_drain_w.encode_ueba(ctx, &mut buffer)?;
            value.branch.encode_ueba(ctx, &mut buffer)?;
            value.hard_clamp_engaged.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.captured_at_epoch_ms.encode_ueba(ctx, writer)?;
            value.compensated_drain_w.encode_ueba(ctx, writer)?;
            value.branch.encode_ueba(ctx, writer)?;
            value.hard_clamp_engaged.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ZappiDrainSample {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let captured_at_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let compensated_drain_w = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let branch = ZappiDrainBranch::decode_ueba(ctx, reader)?;
        let hard_clamp_engaged = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        Ok(ZappiDrainSample {
            captured_at_epoch_ms,
            compensated_drain_w,
            branch,
            hard_clamp_engaged,
        })
    }
}