use common::influxdb::{HttpResponse, HttpTransport, InfluxDbConfig, InfluxDbSink};
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
        "aquarium_telemetry source=hardware quality=ok ec_us_cm=450,temp_c=26.187 1770300600000000000"
    );
}
