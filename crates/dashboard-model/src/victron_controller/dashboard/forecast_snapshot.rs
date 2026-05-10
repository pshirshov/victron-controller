

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ForecastSnapshot {
    pub today_kwh: f64,
    pub tomorrow_kwh: f64,
    #[serde(deserialize_with = "crate::baboon_runtime::lenient_numeric::deserialize")]
    pub fetched_at_epoch_ms: i64,
    pub hourly_kwh: Vec<f64>,
    pub hourly_temperature_c: Vec<f64>,
}

impl PartialEq for ForecastSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ForecastSnapshot {}

impl PartialOrd for ForecastSnapshot {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ForecastSnapshot {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.today_kwh.total_cmp(&other.today_kwh) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.tomorrow_kwh.total_cmp(&other.tomorrow_kwh) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match self.fetched_at_epoch_ms.cmp(&other.fetched_at_epoch_ms) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__vec_f64_total_cmp(&self.hourly_kwh, &other.hourly_kwh) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        match crate::baboon_runtime::__vec_f64_total_cmp(&self.hourly_temperature_c, &other.hourly_temperature_c) {
            std::cmp::Ordering::Equal => {},
            ord => return ord,
        }
        std::cmp::Ordering::Equal
    }
}

impl crate::baboon_runtime::BaboonBinCodecIndexed for ForecastSnapshot {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        2
    }
}

impl crate::baboon_runtime::BaboonBinEncode for ForecastSnapshot {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.today_kwh.encode_ueba(ctx, &mut buffer)?;
            value.tomorrow_kwh.encode_ueba(ctx, &mut buffer)?;
            value.fetched_at_epoch_ms.encode_ueba(ctx, &mut buffer)?;
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.hourly_kwh.len() as i32)?;
            for item in (value.hourly_kwh).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            {
                let before = buffer.len();
                crate::baboon_runtime::bin_tools::write_i32(writer, before as i32)?;
                crate::baboon_runtime::bin_tools::write_i32(&mut buffer, value.hourly_temperature_c.len() as i32)?;
            for item in (value.hourly_temperature_c).iter() {
                item.encode_ueba(ctx, &mut buffer)?;
            }
                let after = buffer.len();
                let length = after - before;
                crate::baboon_runtime::bin_tools::write_i32(writer, length as i32)?;
            }
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.today_kwh.encode_ueba(ctx, writer)?;
            value.tomorrow_kwh.encode_ueba(ctx, writer)?;
            value.fetched_at_epoch_ms.encode_ueba(ctx, writer)?;
            crate::baboon_runtime::bin_tools::write_i32(writer, value.hourly_kwh.len() as i32)?;
            for item in (value.hourly_kwh).iter() {
                item.encode_ueba(ctx, writer)?;
            }
            crate::baboon_runtime::bin_tools::write_i32(writer, value.hourly_temperature_c.len() as i32)?;
            for item in (value.hourly_temperature_c).iter() {
                item.encode_ueba(ctx, writer)?;
            }
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for ForecastSnapshot {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let today_kwh = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let tomorrow_kwh = crate::baboon_runtime::bin_tools::read_f64(reader)?;
        let fetched_at_epoch_ms = crate::baboon_runtime::bin_tools::read_i64(reader)?;
        let hourly_kwh = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(crate::baboon_runtime::bin_tools::read_f64(reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        let hourly_temperature_c = {
            let count = crate::baboon_runtime::bin_tools::read_i32(reader)? as usize;
            (0..count).map(|_| Ok(crate::baboon_runtime::bin_tools::read_f64(reader)?)).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?
        };
        Ok(ForecastSnapshot {
            today_kwh,
            tomorrow_kwh,
            fetched_at_epoch_ms,
            hourly_kwh,
            hourly_temperature_c,
        })
    }
}