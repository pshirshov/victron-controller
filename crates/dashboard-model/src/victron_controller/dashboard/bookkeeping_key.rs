

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum BookkeepingKey {
    NextFullCharge,
    AboveSocDate,
    PrevEssState,
    ChargeToFullRequired,
}

impl BookkeepingKey {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "NextFullCharge" => Ok(BookkeepingKey::NextFullCharge),
            "AboveSocDate" => Ok(BookkeepingKey::AboveSocDate),
            "PrevEssState" => Ok(BookkeepingKey::PrevEssState),
            "ChargeToFullRequired" => Ok(BookkeepingKey::ChargeToFullRequired),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<BookkeepingKey> {
        vec![
            BookkeepingKey::NextFullCharge,
            BookkeepingKey::AboveSocDate,
            BookkeepingKey::PrevEssState,
            BookkeepingKey::ChargeToFullRequired,
        ]
    }
}

impl std::fmt::Display for BookkeepingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BookkeepingKey::NextFullCharge => write!(f, "NextFullCharge"),
            BookkeepingKey::AboveSocDate => write!(f, "AboveSocDate"),
            BookkeepingKey::PrevEssState => write!(f, "PrevEssState"),
            BookkeepingKey::ChargeToFullRequired => write!(f, "ChargeToFullRequired"),
        }
    }
}

impl std::str::FromStr for BookkeepingKey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for BookkeepingKey {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for BookkeepingKey {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            BookkeepingKey::NextFullCharge => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            BookkeepingKey::AboveSocDate => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            BookkeepingKey::PrevEssState => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            BookkeepingKey::ChargeToFullRequired => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for BookkeepingKey {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(BookkeepingKey::NextFullCharge),
            1 => Ok(BookkeepingKey::AboveSocDate),
            2 => Ok(BookkeepingKey::PrevEssState),
            3 => Ok(BookkeepingKey::ChargeToFullRequired),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}