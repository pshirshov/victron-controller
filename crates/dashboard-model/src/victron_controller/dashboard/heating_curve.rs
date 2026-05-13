use crate::victron_controller::dashboard::heating_curve_bucket::HeatingCurveBucket;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct HeatingCurve {
    pub row_0: HeatingCurveBucket,
    pub row_1: HeatingCurveBucket,
    pub row_2: HeatingCurveBucket,
    pub row_3: HeatingCurveBucket,
    pub row_4: HeatingCurveBucket,
}



impl crate::baboon_runtime::BaboonBinCodecIndexed for HeatingCurve {
    fn index_elements_count(_ctx: &crate::baboon_runtime::BaboonCodecContext) -> u16 {
        0
    }
}

impl crate::baboon_runtime::BaboonBinEncode for HeatingCurve {
    fn encode_ueba(&self, ctx: &crate::baboon_runtime::BaboonCodecContext, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let value = self;
        if ctx.use_indices() {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x01)?;
            let mut buffer: Vec<u8> = Vec::new();
            value.row_0.encode_ueba(ctx, &mut buffer)?;
            value.row_1.encode_ueba(ctx, &mut buffer)?;
            value.row_2.encode_ueba(ctx, &mut buffer)?;
            value.row_3.encode_ueba(ctx, &mut buffer)?;
            value.row_4.encode_ueba(ctx, &mut buffer)?;
            writer.write_all(&buffer)?;
        } else {
            crate::baboon_runtime::bin_tools::write_byte(writer, 0x00)?;
            value.row_0.encode_ueba(ctx, writer)?;
            value.row_1.encode_ueba(ctx, writer)?;
            value.row_2.encode_ueba(ctx, writer)?;
            value.row_3.encode_ueba(ctx, writer)?;
            value.row_4.encode_ueba(ctx, writer)?;
        }
        Ok(())
    }
}

impl crate::baboon_runtime::BaboonBinDecode for HeatingCurve {
    fn decode_ueba(ctx: &crate::baboon_runtime::BaboonCodecContext, reader: &mut dyn std::io::Read) -> Result<Self, Box<dyn std::error::Error>> {
        let (_header, index) = <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::read_index(ctx, reader)?;
        if ctx.use_indices() {
            assert_eq!(index.len(), <Self as crate::baboon_runtime::BaboonBinCodecIndexed>::index_elements_count(ctx) as usize);
        }
        let row_0 = HeatingCurveBucket::decode_ueba(ctx, reader)?;
        let row_1 = HeatingCurveBucket::decode_ueba(ctx, reader)?;
        let row_2 = HeatingCurveBucket::decode_ueba(ctx, reader)?;
        let row_3 = HeatingCurveBucket::decode_ueba(ctx, reader)?;
        let row_4 = HeatingCurveBucket::decode_ueba(ctx, reader)?;
        Ok(HeatingCurve {
            row_0,
            row_1,
            row_2,
            row_3,
            row_4,
        })
    }
}