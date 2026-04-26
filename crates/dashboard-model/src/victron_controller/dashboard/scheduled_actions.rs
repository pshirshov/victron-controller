use crate::victron_controller::dashboard::scheduled_action::ScheduledAction;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct ScheduledActions {
    pub entries: Vec<ScheduledAction>,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for ScheduledActions {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        1
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ScheduledActions {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.entries.len() as i32)?;
            for item in (value.entries).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            crate::baboon_runtime::bin_tools::write_i32(writer, value.entries.len() as i32)?;
            for item in (value.entries).iter() {
                item.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ScheduledActions {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let entries = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(ScheduledAction::decode_ueba(ctx, reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        Ok(ScheduledActions {
            entries,
        })
    }
}