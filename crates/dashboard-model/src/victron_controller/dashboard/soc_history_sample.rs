

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SocHistorySample {
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub epoch_ms: i64,
    pub soc_pct: f64,
}

impl PartialEq for SocHistorySample {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SocHistorySample {}

impl PartialOrd for SocHistorySample {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SocHistorySample {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.epoch_ms.cmp(&other.epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.soc_pct.total_cmp(&other.soc_pct) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SocHistorySample {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SocHistorySample {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.epoch_ms.encode_ueba(ctx, &mut buffer)?;
            value.soc_pct.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.epoch_ms.encode_ueba(ctx, writer)?;
            value.soc_pct.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SocHistorySample {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let soc_pct = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        Ok(SocHistorySample {
            epoch_ms,
            soc_pct,
        })
    }
}