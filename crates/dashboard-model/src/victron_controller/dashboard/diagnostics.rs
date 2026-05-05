

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub process_uptime_s: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub process_rss_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub process_vm_hwm_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub process_vm_size_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub jemalloc_allocated_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub jemalloc_resident_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub host_mem_total_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub host_mem_available_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub host_swap_used_bytes: i64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub sampled_at_epoch_ms: i64,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for Diagnostics {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for Diagnostics {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.process_uptime_s.encode_ueba(ctx, &mut buffer)?;
            value.process_rss_bytes.encode_ueba(ctx, &mut buffer)?;
            value.process_vm_hwm_bytes.encode_ueba(ctx, &mut buffer)?;
            value.process_vm_size_bytes.encode_ueba(ctx, &mut buffer)?;
            value.jemalloc_allocated_bytes.encode_ueba(ctx, &mut buffer)?;
            value.jemalloc_resident_bytes.encode_ueba(ctx, &mut buffer)?;
            value.host_mem_total_bytes.encode_ueba(ctx, &mut buffer)?;
            value.host_mem_available_bytes.encode_ueba(ctx, &mut buffer)?;
            value.host_swap_used_bytes.encode_ueba(ctx, &mut buffer)?;
            value.sampled_at_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.process_uptime_s.encode_ueba(ctx, writer)?;
            value.process_rss_bytes.encode_ueba(ctx, writer)?;
            value.process_vm_hwm_bytes.encode_ueba(ctx, writer)?;
            value.process_vm_size_bytes.encode_ueba(ctx, writer)?;
            value.jemalloc_allocated_bytes.encode_ueba(ctx, writer)?;
            value.jemalloc_resident_bytes.encode_ueba(ctx, writer)?;
            value.host_mem_total_bytes.encode_ueba(ctx, writer)?;
            value.host_mem_available_bytes.encode_ueba(ctx, writer)?;
            value.host_swap_used_bytes.encode_ueba(ctx, writer)?;
            value.sampled_at_epoch_ms.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for Diagnostics {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let process_uptime_s = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let process_rss_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let process_vm_hwm_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let process_vm_size_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let jemalloc_allocated_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let jemalloc_resident_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let host_mem_total_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let host_mem_available_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let host_swap_used_bytes = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let sampled_at_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        Ok(Diagnostics {
            process_uptime_s,
            process_rss_bytes,
            process_vm_hwm_bytes,
            process_vm_size_bytes,
            jemalloc_allocated_bytes,
            jemalloc_resident_bytes,
            host_mem_total_bytes,
            host_mem_available_bytes,
            host_swap_used_bytes,
            sampled_at_epoch_ms,
        })
    }
}