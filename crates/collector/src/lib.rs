use common::{Quality, SourceId, TelemetrySample};

pub fn parse_ec_frame(frame: &str) -> Result<TelemetrySample, String> {
    let parts: Vec<&str> = frame.split(',').map(|f: &str| f.trim()).collect();
    let [_response_type, _measurement_type, value] = parts.as_slice() else {
        return Err(format!("Invalid frame format: {}", frame));
    };
    Ok(TelemetrySample {
        timestamp: time::OffsetDateTime::now_utc(),
        ec_us_cm: Some(
            value
                .parse::<f32>()
                .map_err(|e| format!("Failed to parse EC value: {}", e))?,
        ),
        temp_c: None,
        source: SourceId::EzoEc,
        quality: Quality::Ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use parameterized::parameterized;

    #[parameterized(frame = {
        "?R,EC,450.00\n\r", "?R,EC,0.00", "?R,EC,132.40\n", "?R,EC,1000"
    })]
    fn test_valid_ec_frames(frame: &str) {
        let result = parse_ec_frame(frame);
        assert!(result.is_ok());
        let sample = result.unwrap();
        assert_eq!(sample.source, SourceId::EzoEc);
        assert_eq!(sample.quality, Quality::Ok);
    }
}
