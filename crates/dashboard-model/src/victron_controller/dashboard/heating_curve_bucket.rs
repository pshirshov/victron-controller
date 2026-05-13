

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HeatingCurveBucket {
    pub outdoor_max_c: f64,
    pub water_target_c: f64,
}

impl PartialEq for HeatingCurveBucket {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for HeatingCurveBucket {}

impl PartialOrd for HeatingCurveBucket {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeatingCurveBucket {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.outdoor_max_c.total_cmp(&other.outdoor_max_c) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.water_target_c.total_cmp(&other.water_target_c) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for HeatingCurveBucket {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for HeatingCurveBucket {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.outdoor_max_c.encode_ueba(ctx, &mut buffer)?;
            value.water_target_c.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.outdoor_max_c.encode_ueba(ctx, writer)?;
            value.water_target_c.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for HeatingCurveBucket {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let outdoor_max_c = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let water_target_c = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        Ok(HeatingCurveBucket {
            outdoor_max_c,
            water_target_c,
        })
    }
}