use serde::{Deserialize, Serialize};
use std::error::Error;
use time::OffsetDateTime;

pub mod influxdb;

/// A synchronous source of complete raw sensor frames.
pub trait Source {
    type Error: Error + Send + Sync + 'static;

    fn read_frame(&mut self) -> Result<String, Self::Error>;
}

/// A synchronous destination for normalized telemetry.
pub trait Sink {
    type Error: Error + Send + Sync + 'static;

    fn write(&mut self, sample: TelemetrySample) -> Result<(), Self::Error>;
}

/// Normalized telemetry shared by sensor adapters and downstream consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySample {
    pub tank_id: String,
    pub timestamp: OffsetDateTime,
    pub ec_us_cm: Option<f32>,
    pub temp_c: Option<f32>,
    pub source: SourceId,
    pub quality: Quality,
}

/// A normalized temperature reading independent of its sensor transport.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TemperatureSample {
    pub temp_c: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceId {
    #[default]
    #[serde(rename = "sim")]
    Simulator,
    #[serde(rename = "ezo_ec")]
    EzoEc,
    #[serde(rename = "hardware")]
    Hardware,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    #[default]
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "invalid")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn timestamp() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_770_300_600).expect("valid timestamp")
    }

    #[test]
    fn telemetry_round_trips_through_json() {
        let sample = TelemetrySample {
            tank_id: "tank-1".to_owned(),
            timestamp: timestamp(),
            ec_us_cm: Some(132.4),
            temp_c: Some(26.4),
            source: SourceId::Simulator,
            quality: Quality::Ok,
        };
        let encoded = serde_json::to_string(&sample).expect("serialize sample");
        let decoded: TelemetrySample = serde_json::from_str(&encoded).expect("deserialize sample");
        assert_eq!(decoded, sample);
    }

    #[test]
    fn optional_measurements_are_preserved() {
        let sample = TelemetrySample {
            tank_id: "tank-1".to_owned(),
            timestamp: timestamp(),
            ec_us_cm: Some(450.0),
            temp_c: None,
            source: SourceId::EzoEc,
            quality: Quality::Ok,
        };
        let encoded = serde_json::to_string(&sample).expect("serialize sample");
        assert!(encoded.contains("\"temp_c\":null"));
        assert!(encoded.contains("\"source\":\"ezo_ec\""));
        assert!(encoded.contains("\"quality\":\"ok\""));
    }
}
