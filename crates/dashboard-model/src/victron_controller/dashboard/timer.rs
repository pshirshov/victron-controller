

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Timer {
    pub id: String,
    pub description: String,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub period_ms: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub last_fire_epoch_ms: Option<i64>,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub next_fire_epoch_ms: Option<i64>,
    pub status: String,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for Timer {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        5
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Timer {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.id.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.description.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.period_ms.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.last_fire_epoch_ms {
                None => crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 1)?;
                    v.encode_ueba(ctx, &mut buffer)?;
                }
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.next_fire_epoch_ms {
                None => crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(&mut buffer, 1)?;
                    v.encode_ueba(ctx, &mut buffer)?;
                }
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.status.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.id.encode_ueba(ctx, writer)?;
            value.description.encode_ueba(ctx, writer)?;
            value.period_ms.encode_ueba(ctx, writer)?;
            match &value.last_fire_epoch_ms {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            match &value.next_fire_epoch_ms {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            value.status.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Timer {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let id = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let description = crate::baboon_runtime::bin_tools::read_string(reader)?;
        let period_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let last_fire_epoch_ms = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_i64(reader)?) }
        };
        let next_fire_epoch_ms = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_i64(reader)?) }
        };
        let status = crate::baboon_runtime::bin_tools::read_string(reader)?;
        Ok(Timer {
            id,
            description,
            period_ms,
            last_fire_epoch_ms,
            next_fire_epoch_ms,
            status,
        })
    }
}