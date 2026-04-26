

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScheduleSpec {
    pub start_s: i32,
    pub duration_s: i32,
    pub discharge: i32,
    pub soc: f64,
    pub days: i32,
}

impl PartialEq for ScheduleSpec {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ScheduleSpec {}

impl PartialOrd for ScheduleSpec {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduleSpec {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.start_s.cmp(&other.start_s) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.duration_s.cmp(&other.duration_s) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.discharge.cmp(&other.discharge) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.soc.total_cmp(&other.soc) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.days.cmp(&other.days) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ScheduleSpec {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ScheduleSpec {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.start_s.encode_ueba(ctx, &mut buffer)?;
            value.duration_s.encode_ueba(ctx, &mut buffer)?;
            value.discharge.encode_ueba(ctx, &mut buffer)?;
            value.soc.encode_ueba(ctx, &mut buffer)?;
            value.days.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.start_s.encode_ueba(ctx, writer)?;
            value.duration_s.encode_ueba(ctx, writer)?;
            value.discharge.encode_ueba(ctx, writer)?;
            value.soc.encode_ueba(ctx, writer)?;
            value.days.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ScheduleSpec {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let start_s = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let duration_s = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let discharge = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        let soc = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let days = crate::baboon_runtime::bin_tools::read_i32(reader)?;
        Ok(ScheduleSpec {
            start_s,
            duration_s,
            discharge,
            soc,
            days,
        })
    }
}