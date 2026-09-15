use common::influxdb::{
    AlarmSink, HttpResponse, HttpTransport, InfluxDbConfig, InfluxDbSink,
    encode_alarm_line_protocol,
};
use common::rules::{AlarmEvent, Severity};
use common::{Quality, Sink, SourceId, TelemetrySample};
use std::error::Error;
use std::fmt;
use time::OffsetDateTime;

#[derive(Debug)]
struct FakeTransportError;
impl fmt::Display for FakeTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fake transport")
    }
}
impl Error for FakeTransportError {}

struct FakeTransport {
    body: Vec<u8>,
    status: u16,
}
impl HttpTransport for FakeTransport {
    type Error = FakeTransportError;
    fn post(
        &mut self,
        _url: &str,
        _headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<HttpResponse, Self::Error> {
        self.body = body.to_vec();
        Ok(HttpResponse {
            status: self.status,
            body: Vec::new(),
        })
    }
}

#[test]
fn combined_ec_and_ds18b20_sample_is_written_without_a_live_database() {
    let sample = TelemetrySample {
        tank_id: "tank-1".into(),
        timestamp: OffsetDateTime::from_unix_timestamp(1_770_300_600).unwrap(),
        ec_us_cm: Some(450.0),
        temp_c: Some(26.187),
        source: SourceId::Hardware,
        quality: Quality::Ok,
    };
    let transport = FakeTransport {
        body: Vec::new(),
        status: 204,
    };
    let mut sink = InfluxDbSink::new(
        InfluxDbConfig::new("http://127.0.0.1:8086", "org", "bucket", "test-token"),
        transport,
    )
    .unwrap();
    sink.write(sample).unwrap();
    let body = String::from_utf8(sink.into_inner().body).unwrap();
    assert_eq!(
        body,
        "aquarium_telemetry,tank_id=tank-1,source=hardware,quality=ok ec_us_cm=450,temp_c=26.187 1770300600000000000"
    );
}

#[test]
fn alarm_line_protocol_contains_context_without_token() {
    let alarm = AlarmEvent {
        tank_id: "tank, west".into(),
        rule_id: "ec.minimum".into(),
        severity: Severity::Critical,
        reason: "value \"too low\"".into(),
        observed_value: Some(42.5),
        timestamp: OffsetDateTime::from_unix_timestamp(1_770_300_600).unwrap(),
    };
    let line = encode_alarm_line_protocol(&alarm).unwrap();
    assert_eq!(
        line,
        "aquarium_alarm,tank_id=tank\\,\\ west,rule_id=ec.minimum,severity=critical reason=\"value \\\"too low\\\"\",observed_value=42.5 1770300600000000000"
    );
    assert!(!line.contains("token"));
}

#[test]
fn alarm_sink_posts_to_the_existing_write_endpoint() {
    let transport = FakeTransport {
        body: Vec::new(),
        status: 204,
    };
    let mut sink = AlarmSink::new(
        InfluxDbConfig::new("http://127.0.0.1:8086", "org", "bucket", "secret-token"),
        transport,
    )
    .unwrap();
    sink.write(AlarmEvent {
        tank_id: "tank-1".into(),
        rule_id: "temperature.maximum".into(),
        severity: Severity::Warning,
        reason: "too warm".into(),
        observed_value: None,
        timestamp: OffsetDateTime::from_unix_timestamp(1_770_300_600).unwrap(),
    })
    .unwrap();
    let transport = sink.into_inner();
    assert!(
        String::from_utf8(transport.body)
            .unwrap()
            .starts_with("aquarium_alarm,tank_id=tank-1")
    );
}
