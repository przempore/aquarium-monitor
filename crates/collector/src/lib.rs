use common::{Quality, SourceId, TelemetrySample};
use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};
use std::time::Duration;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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

pub use common::{Sink, Source};

/// Reads complete EZO responses from a line-oriented stream.
///
/// EZO responses contain a measurement line followed by a status line. The
/// status line is consumed as part of the same frame, while a following
/// measurement is left buffered for the next call.
pub struct StdinSource<R> {
    reader: R,
}

impl<R: BufRead> StdinSource<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Reads one frame, returning `None` when EOF is reached before any data.
    pub fn read_frame_or_eof(&mut self) -> io::Result<Option<String>> {
        let Some(mut measurement) = self.read_ezo_line()? else {
            return Ok(None);
        };

        if self
            .reader
            .fill_buf()?
            .first()
            .is_some_and(|byte| *byte == b'*')
        {
            let status = self.read_ezo_line()?.unwrap_or_default();
            measurement.push_str(&status);
        }

        Ok(Some(measurement))
    }

    fn read_ezo_line(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if line.ends_with('\n') && self.reader.fill_buf()?.first() == Some(&b'\r') {
            self.reader.consume(1);
            line.push('\r');
        }
        Ok(Some(line))
    }
}

impl<R: BufRead> Source for StdinSource<R> {
    type Error = io::Error;

    fn read_frame(&mut self) -> Result<String, Self::Error> {
        self.read_frame_or_eof()?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "no more EZO-EC frames"))
    }
}

/// Writes normalized samples as one JSON object per line.
pub struct NdjsonSink<W> {
    writer: W,
}

impl<W: Write> NdjsonSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> Sink for NdjsonSink<W> {
    type Error = io::Error;

    fn write(&mut self, sample: TelemetrySample) -> Result<(), Self::Error> {
        let encoded = serde_json::to_vec(&sample).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("serialize telemetry: {error}"),
            )
        })?;
        self.writer
            .write_all(&encoded)
            .and_then(|_| self.writer.write_all(b"\n"))
            .map_err(|error| io::Error::new(error.kind(), format!("write NDJSON sample: {error}")))
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RawFrameRecord<'a> {
    pub timestamp: String,
    pub raw_frame: &'a str,
}

/// Writes every received raw frame as an inspectable JSON object per line.
pub struct RawFrameLogger<W> {
    writer: W,
}

impl<W: Write> RawFrameLogger<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn write_frame(&mut self, raw_frame: &str) -> io::Result<()> {
        let record = RawFrameRecord {
            timestamp: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("format raw frame timestamp: {error}"),
                    )
                })?,
            raw_frame,
        };
        let encoded = serde_json::to_vec(&record).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("serialize raw frame: {error}"),
            )
        })?;
        self.writer
            .write_all(&encoded)
            .and_then(|_| self.writer.write_all(b"\n"))
            .and_then(|_| self.writer.flush())
            .map_err(|error| io::Error::new(error.kind(), format!("write raw frame: {error}")))
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

/// Collects all available frames from a buffered reader into an NDJSON writer.
/// Malformed frames are reported to stderr with their raw contents and do not
/// prevent subsequent frames from being collected.
pub fn collect_reader<R: BufRead, W: Write>(reader: R, writer: W) -> io::Result<usize> {
    collect_reader_with_raw_log(reader, writer, io::sink())
}

/// Collects frames while persisting every received frame before parsing it.
/// Normalized telemetry and raw frames are written to separate destinations.
pub fn collect_reader_with_raw_log<R: BufRead, W: Write, L: Write>(
    reader: R,
    writer: W,
    raw_writer: L,
) -> io::Result<usize> {
    let mut source = StdinSource::new(reader);
    let mut sink = NdjsonSink::new(writer);
    let mut raw_logger = RawFrameLogger::new(raw_writer);
    let mut successful_samples = 0;

    while let Some(frame) = source.read_frame_or_eof()? {
        raw_logger.write_frame(&frame)?;
        match parse_ec_frame(&frame) {
            Ok(sample) => {
                sink.write(sample).map_err(|error| {
                    io::Error::new(error.kind(), format!("collector sink failed: {error}"))
                })?;
                successful_samples += 1;
            }
            Err(error) => {
                eprintln!("collector rejected raw frame {frame:?}: {error}");
            }
        }
    }

    sink.into_inner().flush()?;
    raw_logger.into_inner().flush()?;
    Ok(successful_samples)
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
    use std::io::Cursor;

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

    #[test]
    fn source_keeps_simulator_status_with_its_measurement() {
        let input = Cursor::new(b"?R,EC,450.00\n\r*OK\n\r?R,EC,452.50\n\r*OK\n\r".to_vec());
        let mut source = StdinSource::new(input);

        assert_eq!(
            source.read_frame_or_eof().expect("first frame"),
            Some("?R,EC,450.00\n\r*OK\n\r".to_owned())
        );
        assert_eq!(
            source.read_frame_or_eof().expect("second frame"),
            Some("?R,EC,452.50\n\r*OK\n\r".to_owned())
        );
        assert_eq!(source.read_frame_or_eof().expect("EOF"), None);
    }

    #[test]
    fn source_returns_final_frame_at_eof_without_status() {
        let mut source = StdinSource::new(Cursor::new(b"?R,EC,450.00\n\r".to_vec()));

        assert_eq!(
            source.read_frame_or_eof().expect("final frame"),
            Some("?R,EC,450.00\n\r".to_owned())
        );
        assert_eq!(source.read_frame_or_eof().expect("EOF"), None);
    }

    #[test]
    fn ndjson_sink_writes_one_sample_per_line() {
        let timestamp = OffsetDateTime::from_unix_timestamp(1_770_300_600).expect("timestamp");
        let sample = TelemetrySample {
            timestamp,
            ec_us_cm: Some(450.0),
            temp_c: None,
            source: SourceId::EzoEc,
            quality: Quality::Ok,
        };
        let mut sink = NdjsonSink::new(Vec::new());

        sink.write(sample.clone()).expect("write sample");
        let output = String::from_utf8(sink.into_inner()).expect("UTF-8 output");
        assert_eq!(output.matches('\n').count(), 1);
        let decoded =
            serde_json::from_str::<TelemetrySample>(output.trim()).expect("decode sample");
        assert_eq!(decoded, sample);
    }

    #[test]
    fn malformed_input_does_not_hide_following_valid_frame() {
        let input = Cursor::new(b"malformed\n\r?R,EC,450.00\n\r*OK\n\r".to_vec());
        let mut output = Vec::new();

        let count = collect_reader(input, &mut output).expect("collect frames");

        assert_eq!(count, 1);
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
    }

    #[test]
    fn raw_log_contains_valid_and_malformed_frames_separately_from_normalized_output() {
        let input = Cursor::new(b"malformed\n\r?R,EC,450.00\n\r*OK\n\r".to_vec());
        let mut output = Vec::new();
        let mut raw_log = Vec::new();

        let count =
            collect_reader_with_raw_log(input, &mut output, &mut raw_log).expect("collect frames");

        assert_eq!(count, 1);
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        let records: Vec<serde_json::Value> = raw_log
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("raw record"))
            .collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["raw_frame"], "malformed\n\r");
        assert_eq!(records[1]["raw_frame"], "?R,EC,450.00\n\r*OK\n\r");
        assert!(records.iter().all(|record| record["timestamp"].is_string()));
        assert!(serde_json::from_slice::<TelemetrySample>(&output).is_ok());
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("raw log unavailable"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn raw_log_write_errors_stop_collection() {
        let input = Cursor::new(b"?R,EC,450.00\n\r".to_vec());
        let mut output = Vec::new();

        let error = collect_reader_with_raw_log(input, &mut output, FailingWriter)
            .expect_err("raw logger should fail");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("write raw frame"));
        assert!(output.is_empty());
    }

    #[test]
    fn simulator_response_is_collected_as_normalized_ndjson() {
        let mut simulator = simulator_ezo_ec::EzoEcCore::new();
        let response = simulator.handle_command("R");
        let mut output = Vec::new();

        collect_reader(Cursor::new(response.into_bytes()), &mut output).expect("collect response");

        let sample: TelemetrySample = serde_json::from_slice(&output).expect("NDJSON sample");
        assert_eq!(sample.ec_us_cm, Some(450.0));
        assert_eq!(sample.source, SourceId::EzoEc);
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

    struct CommonSource(Option<String>);

    impl common::Source for CommonSource {
        type Error = FakeSourceError;

        fn read_frame(&mut self) -> Result<String, Self::Error> {
            self.0.take().ok_or(FakeSourceError)
        }
    }

    struct CommonSink {
        samples: Vec<TelemetrySample>,
    }

    impl common::Sink for CommonSink {
        type Error = FakeSinkError;

        fn write(&mut self, sample: TelemetrySample) -> Result<(), Self::Error> {
            self.samples.push(sample);
            Ok(())
        }
    }

    #[test]
    fn consumes_adapters_implemented_against_common_contracts() {
        let mut source = CommonSource(Some("?R,EC,450.00".to_owned()));
        let mut sink = CommonSink {
            samples: Vec::new(),
        };

        let report = poll_steps(&mut source, &mut sink, config(1)).expect("polling succeeds");

        assert_eq!(report.successful_samples, 1);
        assert_eq!(sink.samples[0].ec_us_cm, Some(450.0));
    }
}
