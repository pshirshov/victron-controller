

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum ChargeBatteryExtendedMode {
    Auto,
    Forced,
    Disabled,
}

impl ChargeBatteryExtendedMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Auto" => Ok(ChargeBatteryExtendedMode::Auto),
            "Forced" => Ok(ChargeBatteryExtendedMode::Forced),
            "Disabled" => Ok(ChargeBatteryExtendedMode::Disabled),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<ChargeBatteryExtendedMode> {
        vec![
            ChargeBatteryExtendedMode::Auto,
            ChargeBatteryExtendedMode::Forced,
            ChargeBatteryExtendedMode::Disabled,
        ]
    }
}

impl std::fmt::Display for ChargeBatteryExtendedMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChargeBatteryExtendedMode::Auto => write!(f, "Auto"),
            ChargeBatteryExtendedMode::Forced => write!(f, "Forced"),
            ChargeBatteryExtendedMode::Disabled => write!(f, "Disabled"),
        }
    }
}

impl std::str::FromStr for ChargeBatteryExtendedMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ChargeBatteryExtendedMode {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ChargeBatteryExtendedMode {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            ChargeBatteryExtendedMode::Auto => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            ChargeBatteryExtendedMode::Forced => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            ChargeBatteryExtendedMode::Disabled => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ChargeBatteryExtendedMode {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(ChargeBatteryExtendedMode::Auto),
            1 => Ok(ChargeBatteryExtendedMode::Forced),
            2 => Ok(ChargeBatteryExtendedMode::Disabled),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}