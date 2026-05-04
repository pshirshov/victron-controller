

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WeatherSocCell {
    pub export_soc_threshold: f64,
    pub battery_soc_target: f64,
    pub discharge_soc_target: f64,
    pub extended: bool,
}

impl PartialEq for WeatherSocCell {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for WeatherSocCell {}

impl PartialOrd for WeatherSocCell {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeatherSocCell {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.export_soc_threshold.total_cmp(&other.export_soc_threshold) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.battery_soc_target.total_cmp(&other.battery_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.discharge_soc_target.total_cmp(&other.discharge_soc_target) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.extended.cmp(&other.extended) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for WeatherSocCell {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WeatherSocCell {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.export_soc_threshold.encode_ueba(ctx, &mut buffer)?;
            value.battery_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.discharge_soc_target.encode_ueba(ctx, &mut buffer)?;
            value.extended.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.export_soc_threshold.encode_ueba(ctx, writer)?;
            value.battery_soc_target.encode_ueba(ctx, writer)?;
            value.discharge_soc_target.encode_ueba(ctx, writer)?;
            value.extended.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WeatherSocCell {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let export_soc_threshold = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let battery_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let discharge_soc_target = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let extended = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        Ok(WeatherSocCell {
            export_soc_threshold,
            battery_soc_target,
            discharge_soc_target,
            extended,
        })
    }
}