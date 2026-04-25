use crate::victron_controller::dashboard::soc_projection_kind::SocProjectionKind;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SocProjectionSegment {
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub start_epoch_ms: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub end_epoch_ms: i64,
    pub start_soc_pct: f64,
    pub end_soc_pct: f64,
    pub kind: SocProjectionKind,
}

impl PartialEq for SocProjectionSegment {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SocProjectionSegment {}

impl PartialOrd for SocProjectionSegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SocProjectionSegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.start_epoch_ms.cmp(&other.start_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.end_epoch_ms.cmp(&other.end_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.start_soc_pct.total_cmp(&other.start_soc_pct) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.end_soc_pct.total_cmp(&other.end_soc_pct) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.kind.cmp(&other.kind) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SocProjectionSegment {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SocProjectionSegment {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.start_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            value.end_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            value.start_soc_pct.encode_ueba(ctx, &mut buffer)?;
            value.end_soc_pct.encode_ueba(ctx, &mut buffer)?;
            value.kind.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.start_epoch_ms.encode_ueba(ctx, writer)?;
            value.end_epoch_ms.encode_ueba(ctx, writer)?;
            value.start_soc_pct.encode_ueba(ctx, writer)?;
            value.end_soc_pct.encode_ueba(ctx, writer)?;
            value.kind.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SocProjectionSegment {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let start_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let end_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let start_soc_pct = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let end_soc_pct = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let kind = SocProjectionKind::decode_ueba(ctx, reader)?;
        Ok(SocProjectionSegment {
            start_epoch_ms,
            end_epoch_ms,
            start_soc_pct,
            end_soc_pct,
            kind,
        })
    }
}