use crate::victron_controller::dashboard::weather_soc_cell::WeatherSocCell;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct WeatherSocTable {
    pub very_sunny_warm: WeatherSocCell,
    pub very_sunny_cold: WeatherSocCell,
    pub sunny_warm: WeatherSocCell,
    pub sunny_cold: WeatherSocCell,
    pub mid_warm: WeatherSocCell,
    pub mid_cold: WeatherSocCell,
    pub low_warm: WeatherSocCell,
    pub low_cold: WeatherSocCell,
    pub dim_warm: WeatherSocCell,
    pub dim_cold: WeatherSocCell,
    pub very_dim_warm: WeatherSocCell,
    pub very_dim_cold: WeatherSocCell,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for WeatherSocTable {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WeatherSocTable {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.very_sunny_warm.encode_ueba(ctx, &mut buffer)?;
            value.very_sunny_cold.encode_ueba(ctx, &mut buffer)?;
            value.sunny_warm.encode_ueba(ctx, &mut buffer)?;
            value.sunny_cold.encode_ueba(ctx, &mut buffer)?;
            value.mid_warm.encode_ueba(ctx, &mut buffer)?;
            value.mid_cold.encode_ueba(ctx, &mut buffer)?;
            value.low_warm.encode_ueba(ctx, &mut buffer)?;
            value.low_cold.encode_ueba(ctx, &mut buffer)?;
            value.dim_warm.encode_ueba(ctx, &mut buffer)?;
            value.dim_cold.encode_ueba(ctx, &mut buffer)?;
            value.very_dim_warm.encode_ueba(ctx, &mut buffer)?;
            value.very_dim_cold.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.very_sunny_warm.encode_ueba(ctx, writer)?;
            value.very_sunny_cold.encode_ueba(ctx, writer)?;
            value.sunny_warm.encode_ueba(ctx, writer)?;
            value.sunny_cold.encode_ueba(ctx, writer)?;
            value.mid_warm.encode_ueba(ctx, writer)?;
            value.mid_cold.encode_ueba(ctx, writer)?;
            value.low_warm.encode_ueba(ctx, writer)?;
            value.low_cold.encode_ueba(ctx, writer)?;
            value.dim_warm.encode_ueba(ctx, writer)?;
            value.dim_cold.encode_ueba(ctx, writer)?;
            value.very_dim_warm.encode_ueba(ctx, writer)?;
            value.very_dim_cold.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WeatherSocTable {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let very_sunny_warm = WeatherSocCell::decode_ueba(ctx, reader)?;
        let very_sunny_cold = WeatherSocCell::decode_ueba(ctx, reader)?;
        let sunny_warm = WeatherSocCell::decode_ueba(ctx, reader)?;
        let sunny_cold = WeatherSocCell::decode_ueba(ctx, reader)?;
        let mid_warm = WeatherSocCell::decode_ueba(ctx, reader)?;
        let mid_cold = WeatherSocCell::decode_ueba(ctx, reader)?;
        let low_warm = WeatherSocCell::decode_ueba(ctx, reader)?;
        let low_cold = WeatherSocCell::decode_ueba(ctx, reader)?;
        let dim_warm = WeatherSocCell::decode_ueba(ctx, reader)?;
        let dim_cold = WeatherSocCell::decode_ueba(ctx, reader)?;
        let very_dim_warm = WeatherSocCell::decode_ueba(ctx, reader)?;
        let very_dim_cold = WeatherSocCell::decode_ueba(ctx, reader)?;
        Ok(WeatherSocTable {
            very_sunny_warm,
            very_sunny_cold,
            sunny_warm,
            sunny_cold,
            mid_warm,
            mid_cold,
            low_warm,
            low_cold,
            dim_warm,
            dim_cold,
            very_dim_warm,
            very_dim_cold,
        })
    }
}