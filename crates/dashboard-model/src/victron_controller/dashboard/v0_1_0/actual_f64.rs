use crate::victron_controller::dashboard::v0_1_0::freshness::Freshness;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActualF64 {
    pub value: Option<f64>,
    pub freshness: Freshness,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub since_epoch_ms: i64,
}

impl PartialEq for ActualF64 {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ActualF64 {}

impl PartialOrd for ActualF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ActualF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.value, &other.value) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.freshness.cmp(&other.freshness) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.since_epoch_ms.cmp(&other.since_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ActualF64 {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ActualF64 {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.value {
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
            value.freshness.encode_ueba(ctx, &mut buffer)?;
            value.since_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            match &value.value {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.freshness.encode_ueba(ctx, writer)?;
            value.since_epoch_ms.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ActualF64 {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        let freshness = Freshness::decode_ueba(ctx, reader)?;
        let since_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        Ok(ActualF64 {
            value,
            freshness,
            since_epoch_ms,
        })
    }
}