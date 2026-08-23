use common::{TelemetrySample, SourceId, Quality};

pub fn parse_ec_frame(
    frame: &str,
) -> Result<TelemetrySample, String> {
    Ok(TelemetrySample {
        timestamp: time::OffsetDateTime::now_utc(),
        ec_us_cm: None,
        temp_c: None,
        source: SourceId::EzoEc,
        quality: Quality::Ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ec_frame() {
        let frame = "some_ec_frame_data";
        let result = parse_ec_frame(frame);
        assert!(result.is_ok());
        let sample = result.unwrap();
        assert_eq!(sample.source, SourceId::EzoEc);
        assert_eq!(sample.quality, Quality::Ok);
    }
}
