

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Weather,
    Forced,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Weather" => Ok(Mode::Weather),
            "Forced" => Ok(Mode::Forced),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<Mode> {
        vec![
            Mode::Weather,
            Mode::Forced,
        ]
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Weather => write!(f, "Weather"),
            Mode::Forced => write!(f, "Forced"),
        }
    }
}

impl std::str::FromStr for Mode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Mode {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Mode {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            Mode::Weather => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            Mode::Forced => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Mode {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(Mode::Weather),
            1 => Ok(Mode::Forced),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}