use crate::victron_controller::dashboard::charge_battery_extended_mode::ChargeBatteryExtendedMode;
use crate::victron_controller::dashboard::debug_full_charge::DebugFullCharge;
use crate::victron_controller::dashboard::discharge_time::DischargeTime;
use crate::victron_controller::dashboard::forecast_disagreement_strategy::ForecastDisagreementStrategy;
use crate::victron_controller::dashboard::mode::Mode;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetBoolKnob {
    pub knob_name: String,
    pub value: bool,
}



#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SetFloatKnob {
    pub knob_name: String,
    pub value: f64,
}

impl PartialEq for SetFloatKnob {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SetFloatKnob {}

impl PartialOrd for SetFloatKnob {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SetFloatKnob {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.knob_name.cmp(&other.knob_name) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.value.total_cmp(&other.value) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetUintKnob {
    pub knob_name: String,
    pub value: i32,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetDischargeTime {
    pub value: DischargeTime,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetDebugFullCharge {
    pub value: DebugFullCharge,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetForecastDisagreementStrategy {
    pub value: ForecastDisagreementStrategy,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetChargeBatteryExtendedMode {
    pub value: ChargeBatteryExtendedMode,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetMode {
    pub knob_name: String,
    pub value: Mode,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SetKillSwitch {
    pub value: bool,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for SetBoolKnob {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetBoolKnob {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.knob_name.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.knob_name.encode_ueba(ctx, writer)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetBoolKnob {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let knob_name = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let value = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        Ok(SetBoolKnob {
            knob_name,
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetFloatKnob {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetFloatKnob {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.knob_name.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.knob_name.encode_ueba(ctx, writer)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetFloatKnob {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let knob_name = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let value = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        Ok(SetFloatKnob {
            knob_name,
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetUintKnob {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetUintKnob {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.knob_name.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.knob_name.encode_ueba(ctx, writer)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetUintKnob {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let knob_name = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let value = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        Ok(SetUintKnob {
            knob_name,
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetDischargeTime {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetDischargeTime {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetDischargeTime {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = DischargeTime::decode_ueba(ctx, reader)?;
        Ok(SetDischargeTime {
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetDebugFullCharge {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetDebugFullCharge {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetDebugFullCharge {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = DebugFullCharge::decode_ueba(ctx, reader)?;
        Ok(SetDebugFullCharge {
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetForecastDisagreementStrategy {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetForecastDisagreementStrategy {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetForecastDisagreementStrategy {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = ForecastDisagreementStrategy::decode_ueba(ctx, reader)?;
        Ok(SetForecastDisagreementStrategy {
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetChargeBatteryExtendedMode {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetChargeBatteryExtendedMode {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetChargeBatteryExtendedMode {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = ChargeBatteryExtendedMode::decode_ueba(ctx, reader)?;
        Ok(SetChargeBatteryExtendedMode {
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetMode {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetMode {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.knob_name.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.knob_name.encode_ueba(ctx, writer)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetMode {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let knob_name = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let value = Mode::decode_ueba(ctx, reader)?;
        Ok(SetMode {
            knob_name,
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SetKillSwitch {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SetKillSwitch {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.value.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.value.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SetKillSwitch {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        Ok(SetKillSwitch {
            value,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Command {
    SetBoolKnob(SetBoolKnob),
    SetFloatKnob(SetFloatKnob),
    SetUintKnob(SetUintKnob),
    SetDischargeTime(SetDischargeTime),
    SetDebugFullCharge(SetDebugFullCharge),
    SetForecastDisagreementStrategy(SetForecastDisagreementStrategy),
    SetChargeBatteryExtendedMode(SetChargeBatteryExtendedMode),
    SetMode(SetMode),
    SetKillSwitch(SetKillSwitch),
}

impl serde::Serialize for Command {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Command::SetBoolKnob(v) => {
                map.serialize_entry("SetBoolKnob", v)?;
            }
            Command::SetFloatKnob(v) => {
                map.serialize_entry("SetFloatKnob", v)?;
            }
            Command::SetUintKnob(v) => {
                map.serialize_entry("SetUintKnob", v)?;
            }
            Command::SetDischargeTime(v) => {
                map.serialize_entry("SetDischargeTime", v)?;
            }
            Command::SetDebugFullCharge(v) => {
                map.serialize_entry("SetDebugFullCharge", v)?;
            }
            Command::SetForecastDisagreementStrategy(v) => {
                map.serialize_entry("SetForecastDisagreementStrategy", v)?;
            }
            Command::SetChargeBatteryExtendedMode(v) => {
                map.serialize_entry("SetChargeBatteryExtendedMode", v)?;
            }
            Command::SetMode(v) => {
                map.serialize_entry("SetMode", v)?;
            }
            Command::SetKillSwitch(v) => {
                map.serialize_entry("SetKillSwitch", v)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for Command {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AdtVisitor;
        impl<'de> serde::de::Visitor<'de> for AdtVisitor {
            type Value = Command;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a single-key map representing Command")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let key: String = map.next_key()?
                    .ok_or_else(|| serde::de::Error::custom("expected single-key map for ADT"))?;
                match key.as_str() {
                    "SetBoolKnob" => Ok(Command::SetBoolKnob(map.next_value()?)),
                    "SetFloatKnob" => Ok(Command::SetFloatKnob(map.next_value()?)),
                    "SetUintKnob" => Ok(Command::SetUintKnob(map.next_value()?)),
                    "SetDischargeTime" => Ok(Command::SetDischargeTime(map.next_value()?)),
                    "SetDebugFullCharge" => Ok(Command::SetDebugFullCharge(map.next_value()?)),
                    "SetForecastDisagreementStrategy" => Ok(Command::SetForecastDisagreementStrategy(map.next_value()?)),
                    "SetChargeBatteryExtendedMode" => Ok(Command::SetChargeBatteryExtendedMode(map.next_value()?)),
                    "SetMode" => Ok(Command::SetMode(map.next_value()?)),
                    "SetKillSwitch" => Ok(Command::SetKillSwitch(map.next_value()?)),
                    _ => Err(serde::de::Error::unknown_variant(&key, &["SetBoolKnob", "SetFloatKnob", "SetUintKnob", "SetDischargeTime", "SetDebugFullCharge", "SetForecastDisagreementStrategy", "SetChargeBatteryExtendedMode", "SetMode", "SetKillSwitch"])),
                }
            }
        }
        deserializer.deserialize_map(AdtVisitor)
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::SetBoolKnob(v) => write!(f, "Command::SetBoolKnob({:?})", v),
            Command::SetFloatKnob(v) => write!(f, "Command::SetFloatKnob({:?})", v),
            Command::SetUintKnob(v) => write!(f, "Command::SetUintKnob({:?})", v),
            Command::SetDischargeTime(v) => write!(f, "Command::SetDischargeTime({:?})", v),
            Command::SetDebugFullCharge(v) => write!(f, "Command::SetDebugFullCharge({:?})", v),
            Command::SetForecastDisagreementStrategy(v) => write!(f, "Command::SetForecastDisagreementStrategy({:?})", v),
            Command::SetChargeBatteryExtendedMode(v) => write!(f, "Command::SetChargeBatteryExtendedMode({:?})", v),
            Command::SetMode(v) => write!(f, "Command::SetMode({:?})", v),
            Command::SetKillSwitch(v) => write!(f, "Command::SetKillSwitch({:?})", v),
        }
    }
}

impl std::error::Error for Command {}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Command {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Command {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            Command::SetBoolKnob(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 0)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetFloatKnob(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetUintKnob(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 2)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetDischargeTime(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 3)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetDebugFullCharge(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 4)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetForecastDisagreementStrategy(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 5)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetChargeBatteryExtendedMode(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 6)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetMode(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 7)?;
                v.encode_ueba(ctx, writer)?;
            }
            Command::SetKillSwitch(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 8)?;
                v.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Command {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => {
                let v = SetBoolKnob::decode_ueba(ctx, reader)?;
                Ok(Command::SetBoolKnob(v))
            }
            1 => {
                let v = SetFloatKnob::decode_ueba(ctx, reader)?;
                Ok(Command::SetFloatKnob(v))
            }
            2 => {
                let v = SetUintKnob::decode_ueba(ctx, reader)?;
                Ok(Command::SetUintKnob(v))
            }
            3 => {
                let v = SetDischargeTime::decode_ueba(ctx, reader)?;
                Ok(Command::SetDischargeTime(v))
            }
            4 => {
                let v = SetDebugFullCharge::decode_ueba(ctx, reader)?;
                Ok(Command::SetDebugFullCharge(v))
            }
            5 => {
                let v = SetForecastDisagreementStrategy::decode_ueba(ctx, reader)?;
                Ok(Command::SetForecastDisagreementStrategy(v))
            }
            6 => {
                let v = SetChargeBatteryExtendedMode::decode_ueba(ctx, reader)?;
                Ok(Command::SetChargeBatteryExtendedMode(v))
            }
            7 => {
                let v = SetMode::decode_ueba(ctx, reader)?;
                Ok(Command::SetMode(v))
            }
            8 => {
                let v = SetKillSwitch::decode_ueba(ctx, reader)?;
                Ok(Command::SetKillSwitch(v))
            }
            _ => Err(format!("Unknown ADT branch tag: {}", tag).into()),
        }
    }
}