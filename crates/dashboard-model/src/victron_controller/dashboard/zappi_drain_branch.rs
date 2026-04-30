

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum ZappiDrainBranch {
    Tighten,
    Relax,
    Bypass,
    Disabled,
}

impl ZappiDrainBranch {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Tighten" => Ok(ZappiDrainBranch::Tighten),
            "Relax" => Ok(ZappiDrainBranch::Relax),
            "Bypass" => Ok(ZappiDrainBranch::Bypass),
            "Disabled" => Ok(ZappiDrainBranch::Disabled),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<ZappiDrainBranch> {
        vec![
            ZappiDrainBranch::Tighten,
            ZappiDrainBranch::Relax,
            ZappiDrainBranch::Bypass,
            ZappiDrainBranch::Disabled,
        ]
    }
}

impl std::fmt::Display for ZappiDrainBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZappiDrainBranch::Tighten => write!(f, "Tighten"),
            ZappiDrainBranch::Relax => write!(f, "Relax"),
            ZappiDrainBranch::Bypass => write!(f, "Bypass"),
            ZappiDrainBranch::Disabled => write!(f, "Disabled"),
        }
    }
}

impl std::str::FromStr for ZappiDrainBranch {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ZappiDrainBranch {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ZappiDrainBranch {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            ZappiDrainBranch::Tighten => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            ZappiDrainBranch::Relax => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            ZappiDrainBranch::Bypass => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            ZappiDrainBranch::Disabled => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ZappiDrainBranch {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(ZappiDrainBranch::Tighten),
            1 => Ok(ZappiDrainBranch::Relax),
            2 => Ok(ZappiDrainBranch::Bypass),
            3 => Ok(ZappiDrainBranch::Disabled),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}