use crate::victron_controller::dashboard::v0_2_0::command_ack::CommandAck;
use crate::victron_controller::dashboard::v0_2_0::world_snapshot::WorldSnapshot;
use crate::victron_controller::dashboard::v0_2_0::ws_log_line::WsLogLine;
use crate::victron_controller::dashboard::v0_2_0::ws_pong::WsPong;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Hello {
    pub server_version: String,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub server_ts_ms: i64,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Pong {
    pub body: WsPong,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub body: WorldSnapshot,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Log {
    pub body: WsLogLine,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Ack {
    pub body: CommandAck,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for Hello {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Hello {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.server_version.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.server_ts_ms.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.server_version.encode_ueba(ctx, writer)?;
            value.server_ts_ms.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Hello {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let server_version = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let server_ts_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        Ok(Hello {
            server_version,
            server_ts_ms,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Pong {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Pong {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.body.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.body.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Pong {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let body = WsPong::decode_ueba(ctx, reader)?;
        Ok(Pong {
            body,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Snapshot {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Snapshot {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.body.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.body.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Snapshot {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let body = WorldSnapshot::decode_ueba(ctx, reader)?;
        Ok(Snapshot {
            body,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Log {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Log {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.body.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.body.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Log {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let body = WsLogLine::decode_ueba(ctx, reader)?;
        Ok(Log {
            body,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Ack {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Ack {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.body.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.body.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Ack {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let body = CommandAck::decode_ueba(ctx, reader)?;
        Ok(Ack {
            body,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WsServerMessage {
    Hello(Hello),
    Pong(Pong),
    Snapshot(Snapshot),
    Log(Log),
    Ack(Ack),
}

impl serde::Serialize for WsServerMessage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            WsServerMessage::Hello(v) => {
                map.serialize_entry("Hello", v)?;
            }
            WsServerMessage::Pong(v) => {
                map.serialize_entry("Pong", v)?;
            }
            WsServerMessage::Snapshot(v) => {
                map.serialize_entry("Snapshot", v)?;
            }
            WsServerMessage::Log(v) => {
                map.serialize_entry("Log", v)?;
            }
            WsServerMessage::Ack(v) => {
                map.serialize_entry("Ack", v)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for WsServerMessage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AdtVisitor;
        impl<'de> serde::de::Visitor<'de> for AdtVisitor {
            type Value = WsServerMessage;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a single-key map representing WsServerMessage")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let key: String = map.next_key()?
                    .ok_or_else(|| serde::de::Error::custom("expected single-key map for ADT"))?;
                match key.as_str() {
                    "Hello" => Ok(WsServerMessage::Hello(map.next_value()?)),
                    "Pong" => Ok(WsServerMessage::Pong(map.next_value()?)),
                    "Snapshot" => Ok(WsServerMessage::Snapshot(map.next_value()?)),
                    "Log" => Ok(WsServerMessage::Log(map.next_value()?)),
                    "Ack" => Ok(WsServerMessage::Ack(map.next_value()?)),
                    _ => Err(serde::de::Error::unknown_variant(&key, &["Hello", "Pong", "Snapshot", "Log", "Ack"])),
                }
            }
        }
        deserializer.deserialize_map(AdtVisitor)
    }
}

impl std::fmt::Display for WsServerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsServerMessage::Hello(v) => write!(f, "WsServerMessage::Hello({:?})", v),
            WsServerMessage::Pong(v) => write!(f, "WsServerMessage::Pong({:?})", v),
            WsServerMessage::Snapshot(v) => write!(f, "WsServerMessage::Snapshot({:?})", v),
            WsServerMessage::Log(v) => write!(f, "WsServerMessage::Log({:?})", v),
            WsServerMessage::Ack(v) => write!(f, "WsServerMessage::Ack({:?})", v),
        }
    }
}

impl std::error::Error for WsServerMessage {}

impl crate::baboon_runtime::BaboonBinCodecIndexed for WsServerMessage {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WsServerMessage {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            WsServerMessage::Hello(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 0)?;
                v.encode_ueba(ctx, writer)?;
            }
            WsServerMessage::Pong(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                v.encode_ueba(ctx, writer)?;
            }
            WsServerMessage::Snapshot(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 2)?;
                v.encode_ueba(ctx, writer)?;
            }
            WsServerMessage::Log(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 3)?;
                v.encode_ueba(ctx, writer)?;
            }
            WsServerMessage::Ack(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 4)?;
                v.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WsServerMessage {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => {
                let v = Hello::decode_ueba(ctx, reader)?;
                Ok(WsServerMessage::Hello(v))
            }
            1 => {
                let v = Pong::decode_ueba(ctx, reader)?;
                Ok(WsServerMessage::Pong(v))
            }
            2 => {
                let v = Snapshot::decode_ueba(ctx, reader)?;
                Ok(WsServerMessage::Snapshot(v))
            }
            3 => {
                let v = Log::decode_ueba(ctx, reader)?;
                Ok(WsServerMessage::Log(v))
            }
            4 => {
                let v = Ack::decode_ueba(ctx, reader)?;
                Ok(WsServerMessage::Ack(v))
            }
            _ => Err(format!("Unknown ADT branch tag: {}", tag).into()),
        }
    }
}