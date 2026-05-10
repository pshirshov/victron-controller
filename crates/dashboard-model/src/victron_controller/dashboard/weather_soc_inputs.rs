use crate::victron_controller::dashboard::weather_soc_day::WeatherSocDay;
use crate::victron_controller::dashboard::weather_soc_temperature_source::WeatherSocTemperatureSource;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WeatherSocInputs {
    pub temperature_c: f64,
    pub temperature_source: WeatherSocTemperatureSource,
    pub energy_kwh: f64,
    pub day: WeatherSocDay,
}

impl PartialEq for WeatherSocInputs {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for WeatherSocInputs {}

impl PartialOrd for WeatherSocInputs {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeatherSocInputs {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.temperature_c.total_cmp(&other.temperature_c) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.temperature_source.cmp(&other.temperature_source) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.energy_kwh.total_cmp(&other.energy_kwh) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.day.cmp(&other.day) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for WeatherSocInputs {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WeatherSocInputs {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.temperature_c.encode_ueba(ctx, &mut buffer)?;
            value.temperature_source.encode_ueba(ctx, &mut buffer)?;
            value.energy_kwh.encode_ueba(ctx, &mut buffer)?;
            value.day.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.temperature_c.encode_ueba(ctx, writer)?;
            value.temperature_source.encode_ueba(ctx, writer)?;
            value.energy_kwh.encode_ueba(ctx, writer)?;
            value.day.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WeatherSocInputs {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let temperature_c = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let temperature_source = WeatherSocTemperatureSource::decode_ueba(ctx, reader)?;
        let energy_kwh = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let day = WeatherSocDay::decode_ueba(ctx, reader)?;
        Ok(WeatherSocInputs {
            temperature_c,
            temperature_source,
            energy_kwh,
            day,
        })
    }
}