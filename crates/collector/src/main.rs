use collector::collect_reader_with_raw_log;
use collector::{EzoEcSource, NdjsonSink, PollingConfig, SerialTransport, poll_with_raw_log};
use std::env;
use std::fs::OpenOptions;
use std::io::{self, BufWriter};
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

    if let Some(device) = config.device {
        let transport = SerialTransport::open(&device).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("open serial device {}: {error}", device.display()),
            )
        })?;
        let mut source = EzoEcSource::new(transport);
        let mut sink = NdjsonSink::new(BufWriter::new(io::stdout().lock()));
        loop {
            let report = poll_with_raw_log(
                &mut source,
                &mut sink,
                BufWriter::new(&raw_file),
                PollingConfig {
                    attempts: 1,
                    interval: config.interval,
                },
                std::thread::sleep,
            )
            .map_err(|error| io::Error::other(format!("collector stopped: {error}")))?;
            for failure in report.failures {
                eprintln!("collector {failure}");
            }
            sink.flush()?;
            std::thread::sleep(config.interval);
        }
    } else {
        collect_reader_with_raw_log(
            io::stdin().lock(),
            BufWriter::new(io::stdout().lock()),
            BufWriter::new(raw_file),
        )
        .map(|_| ())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Config {
    raw_log: PathBuf,
    device: Option<PathBuf>,
    interval: std::time::Duration,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut args = args.into_iter();
        let mut raw_log = None;
        let mut device = None;
        let mut interval = None;
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
                "--device" => device = Some(PathBuf::from(value)),
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
        Ok(Self {
            raw_log,
            device,
            interval: std::time::Duration::from_secs(interval.unwrap_or(0)),
        })
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
                device: None,
                interval: std::time::Duration::ZERO,
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
        ] {
            assert!(Config::from_args(args).is_err());
        }
    }
}
