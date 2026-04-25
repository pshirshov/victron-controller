

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum TargetPhase {
    Unset,
    Pending,
    Commanded,
    Confirmed,
}

impl TargetPhase {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Unset" => Ok(TargetPhase::Unset),
            "Pending" => Ok(TargetPhase::Pending),
            "Commanded" => Ok(TargetPhase::Commanded),
            "Confirmed" => Ok(TargetPhase::Confirmed),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<TargetPhase> {
        vec![
            TargetPhase::Unset,
            TargetPhase::Pending,
            TargetPhase::Commanded,
            TargetPhase::Confirmed,
        ]
    }
}

impl std::fmt::Display for TargetPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetPhase::Unset => write!(f, "Unset"),
            TargetPhase::Pending => write!(f, "Pending"),
            TargetPhase::Commanded => write!(f, "Commanded"),
            TargetPhase::Confirmed => write!(f, "Confirmed"),
        }
    }
}

impl std::str::FromStr for TargetPhase {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for TargetPhase {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for TargetPhase {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            TargetPhase::Unset => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            TargetPhase::Pending => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            TargetPhase::Commanded => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            TargetPhase::Confirmed => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for TargetPhase {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(TargetPhase::Unset),
            1 => Ok(TargetPhase::Pending),
            2 => Ok(TargetPhase::Commanded),
            3 => Ok(TargetPhase::Confirmed),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}