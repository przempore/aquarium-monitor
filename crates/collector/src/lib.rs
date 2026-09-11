use common::{Quality, SourceId, TelemetrySample};
use std::error::Error;
use std::fmt;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongFieldCount,
    WrongResponseType,
    WrongMeasurementType,
    EmptyValue,
    InvalidValue,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::WrongFieldCount => "wrong field count",
            Self::WrongResponseType => "wrong response type",
            Self::WrongMeasurementType => "wrong measurement type",
            Self::EmptyValue => "empty measurement value",
            Self::InvalidValue => "invalid measurement value",
        };
        formatter.write_str(message)
    }
}

impl Error for ParseError {}

/// Parse one EZO-EC read response, with or without its trailing `*OK` status.
pub fn parse_ec_frame(frame: &str) -> Result<TelemetrySample, ParseError> {
    let frame = frame
        .strip_suffix("\r\n")
        .or_else(|| frame.strip_suffix("\n\r"))
        .or_else(|| frame.strip_suffix('\n'))
        .or_else(|| frame.strip_suffix('\r'))
        .unwrap_or(frame);
    let (frame, status) = frame
        .split_once("\r\n")
        .or_else(|| frame.split_once("\n\r"))
        .unwrap_or((frame, ""));
    if !status.is_empty() && status != "*OK" {
        return Err(ParseError::WrongResponseType);
    }
    let fields: Vec<&str> = frame.split(',').collect();
    if fields.len() != 3 {
        return Err(ParseError::WrongFieldCount);
    }
    if fields[0] != "?R" {
        return Err(ParseError::WrongResponseType);
    }
    if fields[1] != "EC" {
        return Err(ParseError::WrongMeasurementType);
    }
    if fields[2].is_empty() {
        return Err(ParseError::EmptyValue);
    }
    let value = fields[2]
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidValue)?;
    if !value.is_finite() || value < 0.0 {
        return Err(ParseError::InvalidValue);
    }

    Ok(TelemetrySample {
        timestamp: OffsetDateTime::now_utc(),
        ec_us_cm: Some(value),
        temp_c: None,
        source: SourceId::EzoEc,
        quality: Quality::Ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_frames() {
        for (frame, expected) in [
            ("?R,EC,450.00\n\r", 450.0),
            ("?R,EC,0.00", 0.0),
            ("?R,EC,132.40\n", 132.4),
        ] {
            let sample = parse_ec_frame(frame).expect("valid frame");
            assert_eq!(sample.ec_us_cm, Some(expected));
            assert_eq!(sample.source, SourceId::EzoEc);
            assert_eq!(sample.quality, Quality::Ok);
        }
    }

    #[test]
    fn rejects_invalid_frames() {
        for frame in [
            "garbage",
            "?R,EC",
            "?R,EC,",
            "?R,EC,abc",
            "?R,EC,-1",
            "?R,EC,NaN",
            "?R,TEMP,25.0",
            "?X,EC,450.00",
            "?R,EC,450,extra",
            "?R, EC,450",
            "?R,EC,450.00\n\r*ER\n\r",
        ] {
            assert!(
                parse_ec_frame(frame).is_err(),
                "accepted invalid frame: {frame:?}"
            );
        }
    }

    #[test]
    fn parses_simulator_response_with_status() {
        let sample = parse_ec_frame("?R,EC,450.00\n\r*OK\n\r").expect("valid response");
        assert_eq!(sample.ec_us_cm, Some(450.0));
    }

}
