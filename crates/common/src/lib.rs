use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct TelemetrySample {
    pub timestamp: OffsetDateTime,
    pub ec_us_cm: Option<f32>,
    pub temp_c: Option<f32>,
    pub source: SourceId,
    pub quality: Quality,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum SourceId {
    #[default] Simulator,
    EzoEc,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Quality {
    #[default] Ok,
    Invalid,
}
