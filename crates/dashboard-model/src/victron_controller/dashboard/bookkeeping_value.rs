

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct NaiveDateTime {
    pub iso: String,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Bool {
    pub value: bool,
}



#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Cleared {}



impl crate::baboon_runtime::BaboonBinCodecIndexed for NaiveDateTime {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for NaiveDateTime {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.iso.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.iso.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for NaiveDateTime {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let iso = crate::baboon_runtime::bin_tools::read_string(reader)?;
        Ok(NaiveDateTime {
            iso,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Bool {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Bool {
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

impl crate::baboon_runtime::BaboonBinDecode for Bool {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let value = crate::baboon_runtime::bin_tools::read_bool(reader)?;
        Ok(Bool {
            value,
        })
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for Cleared {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Cleared {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let _value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let buffer: Vec<u8> = Vec::new();
            
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Cleared {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        
        Ok(Cleared {
            
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BookkeepingValue {
    NaiveDateTime(NaiveDateTime),
    Bool(Bool),
    Cleared(Cleared),
}

impl serde::Serialize for BookkeepingValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            BookkeepingValue::NaiveDateTime(v) => {
                map.serialize_entry("NaiveDateTime", v)?;
            }
            BookkeepingValue::Bool(v) => {
                map.serialize_entry("Bool", v)?;
            }
            BookkeepingValue::Cleared(v) => {
                map.serialize_entry("Cleared", v)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for BookkeepingValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AdtVisitor;
        impl<'de> serde::de::Visitor<'de> for AdtVisitor {
            type Value = BookkeepingValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a single-key map representing BookkeepingValue")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let key: String = map.next_key()?
                    .ok_or_else(|| serde::de::Error::custom("expected single-key map for ADT"))?;
                match key.as_str() {
                    "NaiveDateTime" => Ok(BookkeepingValue::NaiveDateTime(map.next_value()?)),
                    "Bool" => Ok(BookkeepingValue::Bool(map.next_value()?)),
                    "Cleared" => Ok(BookkeepingValue::Cleared(map.next_value()?)),
                    _ => Err(serde::de::Error::unknown_variant(&key, &["NaiveDateTime", "Bool", "Cleared"])),
                }
            }
        }
        deserializer.deserialize_map(AdtVisitor)
    }
}

impl std::fmt::Display for BookkeepingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BookkeepingValue::NaiveDateTime(v) => write!(f, "BookkeepingValue::NaiveDateTime({:?})", v),
            BookkeepingValue::Bool(v) => write!(f, "BookkeepingValue::Bool({:?})", v),
            BookkeepingValue::Cleared(v) => write!(f, "BookkeepingValue::Cleared({:?})", v),
        }
    }
}

impl std::error::Error for BookkeepingValue {}

impl crate::baboon_runtime::BaboonBinCodecIndexed for BookkeepingValue {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for BookkeepingValue {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            BookkeepingValue::NaiveDateTime(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 0)?;
                v.encode_ueba(ctx, writer)?;
            }
            BookkeepingValue::Bool(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                v.encode_ueba(ctx, writer)?;
            }
            BookkeepingValue::Cleared(v) => {
                crate::baboon_runtime::bin_tools::write_byte(writer, 2)?;
                v.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for BookkeepingValue {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
        match tag {
            0 => {
                let v = NaiveDateTime::decode_ueba(ctx, reader)?;
                Ok(BookkeepingValue::NaiveDateTime(v))
            }
            1 => {
                let v = Bool::decode_ueba(ctx, reader)?;
                Ok(BookkeepingValue::Bool(v))
            }
            2 => {
                let v = Cleared::decode_ueba(ctx, reader)?;
                Ok(BookkeepingValue::Cleared(v))
            }
            _ => Err(format!("Unknown ADT branch tag: {}", tag).into()),
        }
    }
}