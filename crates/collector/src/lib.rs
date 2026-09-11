use common::{Quality, SourceId, TelemetrySample};
use std::error::Error;
use std::fmt;
use std::time::Duration;
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

#[derive(Debug)]
pub enum CollectionError<E> {
    Source(E),
    Parse { frame: String, error: ParseError },
}

impl<E: fmt::Display> fmt::Display for CollectionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source error: {error}"),
            Self::Parse { frame, error } => {
                write!(formatter, "could not parse raw frame {frame:?}: {error}")
            }
        }
    }
}

impl<E: Error + 'static> Error for CollectionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Parse { error, .. } => Some(error),
        }
    }
}

/// A failure observed while polling. Source and parse failures are reported
/// and polling continues; sink failures stop polling immediately.
#[derive(Debug)]
pub enum PollError<SourceError, SinkError> {
    Source(SourceError),
    Parse { frame: String, error: ParseError },
    Sink(SinkError),
}

impl<SourceError: fmt::Display, SinkError: fmt::Display> fmt::Display
    for PollError<SourceError, SinkError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source error: {error}"),
            Self::Parse { frame, error } => {
                write!(formatter, "could not parse raw frame {frame:?}: {error}")
            }
            Self::Sink(error) => write!(formatter, "sink error: {error}"),
        }
    }
}

impl<SourceError: Error + 'static, SinkError: Error + 'static> Error
    for PollError<SourceError, SinkError>
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Parse { error, .. } => Some(error),
            Self::Sink(error) => Some(error),
        }
    }
}

impl<SourceError, SinkError> From<CollectionError<SourceError>>
    for PollError<SourceError, SinkError>
{
    fn from(error: CollectionError<SourceError>) -> Self {
        match error {
            CollectionError::Source(error) => Self::Source(error),
            CollectionError::Parse { frame, error } => Self::Parse { frame, error },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollingConfig {
    pub attempts: usize,
    pub interval: Duration,
}

#[derive(Debug)]
pub struct PollingReport<SourceError> {
    pub successful_samples: usize,
    pub failures: Vec<CollectionError<SourceError>>,
}

impl<SourceError> Default for PollingReport<SourceError> {
    fn default() -> Self {
        Self {
            successful_samples: 0,
            failures: Vec::new(),
        }
    }
}

pub type PollingResult<SourceError, SinkError> =
    Result<PollingReport<SourceError>, PollError<SourceError, SinkError>>;

/// Poll without sleeping. This is the deterministic, step-oriented API for
/// callers that provide their own scheduling.
pub fn poll_steps<S, K>(
    source: &mut S,
    sink: &mut K,
    config: PollingConfig,
) -> PollingResult<S::Error, K::Error>
where
    S: Source,
    K: Sink,
{
    poll_with_sleep(source, sink, config, |_| {})
}

/// Poll with an injected sleep operation. The operation runs between attempts.
pub fn poll_with_sleep<S, K, F>(
    source: &mut S,
    sink: &mut K,
    config: PollingConfig,
    mut sleep: F,
) -> PollingResult<S::Error, K::Error>
where
    S: Source,
    K: Sink,
    F: FnMut(Duration),
{
    let mut report = PollingReport::default();
    for attempt in 0..config.attempts {
        match collect_once(source) {
            Ok(sample) => {
                sink.write(sample).map_err(PollError::Sink)?;
                report.successful_samples += 1;
            }
            Err(error) => report.failures.push(error),
        }
        if attempt + 1 < config.attempts {
            sleep(config.interval);
        }
    }
    Ok(report)
}

/// Poll at wall-clock intervals using the standard synchronous sleeper.
pub fn poll<S, K>(
    source: &mut S,
    sink: &mut K,
    config: PollingConfig,
) -> PollingResult<S::Error, K::Error>
where
    S: Source,
    K: Sink,
{
    poll_with_sleep(source, sink, config, std::thread::sleep)
}

/// Read and normalize exactly one frame from a source.
pub fn collect_once<S: Source>(
    source: &mut S,
) -> Result<TelemetrySample, CollectionError<S::Error>> {
    let frame = source.read_frame().map_err(CollectionError::Source)?;
    parse_ec_frame(&frame).map_err(|error| CollectionError::Parse { frame, error })
}

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
    use std::collections::VecDeque;

    #[derive(Debug)]
    struct FakeSourceError;

    impl fmt::Display for FakeSourceError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("fake source failed")
        }
    }

    impl Error for FakeSourceError {}

    struct FakeSource(Option<Result<String, FakeSourceError>>);

    impl Source for FakeSource {
        type Error = FakeSourceError;

        fn read_frame(&mut self) -> Result<String, Self::Error> {
            self.0.take().expect("fake source was read only once")
        }
    }

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

    #[test]
    fn collects_one_frame_from_source() {
        let mut source = FakeSource(Some(Ok("?R,EC,450.00\n\r".to_owned())));
        let sample = collect_once(&mut source).expect("valid frame");
        assert_eq!(sample.ec_us_cm, Some(450.0));
    }

    #[test]
    fn keeps_source_errors_distinguishable() {
        let mut source = FakeSource(Some(Err(FakeSourceError)));
        let error = collect_once(&mut source).expect_err("source should fail");
        assert!(matches!(error, CollectionError::Source(_)));
        assert_eq!(error.to_string(), "source error: fake source failed");
    }

    #[test]
    fn keeps_malformed_frame_and_parse_error_distinguishable() {
        let frame = "not an ec frame".to_owned();
        let mut source = FakeSource(Some(Ok(frame.clone())));
        let error = collect_once(&mut source).expect_err("frame should fail to parse");
        assert!(matches!(
            error,
            CollectionError::Parse { frame: actual, error: ParseError::WrongFieldCount }
                if actual == frame
        ));
    }

    #[test]
    fn collects_simulator_style_response_with_ok_status() {
        let mut source = FakeSource(Some(Ok("?R,EC,450.00\n\r*OK\n\r".to_owned())));
        let sample = collect_once(&mut source).expect("valid simulator response");
        assert_eq!(sample.ec_us_cm, Some(450.0));
    }

    #[derive(Debug)]
    struct FakeSinkError;

    impl fmt::Display for FakeSinkError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("fake sink failed")
        }
    }

    impl Error for FakeSinkError {}

    struct SequenceSource(VecDeque<Result<String, FakeSourceError>>);

    impl Source for SequenceSource {
        type Error = FakeSourceError;

        fn read_frame(&mut self) -> Result<String, Self::Error> {
            self.0.pop_front().expect("unexpected source read")
        }
    }

    struct RecordingSink {
        samples: Vec<TelemetrySample>,
        fail: bool,
    }

    impl Sink for RecordingSink {
        type Error = FakeSinkError;

        fn write(&mut self, sample: TelemetrySample) -> Result<(), Self::Error> {
            if self.fail {
                return Err(FakeSinkError);
            }
            self.samples.push(sample);
            Ok(())
        }
    }

    fn config(attempts: usize) -> PollingConfig {
        PollingConfig {
            attempts,
            interval: Duration::from_millis(25),
        }
    }

    #[test]
    fn polls_multiple_samples_and_reports_the_success_count() {
        let mut source = SequenceSource(VecDeque::from([
            Ok("?R,EC,450.00".to_owned()),
            Ok("?R,EC,452.50".to_owned()),
        ]));
        let mut sink = RecordingSink {
            samples: Vec::new(),
            fail: false,
        };

        let report = poll_steps(&mut source, &mut sink, config(2)).expect("polling succeeds");

        assert_eq!(report.successful_samples, 2);
        assert_eq!(report.failures.len(), 0);
        assert_eq!(sink.samples.len(), 2);
    }

    #[test]
    fn source_and_parse_failures_are_reported_and_polling_continues() {
        let mut source = SequenceSource(VecDeque::from([
            Err(FakeSourceError),
            Ok("malformed".to_owned()),
            Ok("?R,EC,450.00".to_owned()),
        ]));
        let mut sink = RecordingSink {
            samples: Vec::new(),
            fail: false,
        };

        let report = poll_steps(&mut source, &mut sink, config(3)).expect("polling succeeds");

        assert_eq!(report.successful_samples, 1);
        assert!(matches!(report.failures[0], CollectionError::Source(_)));
        assert!(matches!(report.failures[1], CollectionError::Parse { .. }));
        assert_eq!(sink.samples.len(), 1);
    }

    #[test]
    fn sink_failure_is_returned_and_stops_polling() {
        let mut source = SequenceSource(VecDeque::from([
            Ok("?R,EC,450.00".to_owned()),
            Ok("?R,EC,452.50".to_owned()),
        ]));
        let mut sink = RecordingSink {
            samples: Vec::new(),
            fail: true,
        };

        let error = poll_steps(&mut source, &mut sink, config(2)).expect_err("sink should fail");

        assert!(matches!(error, PollError::Sink(_)));
        assert!(sink.samples.is_empty());
    }

    #[test]
    fn polling_honors_attempts_and_injected_intervals() {
        let mut source = SequenceSource(VecDeque::from([
            Ok("?R,EC,450.00".to_owned()),
            Ok("?R,EC,452.50".to_owned()),
            Ok("?R,EC,455.00".to_owned()),
        ]));
        let mut sink = RecordingSink {
            samples: Vec::new(),
            fail: false,
        };
        let mut sleeps = Vec::new();

        let report = poll_with_sleep(&mut source, &mut sink, config(2), |duration| {
            sleeps.push(duration)
        })
        .expect("polling succeeds");

        assert_eq!(report.successful_samples, 2);
        assert_eq!(sleeps, vec![Duration::from_millis(25)]);
    }
}
