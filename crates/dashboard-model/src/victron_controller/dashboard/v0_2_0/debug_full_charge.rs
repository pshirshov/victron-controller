

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum DebugFullCharge {
    Forbid,
    Force,
    Auto,
}

impl DebugFullCharge {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Forbid" => Ok(DebugFullCharge::Forbid),
            "Force" => Ok(DebugFullCharge::Force),
            "Auto" => Ok(DebugFullCharge::Auto),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<DebugFullCharge> {
        vec![
            DebugFullCharge::Forbid,
            DebugFullCharge::Force,
            DebugFullCharge::Auto,
        ]
    }
}

impl std::fmt::Display for DebugFullCharge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DebugFullCharge::Forbid => write!(f, "Forbid"),
            DebugFullCharge::Force => write!(f, "Force"),
            DebugFullCharge::Auto => write!(f, "Auto"),
        }
    }
}

impl std::str::FromStr for DebugFullCharge {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for DebugFullCharge {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for DebugFullCharge {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            DebugFullCharge::Forbid => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            DebugFullCharge::Force => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            DebugFullCharge::Auto => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for DebugFullCharge {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(DebugFullCharge::Forbid),
            1 => Ok(DebugFullCharge::Force),
            2 => Ok(DebugFullCharge::Auto),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}