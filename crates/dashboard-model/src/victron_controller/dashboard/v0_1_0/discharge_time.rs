

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum DischargeTime {
    At0200,
    At2300,
}

impl DischargeTime {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "At0200" => Ok(DischargeTime::At0200),
            "At2300" => Ok(DischargeTime::At2300),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<DischargeTime> {
        vec![
            DischargeTime::At0200,
            DischargeTime::At2300,
        ]
    }
}

impl std::fmt::Display for DischargeTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DischargeTime::At0200 => write!(f, "At0200"),
            DischargeTime::At2300 => write!(f, "At2300"),
        }
    }
}

impl std::str::FromStr for DischargeTime {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for DischargeTime {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for DischargeTime {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            DischargeTime::At0200 => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            DischargeTime::At2300 => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for DischargeTime {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(DischargeTime::At0200),
            1 => Ok(DischargeTime::At2300),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}