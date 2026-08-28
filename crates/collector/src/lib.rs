use common::{Quality, SourceId, TelemetrySample};

pub fn parse_ec_frame(frame: &str) -> Result<TelemetrySample, String> {
    let parts: Vec<&str> = frame.split(',').map(|f: &str| f.trim()).collect();
    let [response_type, measurement_type, value] = parts.as_slice() else {
        return Err(format!("Invalid frame format: {}", frame));
    };
    if *response_type != "?R" || *measurement_type != "EC" {
        return Err(format!("Invalid frame format: {}", frame));
    }

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
    use common::{Quality, SourceId};
    use parameterized::parameterized;

    #[parameterized(frame = {
        "?R,EC,450.00\n\r", "?R,EC,0.00", "?R,EC,132.40\n", "?R,EC,1000"
    }, expected = { 450.00, 0.00, 132.40, 1000.00 })]
    fn test_valid_ec_frames(frame: &str, expected: f32) {
        let sample = parse_ec_frame(frame).expect("valid EC frame");
        assert_eq!(sample.ec_us_cm, Some(expected));
        assert_eq!(sample.temp_c, None);
        assert_eq!(sample.source, SourceId::EzoEc);
        assert_eq!(sample.quality, Quality::Ok);
    }

    #[parameterized(frame = {
        "garbage",
        "?R,EC",
        "?R,EC,",
        "?R,EC,abc",
        "?R,TEMP,25.0",
        "?X,EC,450.00",
        "garbage,anything,450.00"
    })]
    fn rejects_invalid_ec_frames(frame: &str) {
        assert!(
            parse_ec_frame(frame).is_err(),
            "expected invalid frame to be rejected: {frame}"
        );
    }
}
