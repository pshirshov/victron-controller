use crate::victron_controller::dashboard::soc_projection_segment::SocProjectionSegment;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SocProjection {
    pub segments: Vec<SocProjectionSegment>,
    pub net_power_w: Option<f64>,
    pub capacity_wh: Option<f64>,
    pub charge_rate_w: Option<f64>,
}

impl PartialEq for SocProjection {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SocProjection {}

impl PartialOrd for SocProjection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SocProjection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.segments.cmp(&other.segments) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.net_power_w, &other.net_power_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.capacity_wh, &other.capacity_wh) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.charge_rate_w, &other.charge_rate_w) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SocProjection {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        4
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SocProjection {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.segments.len() as i32)?;
            for item in (value.segments).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.net_power_w {
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
                match &value.capacity_wh {
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
                match &value.charge_rate_w {
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
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            crate::baboon_runtime::bin_tools::write_i32(writer, value.segments.len() as i32)?;
            for item in (value.segments).iter() {
                item.encode_ueba(ctx, writer)?;
            }
            match &value.net_power_w {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            match &value.capacity_wh {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            match &value.charge_rate_w {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for SocProjection {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let segments = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(SocProjectionSegment::decode_ueba(ctx, reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        let net_power_w = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        let capacity_wh = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        let charge_rate_w = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        Ok(SocProjection {
            segments,
            net_power_w,
            capacity_wh,
            charge_rate_w,
        })
    }
}