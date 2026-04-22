

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Freshness {
    Unknown,
    Fresh,
    Stale,
    Deprecated,
}

impl Freshness {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Unknown" => Ok(Freshness::Unknown),
            "Fresh" => Ok(Freshness::Fresh),
            "Stale" => Ok(Freshness::Stale),
            "Deprecated" => Ok(Freshness::Deprecated),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<Freshness> {
        vec![
            Freshness::Unknown,
            Freshness::Fresh,
            Freshness::Stale,
            Freshness::Deprecated,
        ]
    }
}

impl std::fmt::Display for Freshness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Freshness::Unknown => write!(f, "Unknown"),
            Freshness::Fresh => write!(f, "Fresh"),
            Freshness::Stale => write!(f, "Stale"),
            Freshness::Deprecated => write!(f, "Deprecated"),
        }
    }
}

impl std::str::FromStr for Freshness {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Freshness {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Freshness {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            Freshness::Unknown => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            Freshness::Fresh => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            Freshness::Stale => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            Freshness::Deprecated => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Freshness {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(Freshness::Unknown),
            1 => Ok(Freshness::Fresh),
            2 => Ok(Freshness::Stale),
            3 => Ok(Freshness::Deprecated),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}