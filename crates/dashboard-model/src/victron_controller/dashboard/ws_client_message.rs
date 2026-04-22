use crate::victron_controller::dashboard::command::Command;
use crate::victron_controller::dashboard::ws_ping::WsPing;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Ping {
    pub body: WsPing,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SendCommand {
    pub body: Command,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for Ping {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Ping {
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

impl crate::baboon_runtime::BaboonBinDecode for Ping {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let body = WsPing::decode_ueba(ctx, reader)?;
        Ok(Ping {
            body,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SendCommand {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SendCommand {
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

impl crate::baboon_runtime::BaboonBinDecode for SendCommand {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let body = Command::decode_ueba(ctx, reader)?;
        Ok(SendCommand {
            body,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WsClientMessage {
    Ping(Ping),
    SendCommand(SendCommand),
}

impl serde::Serialize for WsClientMessage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            WsClientMessage::Ping(v) => {
                map.serialize_entry("Ping", v)?;
            }
            WsClientMessage::SendCommand(v) => {
                map.serialize_entry("SendCommand", v)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for WsClientMessage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AdtVisitor;
        impl<'de> serde::de::Visitor<'de> for AdtVisitor {
            type Value = WsClientMessage;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a single-key map representing WsClientMessage")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let key: String = map.next_key()?
                    .ok_or_else(|| serde::de::Error::custom("expected single-key map for ADT"))?;
                match key.as_str() {
                    "Ping" => Ok(WsClientMessage::Ping(map.next_value()?)),
                    "SendCommand" => Ok(WsClientMessage::SendCommand(map.next_value()?)),
                    _ => Err(serde::de::Error::unknown_variant(&key, &["Ping", "SendCommand"])),
                }
            }
        }
        deserializer.deserialize_map(AdtVisitor)
    }
}

impl std::fmt::Display for WsClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsClientMessage::Ping(v) => write!(f, "WsClientMessage::Ping({:?})", v),
            WsClientMessage::SendCommand(v) => write!(f, "WsClientMessage::SendCommand({:?})", v),
        }
    }
}

impl std::error::Error for WsClientMessage {}

impl crate::baboon_runtime::BaboonBinCodecIndexed for WsClientMessage {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for WsClientMessage {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            WsClientMessage::Ping(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 0)?;
                v.encode_ueba(ctx, writer)?;
            }
            WsClientMessage::SendCommand(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                v.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for WsClientMessage {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => {
                let v = Ping::decode_ueba(ctx, reader)?;
                Ok(WsClientMessage::Ping(v))
            }
            1 => {
                let v = SendCommand::decode_ueba(ctx, reader)?;
                Ok(WsClientMessage::SendCommand(v))
            }
            _ => Err(format!("Unknown ADT branch tag: {}", tag).into()),
        }
    }
}