

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum WeatherSocDay {
    Today,
    Tomorrow,
}

impl WeatherSocDay {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Today" => Ok(WeatherSocDay::Today),
            "Tomorrow" => Ok(WeatherSocDay::Tomorrow),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<WeatherSocDay> {
        vec![
            WeatherSocDay::Today,
            WeatherSocDay::Tomorrow,
        ]
    }
}

impl std::fmt::Display for WeatherSocDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeatherSocDay::Today => write!(f, "Today"),
            WeatherSocDay::Tomorrow => write!(f, "Tomorrow"),
        }
    }
}

impl std::str::FromStr for WeatherSocDay {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for WeatherSocDay {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WeatherSocDay {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            WeatherSocDay::Today => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            WeatherSocDay::Tomorrow => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WeatherSocDay {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(WeatherSocDay::Today),
            1 => Ok(WeatherSocDay::Tomorrow),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}