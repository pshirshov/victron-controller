

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum ForecastDisagreementStrategy {
    Max,
    Min,
    Mean,
    SolcastIfAvailableElseMean,
}

impl ForecastDisagreementStrategy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Max" => Ok(ForecastDisagreementStrategy::Max),
            "Min" => Ok(ForecastDisagreementStrategy::Min),
            "Mean" => Ok(ForecastDisagreementStrategy::Mean),
            "SolcastIfAvailableElseMean" => Ok(ForecastDisagreementStrategy::SolcastIfAvailableElseMean),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<ForecastDisagreementStrategy> {
        vec![
            ForecastDisagreementStrategy::Max,
            ForecastDisagreementStrategy::Min,
            ForecastDisagreementStrategy::Mean,
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
        ]
    }
}

impl std::fmt::Display for ForecastDisagreementStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForecastDisagreementStrategy::Max => write!(f, "Max"),
            ForecastDisagreementStrategy::Min => write!(f, "Min"),
            ForecastDisagreementStrategy::Mean => write!(f, "Mean"),
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean => write!(f, "SolcastIfAvailableElseMean"),
        }
    }
}

impl std::str::FromStr for ForecastDisagreementStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ForecastDisagreementStrategy {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ForecastDisagreementStrategy {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            ForecastDisagreementStrategy::Max => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            ForecastDisagreementStrategy::Min => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            ForecastDisagreementStrategy::Mean => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            ForecastDisagreementStrategy::SolcastIfAvailableElseMean => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ForecastDisagreementStrategy {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(ForecastDisagreementStrategy::Max),
            1 => Ok(ForecastDisagreementStrategy::Min),
            2 => Ok(ForecastDisagreementStrategy::Mean),
            3 => Ok(ForecastDisagreementStrategy::SolcastIfAvailableElseMean),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}