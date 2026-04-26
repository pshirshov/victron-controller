

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum SocProjectionKind {
    Natural,
    Idle,
    ScheduledCharge,
    FullChargePush,
    Clamped,
    SolarCharge,
    Drain,
}

impl SocProjectionKind {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Natural" => Ok(SocProjectionKind::Natural),
            "Idle" => Ok(SocProjectionKind::Idle),
            "ScheduledCharge" => Ok(SocProjectionKind::ScheduledCharge),
            "FullChargePush" => Ok(SocProjectionKind::FullChargePush),
            "Clamped" => Ok(SocProjectionKind::Clamped),
            "SolarCharge" => Ok(SocProjectionKind::SolarCharge),
            "Drain" => Ok(SocProjectionKind::Drain),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<SocProjectionKind> {
        vec![
            SocProjectionKind::Natural,
            SocProjectionKind::Idle,
            SocProjectionKind::ScheduledCharge,
            SocProjectionKind::FullChargePush,
            SocProjectionKind::Clamped,
            SocProjectionKind::SolarCharge,
            SocProjectionKind::Drain,
        ]
    }
}

impl std::fmt::Display for SocProjectionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocProjectionKind::Natural => write!(f, "Natural"),
            SocProjectionKind::Idle => write!(f, "Idle"),
            SocProjectionKind::ScheduledCharge => write!(f, "ScheduledCharge"),
            SocProjectionKind::FullChargePush => write!(f, "FullChargePush"),
            SocProjectionKind::Clamped => write!(f, "Clamped"),
            SocProjectionKind::SolarCharge => write!(f, "SolarCharge"),
            SocProjectionKind::Drain => write!(f, "Drain"),
        }
    }
}

impl std::str::FromStr for SocProjectionKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SocProjectionKind {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SocProjectionKind {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            SocProjectionKind::Natural => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            SocProjectionKind::Idle => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            SocProjectionKind::ScheduledCharge => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            SocProjectionKind::FullChargePush => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
            SocProjectionKind::Clamped => crate::baboon_runtime::bin_tools::write_byte(writer, 4)?,
            SocProjectionKind::SolarCharge => crate::baboon_runtime::bin_tools::write_byte(writer, 5)?,
            SocProjectionKind::Drain => crate::baboon_runtime::bin_tools::write_byte(writer, 6)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SocProjectionKind {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(SocProjectionKind::Natural),
            1 => Ok(SocProjectionKind::Idle),
            2 => Ok(SocProjectionKind::ScheduledCharge),
            3 => Ok(SocProjectionKind::FullChargePush),
            4 => Ok(SocProjectionKind::Clamped),
            5 => Ok(SocProjectionKind::SolarCharge),
            6 => Ok(SocProjectionKind::Drain),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}