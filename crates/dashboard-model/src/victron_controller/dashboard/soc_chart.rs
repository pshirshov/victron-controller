use crate::victron_controller::dashboard::soc_history_sample::SocHistorySample;
use crate::victron_controller::dashboard::soc_projection::SocProjection;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SocChart {
    pub history: Vec<SocHistorySample>,
    pub projection: SocProjection,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub now_epoch_ms: i64,
    pub now_soc_pct: Option<f64>,
    pub discharge_target_pct: Option<f64>,
    pub charge_target_pct: Option<f64>,
}

impl PartialEq for SocChart {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for SocChart {}

impl PartialOrd for SocChart {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SocChart {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.history.cmp(&other.history) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.projection.cmp(&other.projection) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.now_epoch_ms.cmp(&other.now_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.now_soc_pct, &other.now_soc_pct) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.discharge_target_pct, &other.discharge_target_pct) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__opt_f64_total_cmp(&self.charge_target_pct, &other.charge_target_pct) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for SocChart {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        5
    }
}

impl crate::baboon_runtime::BaboonBinEncode for SocChart {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.history.len() as i32)?;
            for item in (value.history).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                value.projection.encode_ueba(ctx, &mut buffer)?;
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            value.now_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                match &value.now_soc_pct {
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
                match &value.discharge_target_pct {
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
                match &value.charge_target_pct {
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
            crate::baboon_runtime::bin_tools::write_i32(writer, value.history.len() as i32)?;
            for item in (value.history).iter() {
                item.encode_ueba(ctx, writer)?;
            }
            value.projection.encode_ueba(ctx, writer)?;
            value.now_epoch_ms.encode_ueba(ctx, writer)?;
            match &value.now_soc_pct {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            match &value.discharge_target_pct {
                None => crate::baboon_runtime::bin_tools::write_byte(writer, 0)?,
                Some(v) => {
                    crate::baboon_runtime::bin_tools::write_byte(writer, 1)?;
                    v.encode_ueba(ctx, writer)?;
                }
            }
            match &value.charge_target_pct {
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

impl crate::baboon_runtime::BaboonBinDecode for SocChart {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let history = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(SocHistorySample::decode_ueba(ctx, reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        let projection = SocProjection::decode_ueba(ctx, reader)?;
        let now_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let now_soc_pct = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        let discharge_target_pct = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        let charge_target_pct = {
            let tag = crate::baboon_runtime::bin_tools::read_byte(reader)?;
            if tag == 0 { None } else { Some(crate::baboon_runtime::bin_tools::read_f64(reader)?) }
        };
        Ok(SocChart {
            history,
            projection,
            now_epoch_ms,
            now_soc_pct,
            discharge_target_pct,
            charge_target_pct,
        })
    }
}