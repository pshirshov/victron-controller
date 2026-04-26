use crate::victron_controller::dashboard::v0_2_0::core_state::CoreState;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct CoresState {
    pub cores: Vec<CoreState>,
    pub topo_order: Vec<String>,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for CoresState {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        2
    }
}

impl crate::baboon_runtime::BaboonBinEncode for CoresState {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.cores.len() as i32)?;
            for item in (value.cores).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.topo_order.len() as i32)?;
            for item in (value.topo_order).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            crate::baboon_runtime::bin_tools::write_i32(writer, value.cores.len() as i32)?;
            for item in (value.cores).iter() {
                item.encode_ueba(ctx, writer)?;
            }
            crate::baboon_runtime::bin_tools::write_i32(writer, value.topo_order.len() as i32)?;
            for item in (value.topo_order).iter() {
                item.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for CoresState {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let cores = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(CoreState::decode_ueba(ctx, reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        let topo_order = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(crate::baboon_runtime::bin_tools::read_string(reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        Ok(CoresState {
            cores,
            topo_order,
        })
    }
}