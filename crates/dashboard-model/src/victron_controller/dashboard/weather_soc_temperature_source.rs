

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum WeatherSocTemperatureSource {
    Forecast,
    Sensor,
}

impl WeatherSocTemperatureSource {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Forecast" => Ok(WeatherSocTemperatureSource::Forecast),
            "Sensor" => Ok(WeatherSocTemperatureSource::Sensor),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<WeatherSocTemperatureSource> {
        vec![
            WeatherSocTemperatureSource::Forecast,
            WeatherSocTemperatureSource::Sensor,
        ]
    }
}

impl std::fmt::Display for WeatherSocTemperatureSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeatherSocTemperatureSource::Forecast => write!(f, "Forecast"),
            WeatherSocTemperatureSource::Sensor => write!(f, "Sensor"),
        }
    }
}

impl std::str::FromStr for WeatherSocTemperatureSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for WeatherSocTemperatureSource {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WeatherSocTemperatureSource {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            WeatherSocTemperatureSource::Forecast => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            WeatherSocTemperatureSource::Sensor => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WeatherSocTemperatureSource {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(WeatherSocTemperatureSource::Forecast),
            1 => Ok(WeatherSocTemperatureSource::Sensor),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}