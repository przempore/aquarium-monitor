use collector::{
    Ds18b20Source, EzoEcSource, NdjsonSink, SerialTransport, Sink, Source, combine_samples,
    parse_ds18b20_frame, parse_ec_frame_for_tank,
};
use collector::{RawFrameLogger, StdinSource, collect_reader_with_raw_log};
use common::TelemetrySample;
use common::influxdb::{InfluxDbConfig, InfluxDbSink, StdHttpTransport};
use common::rules::{RulesConfig, RulesEngine, Severity, SpikeRule, ThresholdRule};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufWriter, Write};
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let config = Config::from_args(env::args().skip(1))?;
    let raw_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.raw_log)
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("open raw log {}: {error}", config.raw_log.display()),
            )
        })?;
    let rules = config.rules()?;
    let mut alarm_output = config.alarm_output.map(AlarmOutput::open).transpose()?;

    if let Some(device) = config.device {
        let transport = SerialTransport::open(&device).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("open serial device {}: {error}", device.display()),
            )
        })?;
        let mut source = EzoEcSource::new(transport);
        let mut temperature_source =
            Ds18b20Source::from_path(config.temperature_path.expect("validated temperature path"));
        let mut sink = match config.influx {
            Some(influx) => {
                let token = read_token_file(&influx.token_file)?;
                DeviceSink::Influx(
                    InfluxDbSink::new(
                        InfluxDbConfig::new(influx.url, influx.organization, influx.bucket, token),
                        StdHttpTransport,
                    )
                    .map_err(|error| {
                        io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
                    })?,
                )
            }
            None => DeviceSink::Ndjson(NdjsonSink::new(BufWriter::new(io::stdout()))),
        };
        let mut raw_logger = collector::RawFrameLogger::new(BufWriter::new(&raw_file));
        let mut runtime = RuleRuntime::new(rules, alarm_output.take());
        loop {
            let ec_frame = match source.read_frame() {
                Ok(frame) => frame,
                Err(error) => {
                    eprintln!("EZO-EC source failed: {error}");
                    std::thread::sleep(config.interval);
                    continue;
                }
            };
            raw_logger.write_frame(&ec_frame)?;
            let temperature_frame = match temperature_source.read_frame() {
                Ok(frame) => frame,
                Err(error) => {
                    eprintln!("DS18B20 source failed: {error}");
                    std::thread::sleep(config.interval);
                    continue;
                }
            };
            raw_logger.write_frame(&temperature_frame)?;
            let ec = match parse_ec_frame_for_tank(
                config.tank_id.as_deref().expect("validated tank id"),
                &ec_frame,
            ) {
                Ok(sample) => sample,
                Err(error) => {
                    eprintln!("EZO-EC parse failed: {error}");
                    std::thread::sleep(config.interval);
                    continue;
                }
            };
            let temperature = match parse_ds18b20_frame(&temperature_frame) {
                Ok(sample) => sample,
                Err(error) => {
                    eprintln!("DS18B20 parse failed: {error}");
                    std::thread::sleep(config.interval);
                    continue;
                }
            };
            let sample = combine_samples(ec, temperature);
            runtime.process(sample, &mut sink)?;
            sink.flush()?;
            std::thread::sleep(config.interval);
        }
    } else if rules.is_some() {
        collect_stdin_with_rules(
            io::stdin().lock(),
            BufWriter::new(io::stdout().lock()),
            raw_file,
            RuleRuntime::new(rules, alarm_output),
        )
        .map(|_| ())
    } else {
        collect_reader_with_raw_log(
            io::stdin().lock(),
            BufWriter::new(io::stdout().lock()),
            BufWriter::new(raw_file),
        )
        .map(|_| ())
    }
}

fn collect_stdin_with_rules<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    raw_file: std::fs::File,
    mut runtime: RuleRuntime,
) -> io::Result<usize> {
    let mut source = StdinSource::new(reader);
    let mut sink = NdjsonSink::new(writer);
    let mut raw_logger = RawFrameLogger::new(BufWriter::new(raw_file));
    let mut successful_samples = 0;
    while let Some(frame) = source.read_frame_or_eof()? {
        raw_logger.write_frame(&frame)?;
        match parse_ec_frame_for_tank("stdin-demo", &frame) {
            Ok(sample) => {
                runtime.process(sample, &mut sink)?;
                successful_samples += 1;
            }
            Err(error) => eprintln!("collector rejected raw frame {frame:?}: {error}"),
        }
    }
    sink.into_inner().flush()?;
    raw_logger.into_inner().flush()?;
    runtime.flush()?;
    Ok(successful_samples)
}

struct RuleRuntime {
    engine: Option<RulesEngine>,
    previous: Option<TelemetrySample>,
    alarm_output: Option<AlarmOutput>,
}

impl RuleRuntime {
    fn new(engine: Option<RulesEngine>, alarm_output: Option<AlarmOutput>) -> Self {
        Self {
            engine,
            previous: None,
            alarm_output,
        }
    }

    fn process<K: Sink>(&mut self, sample: TelemetrySample, sink: &mut K) -> io::Result<()> {
        if let Some(engine) = self.engine {
            let alarms = engine.evaluate(
                &sample,
                self.previous.as_ref(),
                time::OffsetDateTime::now_utc(),
            );
            if let Some(output) = &mut self.alarm_output {
                output.write_all(&alarms)?;
                output.flush()?;
            }
        }
        sink.write(sample.clone())
            .map_err(|error| io::Error::other(format!("collector sink failed: {error}")))?;
        self.previous = Some(sample);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(output) = &mut self.alarm_output {
            output.flush()?;
        }
        Ok(())
    }
}

enum AlarmOutput {
    File(BufWriter<std::fs::File>),
    Stderr(io::Stderr),
}

impl AlarmOutput {
    fn open(destination: String) -> io::Result<Self> {
        if destination == "stderr" {
            return Ok(Self::Stderr(io::stderr()));
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&destination)
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("open alarm output {destination}: {error}"),
                )
            })?;
        Ok(Self::File(BufWriter::new(file)))
    }

    fn write_all(&mut self, alarms: &[common::rules::AlarmEvent]) -> io::Result<()> {
        for alarm in alarms {
            let encoded = serde_json::to_vec(alarm)
                .map_err(|error| io::Error::other(format!("serialize alarm event: {error}")))?;
            match self {
                Self::File(writer) => writer
                    .write_all(&encoded)
                    .and_then(|_| writer.write_all(b"\n")),
                Self::Stderr(writer) => writer
                    .write_all(&encoded)
                    .and_then(|_| writer.write_all(b"\n")),
            }
            .map_err(|error| io::Error::new(error.kind(), format!("write alarm event: {error}")))?;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::File(writer) => writer.flush(),
            Self::Stderr(writer) => writer.flush(),
        }
    }
}

#[derive(Debug, PartialEq)]
struct Config {
    raw_log: PathBuf,
    tank_id: Option<String>,
    device: Option<PathBuf>,
    temperature_path: Option<PathBuf>,
    interval: std::time::Duration,
    influx: Option<InfluxConfig>,
    alarm_output: Option<String>,
    ec_min: Option<f32>,
    ec_max: Option<f32>,
    ec_severity: Option<Severity>,
    temperature_min: Option<f32>,
    temperature_max: Option<f32>,
    temperature_severity: Option<Severity>,
    max_age_seconds: Option<u64>,
    stale_severity: Option<Severity>,
    spike_absolute: Option<f32>,
    spike_relative: Option<f32>,
    spike_severity: Option<Severity>,
}

#[derive(Debug, PartialEq, Eq)]
struct InfluxConfig {
    url: String,
    organization: String,
    bucket: String,
    token_file: PathBuf,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut args = args.into_iter();
        let mut raw_log = None;
        let mut tank_id = None;
        let mut device = None;
        let mut interval = None;
        let mut temperature_path = None;
        let mut influx_url = None;
        let mut influx_organization = None;
        let mut influx_bucket = None;
        let mut influx_token_file = None;
        let mut alarm_output = None;
        let mut ec_min = None;
        let mut ec_max = None;
        let mut ec_severity = None;
        let mut temperature_min = None;
        let mut temperature_max = None;
        let mut temperature_severity = None;
        let mut max_age_seconds = None;
        let mut stale_severity = None;
        let mut spike_absolute = None;
        let mut spike_relative = None;
        let mut spike_severity = None;
        while let Some(argument) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| invalid_arguments(&format!("{argument} requires a value")))?;
            if value.is_empty() {
                return Err(invalid_arguments(&format!(
                    "{argument} requires a non-empty value"
                )));
            }
            match argument.as_str() {
                "--raw-log" => raw_log = Some(PathBuf::from(value)),
                "--tank-id" => tank_id = Some(value),
                "--device" => device = Some(PathBuf::from(value)),
                "--temperature-path" => temperature_path = Some(PathBuf::from(value)),
                "--influx-url" => influx_url = Some(value),
                "--influx-organization" => influx_organization = Some(value),
                "--influx-bucket" => influx_bucket = Some(value),
                "--influx-token-file" => influx_token_file = Some(PathBuf::from(value)),
                "--alarm-output" => alarm_output = Some(value),
                "--ec-min" => ec_min = Some(parse_float(&value, "--ec-min")?),
                "--ec-max" => ec_max = Some(parse_float(&value, "--ec-max")?),
                "--ec-severity" => ec_severity = Some(parse_severity(&value)?),
                "--temperature-min" => {
                    temperature_min = Some(parse_float(&value, "--temperature-min")?)
                }
                "--temperature-max" => {
                    temperature_max = Some(parse_float(&value, "--temperature-max")?)
                }
                "--temperature-severity" => temperature_severity = Some(parse_severity(&value)?),
                "--max-age-seconds" => {
                    max_age_seconds = Some(parse_positive(&value, "--max-age-seconds")?)
                }
                "--stale-severity" => stale_severity = Some(parse_severity(&value)?),
                "--spike-absolute" => {
                    spike_absolute = Some(parse_float(&value, "--spike-absolute")?)
                }
                "--spike-relative" => {
                    spike_relative = Some(parse_float(&value, "--spike-relative")?)
                }
                "--spike-severity" => spike_severity = Some(parse_severity(&value)?),
                "--interval-seconds" => {
                    let seconds = value.parse::<u64>().map_err(|_| {
                        invalid_arguments("--interval-seconds requires a positive integer")
                    })?;
                    if seconds == 0 {
                        return Err(invalid_arguments("--interval-seconds must be positive"));
                    }
                    interval = Some(seconds)
                }
                _ => {
                    return Err(invalid_arguments(&format!(
                        "unexpected argument {argument:?}"
                    )));
                }
            }
        }
        let raw_log =
            raw_log.ok_or_else(|| invalid_arguments("missing required option --raw-log PATH"))?;
        if device.is_some() && interval.is_none() {
            return Err(invalid_arguments("--device requires --interval-seconds"));
        }
        if device.is_some() && tank_id.is_none() {
            return Err(invalid_arguments("--device requires --tank-id ID"));
        }
        if device.is_some() != temperature_path.is_some() {
            return Err(invalid_arguments(
                "--device and --temperature-path must be provided together",
            ));
        }
        let influx_values = [
            influx_url.is_some(),
            influx_organization.is_some(),
            influx_bucket.is_some(),
            influx_token_file.is_some(),
        ];
        if influx_values.iter().any(|present| *present)
            && !influx_values.iter().all(|present| *present)
        {
            return Err(invalid_arguments(
                "--influx-url, --influx-organization, --influx-bucket, and --influx-token-file must be provided together",
            ));
        }
        if device.is_none() && influx_values.iter().any(|present| *present) {
            return Err(invalid_arguments(
                "InfluxDB options require --device hardware mode",
            ));
        }
        Ok(Self {
            raw_log,
            tank_id,
            device,
            temperature_path,
            interval: std::time::Duration::from_secs(interval.unwrap_or(0)),
            influx: influx_url.map(|url| InfluxConfig {
                url,
                organization: influx_organization.expect("validated organization"),
                bucket: influx_bucket.expect("validated bucket"),
                token_file: influx_token_file.expect("validated token file"),
            }),
            alarm_output,
            ec_min,
            ec_max,
            ec_severity,
            temperature_min,
            temperature_max,
            temperature_severity,
            max_age_seconds,
            stale_severity,
            spike_absolute,
            spike_relative,
            spike_severity,
        })
    }

    fn rules(&self) -> io::Result<Option<RulesEngine>> {
        let ec = threshold(self.ec_min, self.ec_max, self.ec_severity, "EC")?;
        let temperature = threshold(
            self.temperature_min,
            self.temperature_max,
            self.temperature_severity,
            "temperature",
        )?;
        let spike = match (
            self.spike_absolute,
            self.spike_relative,
            self.spike_severity,
        ) {
            (None, None, None) => None,
            (Some(absolute), relative, Some(severity)) => Some(SpikeRule {
                absolute: Some(absolute),
                relative,
                severity,
            }),
            (absolute, Some(relative), Some(severity)) => Some(SpikeRule {
                absolute,
                relative: Some(relative),
                severity,
            }),
            _ => return Err(invalid_arguments("spike limits require --spike-severity")),
        };
        let stale = match (self.max_age_seconds, self.stale_severity) {
            (None, None) => None,
            (Some(seconds), Some(severity)) => {
                Some((std::time::Duration::from_secs(seconds), severity))
            }
            _ => {
                return Err(invalid_arguments(
                    "--max-age-seconds requires --stale-severity",
                ));
            }
        };
        let active = ec.is_some() || temperature.is_some() || spike.is_some() || stale.is_some();
        if !active {
            return Ok(None);
        }
        if self.alarm_output.is_none() {
            return Err(invalid_arguments(
                "rule options require --alarm-output PATH|stderr",
            ));
        }
        Ok(Some(RulesEngine::new(RulesConfig {
            ec,
            temperature,
            max_age: stale.map(|value| value.0),
            stale_severity: stale.map_or(Severity::Warning, |value| value.1),
            spike,
        })))
    }
}

fn threshold(
    minimum: Option<f32>,
    maximum: Option<f32>,
    severity: Option<Severity>,
    name: &str,
) -> io::Result<Option<ThresholdRule>> {
    match (minimum, maximum, severity) {
        (None, None, None) => Ok(None),
        (minimum, maximum, Some(severity)) => Ok(Some(ThresholdRule {
            minimum,
            maximum,
            severity,
        })),
        _ => Err(invalid_arguments(&format!(
            "{name} limits require a severity option"
        ))),
    }
}

fn parse_float(value: &str, option: &str) -> io::Result<f32> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| invalid_arguments(&format!("{option} requires a finite number")))?;
    if parsed.is_finite() && parsed >= 0.0 {
        Ok(parsed)
    } else {
        Err(invalid_arguments(&format!(
            "{option} requires a finite non-negative number"
        )))
    }
}

fn parse_positive(value: &str, option: &str) -> io::Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| invalid_arguments(&format!("{option} requires a positive integer")))?;
    if parsed > 0 {
        Ok(parsed)
    } else {
        Err(invalid_arguments(&format!("{option} must be positive")))
    }
}

fn parse_severity(value: &str) -> io::Result<Severity> {
    match value {
        "warning" => Ok(Severity::Warning),
        "critical" => Ok(Severity::Critical),
        _ => Err(invalid_arguments("severity must be warning or critical")),
    }
}

fn read_token_file(path: &std::path::Path) -> io::Result<String> {
    let token = fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("read InfluxDB token file {}: {error}", path.display()),
        )
    })?;
    let token = token.strip_suffix('\n').unwrap_or(&token);
    let token = token.strip_suffix('\r').unwrap_or(token);
    if token.is_empty() {
        return Err(invalid_arguments("InfluxDB token file is empty"));
    }
    Ok(token.to_owned())
}

enum DeviceSink {
    Influx(InfluxDbSink<StdHttpTransport>),
    Ndjson(NdjsonSink<BufWriter<io::Stdout>>),
}

impl Sink for DeviceSink {
    type Error = io::Error;

    fn write(&mut self, sample: common::TelemetrySample) -> Result<(), Self::Error> {
        match self {
            Self::Influx(sink) => sink
                .write(sample)
                .map_err(|error| io::Error::other(error.to_string())),
            Self::Ndjson(sink) => sink.write(sample),
        }
    }
}

impl DeviceSink {
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Influx(_) => Ok(()),
            Self::Ndjson(sink) => sink.flush(),
        }
    }
}

fn invalid_arguments(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_raw_log_option() {
        assert_eq!(
            Config::from_args(["--raw-log".to_owned(), "frames.ndjson".to_owned()])
                .expect("valid arguments"),
            Config {
                raw_log: PathBuf::from("frames.ndjson"),
                tank_id: None,
                device: None,
                temperature_path: None,
                interval: std::time::Duration::ZERO,
                influx: None,
                alarm_output: None,
                ec_min: None,
                ec_max: None,
                ec_severity: None,
                temperature_min: None,
                temperature_max: None,
                temperature_severity: None,
                max_age_seconds: None,
                stale_severity: None,
                spike_absolute: None,
                spike_relative: None,
                spike_severity: None,
            }
        );
    }

    #[test]
    fn rejects_invalid_arguments() {
        for args in [
            Vec::new(),
            vec!["--raw-log".to_owned()],
            vec!["--other".to_owned()],
            vec!["--raw-log".to_owned(), "".to_owned()],
            vec![
                "--raw-log".to_owned(),
                "frames.ndjson".to_owned(),
                "extra".to_owned(),
            ],
            vec![
                "--raw-log".to_owned(),
                "frames.ndjson".to_owned(),
                "--device".to_owned(),
                "/dev/ttyUSB0".to_owned(),
            ],
            vec![
                "--raw-log".to_owned(),
                "frames.ndjson".to_owned(),
                "--device".to_owned(),
                "/dev/ttyUSB0".to_owned(),
                "--interval-seconds".to_owned(),
                "0".to_owned(),
            ],
            vec![
                "--raw-log".to_owned(),
                "frames.ndjson".to_owned(),
                "--device".to_owned(),
                "/dev/ttyUSB0".to_owned(),
                "--interval-seconds".to_owned(),
                "1".to_owned(),
            ],
        ] {
            assert!(Config::from_args(args).is_err());
        }
    }

    #[test]
    fn parses_complete_influx_device_configuration() {
        let config = Config::from_args(
            [
                "--raw-log",
                "frames.ndjson",
                "--device",
                "/dev/ttyUSB0",
                "--tank-id",
                "tank-1",
                "--temperature-path",
                "/sys/w1_slave",
                "--interval-seconds",
                "1",
                "--influx-url",
                "http://127.0.0.1:8086",
                "--influx-organization",
                "aquarium",
                "--influx-bucket",
                "telemetry",
                "--influx-token-file",
                "/run/keys/influx-token",
            ]
            .map(str::to_owned),
        )
        .expect("valid configuration");
        assert_eq!(
            config.influx,
            Some(InfluxConfig {
                url: "http://127.0.0.1:8086".into(),
                organization: "aquarium".into(),
                bucket: "telemetry".into(),
                token_file: "/run/keys/influx-token".into(),
            })
        );
        assert_eq!(config.tank_id.as_deref(), Some("tank-1"));
    }

    #[test]
    fn parses_explicit_rule_configuration() {
        let config = Config::from_args(
            [
                "--raw-log",
                "frames.ndjson",
                "--alarm-output",
                "stderr",
                "--ec-min",
                "100",
                "--ec-max",
                "500",
                "--ec-severity",
                "critical",
                "--max-age-seconds",
                "60",
                "--stale-severity",
                "warning",
                "--spike-relative",
                "0.5",
                "--spike-severity",
                "warning",
            ]
            .map(str::to_owned),
        )
        .expect("valid rule configuration");

        let engine = config.rules().expect("valid rules").expect("rules enabled");
        assert_eq!(
            engine.config().ec.expect("EC rule").severity,
            Severity::Critical
        );
        assert_eq!(
            engine.config().max_age,
            Some(std::time::Duration::from_secs(60))
        );
        assert!(engine.config().spike.is_some());
    }

    #[test]
    fn runtime_emits_alarm_and_keeps_rules_disabled_by_default() {
        let config = Config::from_args(["--raw-log", "frames.ndjson"].map(str::to_owned))
            .expect("valid default configuration");
        assert!(config.rules().expect("rules parse").is_none());

        let path = std::env::temp_dir().join(format!("aquarium-alarm-{}", std::process::id()));
        let mut runtime = RuleRuntime::new(
            Some(RulesEngine::new(RulesConfig {
                ec: Some(ThresholdRule {
                    minimum: Some(100.0),
                    maximum: None,
                    severity: Severity::Critical,
                }),
                temperature: None,
                max_age: None,
                stale_severity: Severity::Warning,
                spike: None,
            })),
            Some(AlarmOutput::open(path.to_string_lossy().into_owned()).expect("alarm file")),
        );
        let sample = TelemetrySample {
            tank_id: "tank-1".into(),
            timestamp: time::OffsetDateTime::from_unix_timestamp(1_000).expect("timestamp"),
            ec_us_cm: Some(50.0),
            temp_c: None,
            source: common::SourceId::Hardware,
            quality: common::Quality::Ok,
        };
        let mut sink = NdjsonSink::new(Vec::new());
        runtime.process(sample, &mut sink).expect("runtime process");
        runtime.flush().expect("alarm flush");
        let alarm = fs::read_to_string(&path).expect("read alarm");
        fs::remove_file(path).expect("remove alarm");
        let value: serde_json::Value = serde_json::from_str(alarm.trim()).expect("alarm JSON");
        assert_eq!(value["rule_id"], "ec.minimum");
        assert_eq!(value["severity"], "critical");
    }

    #[test]
    fn token_file_removes_only_final_newline_and_never_appears_in_error() {
        let path = std::env::temp_dir().join(format!("aquarium-token-{}", std::process::id()));
        fs::write(&path, "secret-token\n").expect("write token fixture");
        assert_eq!(read_token_file(&path).expect("read token"), "secret-token");
        fs::remove_file(&path).expect("remove token fixture");

        let error = read_token_file(&path).expect_err("missing token should fail");
        assert!(!error.to_string().contains("secret-token"));
    }
}
