

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Owner {
    Unset,
    System,
    Dashboard,
    HaMqtt,
    WeatherSocPlanner,
    SetpointController,
    CurrentLimitController,
    ScheduleController,
    ZappiController,
    EddiController,
    FullChargeScheduler,
}

impl Owner {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "Unset" => Ok(Owner::Unset),
            "System" => Ok(Owner::System),
            "Dashboard" => Ok(Owner::Dashboard),
            "HaMqtt" => Ok(Owner::HaMqtt),
            "WeatherSocPlanner" => Ok(Owner::WeatherSocPlanner),
            "SetpointController" => Ok(Owner::SetpointController),
            "CurrentLimitController" => Ok(Owner::CurrentLimitController),
            "ScheduleController" => Ok(Owner::ScheduleController),
            "ZappiController" => Ok(Owner::ZappiController),
            "EddiController" => Ok(Owner::EddiController),
            "FullChargeScheduler" => Ok(Owner::FullChargeScheduler),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }

    pub fn all() -> Vec<Owner> {
        vec![
            Owner::Unset,
            Owner::System,
            Owner::Dashboard,
            Owner::HaMqtt,
            Owner::WeatherSocPlanner,
            Owner::SetpointController,
            Owner::CurrentLimitController,
            Owner::ScheduleController,
            Owner::ZappiController,
            Owner::EddiController,
            Owner::FullChargeScheduler,
        ]
    }
}

impl std::fmt::Display for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Owner::Unset => write!(f, "Unset"),
            Owner::System => write!(f, "System"),
            Owner::Dashboard => write!(f, "Dashboard"),
            Owner::HaMqtt => write!(f, "HaMqtt"),
            Owner::WeatherSocPlanner => write!(f, "WeatherSocPlanner"),
            Owner::SetpointController => write!(f, "SetpointController"),
            Owner::CurrentLimitController => write!(f, "CurrentLimitController"),
            Owner::ScheduleController => write!(f, "ScheduleController"),
            Owner::ZappiController => write!(f, "ZappiController"),
            Owner::EddiController => write!(f, "EddiController"),
            Owner::FullChargeScheduler => write!(f, "FullChargeScheduler"),
        }
    }
}

impl std::str::FromStr for Owner {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Owner {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Owner {
    fn encode_ueba(&self, _ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            Owner::Unset => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
            Owner::System => crate::baboon_runtime::bin_tools::write_byte(writer, 1)?,
            Owner::Dashboard => crate::baboon_runtime::bin_tools::write_byte(writer, 2)?,
            Owner::HaMqtt => crate::baboon_runtime::bin_tools::write_byte(writer, 3)?,
            Owner::WeatherSocPlanner => crate::baboon_runtime::bin_tools::write_byte(writer, 4)?,
            Owner::SetpointController => crate::baboon_runtime::bin_tools::write_byte(writer, 5)?,
            Owner::CurrentLimitController => crate::baboon_runtime::bin_tools::write_byte(writer, 6)?,
            Owner::ScheduleController => crate::baboon_runtime::bin_tools::write_byte(writer, 7)?,
            Owner::ZappiController => crate::baboon_runtime::bin_tools::write_byte(writer, 8)?,
            Owner::EddiController => crate::baboon_runtime::bin_tools::write_byte(writer, 9)?,
            Owner::FullChargeScheduler => crate::baboon_runtime::bin_tools::write_byte(writer, 10)?,
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Owner {
    fn decode_ueba(_ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => Ok(Owner::Unset),
            1 => Ok(Owner::System),
            2 => Ok(Owner::Dashboard),
            3 => Ok(Owner::HaMqtt),
            4 => Ok(Owner::WeatherSocPlanner),
            5 => Ok(Owner::SetpointController),
            6 => Ok(Owner::CurrentLimitController),
            7 => Ok(Owner::ScheduleController),
            8 => Ok(Owner::ZappiController),
            9 => Ok(Owner::EddiController),
            10 => Ok(Owner::FullChargeScheduler),
            _ => Err(format!("Unknown enum variant tag: {}", tag).into()),
        }
    }
}