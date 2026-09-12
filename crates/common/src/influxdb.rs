use crate::{Sink, TelemetrySample};
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;

pub const MEASUREMENT: &str = "aquarium_telemetry";
pub const PRECISION: &str = "ns";

/// Explicit InfluxDB connection settings. The token has no implicit default.
pub struct InfluxDbConfig {
    pub url: String,
    pub organization: String,
    pub bucket: String,
    pub token: String,
}

impl InfluxDbConfig {
    pub fn new(
        url: impl Into<String>,
        organization: impl Into<String>,
        bucket: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            organization: organization.into(),
            bucket: bucket.into(),
            token: token.into(),
        }
    }
}

impl fmt::Debug for InfluxDbConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InfluxDbConfig")
            .field("url", &self.url)
            .field("organization", &self.organization)
            .field("bucket", &self.bucket)
            .field("token", &"[redacted]")
            .finish()
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Minimal synchronous HTTP boundary, deliberately easy to replace in tests.
pub trait HttpTransport {
    type Error: Error + Send + Sync + 'static;

    fn post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<HttpResponse, Self::Error>;
}

#[derive(Debug)]
pub enum InfluxDbError<E> {
    InvalidConfiguration(&'static str),
    Encoding(LineProtocolError),
    Transport(E),
    HttpStatus { status: u16 },
}

impl<E> fmt::Display for InfluxDbError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(f, "invalid InfluxDB configuration: {message}")
            }
            Self::Encoding(error) => write!(f, "invalid InfluxDB line protocol: {error}"),
            Self::Transport(_) => f.write_str("InfluxDB HTTP transport failed"),
            Self::HttpStatus { status } => write!(f, "InfluxDB returned HTTP status {status}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineProtocolError {
    InvalidFieldValue(&'static str),
    NoFields,
}

impl fmt::Display for LineProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFieldValue(field) => write!(f, "invalid field value: {field}"),
            Self::NoFields => f.write_str("at least one measurement is required"),
        }
    }
}

impl Error for LineProtocolError {}

impl<E: Error + 'static> Error for InfluxDbError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

pub struct InfluxDbSink<T> {
    config: InfluxDbConfig,
    transport: T,
}

impl<T> InfluxDbSink<T> {
    pub fn new(config: InfluxDbConfig, transport: T) -> Result<Self, InfluxDbError<T::Error>>
    where
        T: HttpTransport,
    {
        validate_url(&config.url)?;
        if config.organization.is_empty() || config.bucket.is_empty() || config.token.is_empty() {
            return Err(InfluxDbError::InvalidConfiguration(
                "organization, bucket, and token are required",
            ));
        }
        Ok(Self { config, transport })
    }

    pub fn into_inner(self) -> T {
        self.transport
    }
}

impl<T: HttpTransport> Sink for InfluxDbSink<T> {
    type Error = InfluxDbError<T::Error>;

    fn write(&mut self, sample: TelemetrySample) -> Result<(), Self::Error> {
        let body = encode_line_protocol(&sample).map_err(InfluxDbError::Encoding)?;
        let url = write_url(&self.config);
        let auth = format!("Token {}", self.config.token);
        let response = self
            .transport
            .post(
                &url,
                &[
                    ("Authorization", &auth),
                    ("Content-Type", "text/plain; charset=utf-8"),
                ],
                body.as_bytes(),
            )
            .map_err(InfluxDbError::Transport)?;
        if (200..300).contains(&response.status) {
            Ok(())
        } else {
            Err(InfluxDbError::HttpStatus {
                status: response.status,
            })
        }
    }
}

pub fn encode_line_protocol(sample: &TelemetrySample) -> Result<String, LineProtocolError> {
    if sample.ec_us_cm.is_some_and(|value| !value.is_finite()) {
        return Err(LineProtocolError::InvalidFieldValue("ec_us_cm"));
    }
    if sample.temp_c.is_some_and(|value| !value.is_finite()) {
        return Err(LineProtocolError::InvalidFieldValue("temp_c"));
    }
    let mut line = format!(
        "{} tank_id={} source={} quality={}",
        MEASUREMENT,
        tag_value(&sample.tank_id),
        tag_value(&sample.source.to_string()),
        tag_value(&sample.quality.to_string())
    );
    let mut fields = Vec::new();
    if let Some(value) = sample.ec_us_cm {
        fields.push(format!("ec_us_cm={value}"));
    }
    if let Some(value) = sample.temp_c {
        fields.push(format!("temp_c={value}"));
    }
    if fields.is_empty() {
        return Err(LineProtocolError::NoFields);
    }
    line.push(' ');
    line.push_str(&fields.join(","));
    line.push(' ');
    line.push_str(&sample.timestamp.unix_timestamp_nanos().to_string());
    Ok(line)
}

fn tag_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
        .replace('=', "\\=")
}

fn validate_url<E: Error + Send + Sync + 'static>(url: &str) -> Result<(), InfluxDbError<E>> {
    if !url.starts_with("http://") {
        return Err(InfluxDbError::InvalidConfiguration(
            "only http:// URLs are supported; HTTPS is not supported yet",
        ));
    }
    if url[7..].is_empty() || url[7..].starts_with('/') {
        return Err(InfluxDbError::InvalidConfiguration(
            "URL must include an HTTP host",
        ));
    }
    Ok(())
}

fn write_url(config: &InfluxDbConfig) -> String {
    format!(
        "{}/api/v2/write?org={}&bucket={}&precision={}",
        config.url.trim_end_matches('/'),
        query_value(&config.organization),
        query_value(&config.bucket),
        PRECISION
    )
}

fn query_value(value: &str) -> String {
    percent_encode(value)
}
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct StdHttpTransport;

impl HttpTransport for StdHttpTransport {
    type Error = io::Error;

    fn post(
        &mut self,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> io::Result<HttpResponse> {
        if !url.starts_with("http://") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "only http:// URLs are supported",
            ));
        }
        let authority_and_path = &url[7..];
        let (authority, path) = authority_and_path
            .split_once('/')
            .unwrap_or((authority_and_path, ""));
        let (host, port) = authority
            .rsplit_once(':')
            .map_or((authority, 80), |(host, port)| {
                (host, port.parse().unwrap_or(80))
            });
        let mut stream = TcpStream::connect((host, port))?;
        write!(
            stream,
            "POST /{} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            path,
            authority,
            body.len()
        )?;
        for (name, value) in headers {
            write!(stream, "{}: {}\r\n", name, value)?;
        }
        write!(stream, "\r\n")?;
        stream.write_all(body)?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed HTTP response"))?;
        let status = response
            .get(9..12)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|text| text.parse().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing HTTP status"))?;
        Ok(HttpResponse {
            status,
            body: response[header_end + 4..].to_vec(),
        })
    }
}

impl fmt::Display for crate::SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            crate::SourceId::Simulator => "sim",
            crate::SourceId::EzoEc => "ezo_ec",
            crate::SourceId::Hardware => "hardware",
        })
    }
}
impl fmt::Display for crate::Quality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            crate::Quality::Ok => "ok",
            crate::Quality::Invalid => "invalid",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Quality, SourceId};
    use time::OffsetDateTime;

    #[derive(Debug)]
    struct MockError;
    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("mock")
        }
    }
    impl Error for MockError {}
    type CapturedRequest = (String, Vec<(String, String)>, Vec<u8>);
    struct Mock {
        request: Option<CapturedRequest>,
        status: u16,
    }
    impl HttpTransport for Mock {
        type Error = MockError;
        fn post(
            &mut self,
            url: &str,
            headers: &[(&str, &str)],
            body: &[u8],
        ) -> Result<HttpResponse, MockError> {
            self.request = Some((
                url.into(),
                headers
                    .iter()
                    .map(|(a, b)| ((*a).into(), (*b).into()))
                    .collect(),
                body.into(),
            ));
            Ok(HttpResponse {
                status: self.status,
                body: Vec::new(),
            })
        }
    }
    fn sample(ec: Option<f32>, temp: Option<f32>) -> crate::TelemetrySample {
        crate::TelemetrySample {
            tank_id: "tank, west =\\zone".into(),
            timestamp: OffsetDateTime::from_unix_timestamp_nanos(1_234_567_890_123_456_789)
                .unwrap(),
            ec_us_cm: ec,
            temp_c: temp,
            source: SourceId::Hardware,
            quality: Quality::Ok,
        }
    }

    #[test]
    fn line_protocol_has_escaped_tags_optional_fields_and_ns_timestamp() {
        let line = encode_line_protocol(&sample(Some(450.0), Some(26.187))).unwrap();
        assert_eq!(
            line,
            "aquarium_telemetry tank_id=tank\\,\\ west\\ \\=\\\\zone source=hardware quality=ok ec_us_cm=450,temp_c=26.187 1234567890123456789"
        );
    }

    #[test]
    fn absent_fields_are_not_emitted() {
        assert_eq!(
            encode_line_protocol(&sample(Some(450.0), None)).unwrap(),
            "aquarium_telemetry tank_id=tank\\,\\ west\\ \\=\\\\zone source=hardware quality=ok ec_us_cm=450 1234567890123456789"
        );
        assert_eq!(
            encode_line_protocol(&sample(None, Some(26.187))).unwrap(),
            "aquarium_telemetry tank_id=tank\\,\\ west\\ \\=\\\\zone source=hardware quality=ok temp_c=26.187 1234567890123456789"
        );
    }
    #[test]
    fn query_and_auth_are_constructed_without_leaking_token_in_errors() {
        let mock = Mock {
            request: None,
            status: 204,
        };
        let mut sink = InfluxDbSink::new(
            InfluxDbConfig::new(
                "http://localhost:8086/",
                "my org",
                "my bucket",
                "secret-token",
            ),
            mock,
        )
        .unwrap();
        sink.write(sample(Some(1.0), None)).unwrap();
        let request = sink.into_inner().request.unwrap();
        assert_eq!(
            request.0,
            "http://localhost:8086/api/v2/write?org=my%20org&bucket=my%20bucket&precision=ns"
        );
        assert!(
            request
                .1
                .iter()
                .any(|(name, value)| name == "Authorization" && value == "Token secret-token")
        );
        let result = InfluxDbSink::new(
            InfluxDbConfig::new("https://localhost", "org", "bucket", "secret-token"),
            Mock {
                request: None,
                status: 204,
            },
        );
        let error = match result {
            Ok(_) => panic!("HTTPS should be rejected"),
            Err(error) => error,
        };
        assert!(!error.to_string().contains("secret-token"));
    }
    #[test]
    fn non_success_is_typed_and_redacted() {
        let mut sink = InfluxDbSink::new(
            InfluxDbConfig::new("http://localhost", "org", "bucket", "secret-token"),
            Mock {
                request: None,
                status: 500,
            },
        )
        .unwrap();
        let error = sink.write(sample(None, Some(20.0))).unwrap_err();
        assert!(matches!(error, InfluxDbError::HttpStatus { status: 500 }));
        assert!(!error.to_string().contains("secret-token"));
    }
    #[test]
    fn tags_escape_protocol_delimiters() {
        assert_eq!(tag_value("a,b c=d\\e"), "a\\,b\\ c\\=d\\\\e");
    }
}
