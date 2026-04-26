

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum ExtendedChargeMode {
    Auto,
    Forced,
    Disabled,
}

impl ExtendedChargeMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Auto" => Ok(ExtendedChargeMode::Auto),
            "Forced" => Ok(ExtendedChargeMode::Forced),
            "Disabled" => Ok(ExtendedChargeMode::Disabled),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<ExtendedChargeMode> {
        vec![
            ExtendedChargeMode::Auto,
            ExtendedChargeMode::Forced,
            ExtendedChargeMode::Disabled,
        ]
    }
}

impl std::fmt::Display for ExtendedChargeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtendedChargeMode::Auto => write!(f, "Auto"),
            ExtendedChargeMode::Forced => write!(f, "Forced"),
            ExtendedChargeMode::Disabled => write!(f, "Disabled"),
        }
    }
}

impl std::str::FromStr for ExtendedChargeMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ExtendedChargeMode {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ExtendedChargeMode {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            ExtendedChargeMode::Auto => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            ExtendedChargeMode::Forced => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            ExtendedChargeMode::Disabled => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ExtendedChargeMode {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(ExtendedChargeMode::Auto),
            1 => Ok(ExtendedChargeMode::Forced),
            2 => Ok(ExtendedChargeMode::Disabled),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}