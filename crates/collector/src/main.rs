use collector::collect_reader_with_raw_log;
use collector::{
    Ds18b20Source, EzoEcSource, NdjsonSink, SerialTransport, Sink, Source, combine_samples,
    parse_ds18b20_frame, parse_ec_frame,
};
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
        let mut temperature_source =
            Ds18b20Source::from_path(config.temperature_path.expect("validated temperature path"));
        let mut sink = NdjsonSink::new(BufWriter::new(io::stdout().lock()));
        let mut raw_logger = collector::RawFrameLogger::new(BufWriter::new(&raw_file));
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
            let ec = match parse_ec_frame(&ec_frame) {
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
            sink.write(combine_samples(ec, temperature))
                .map_err(|error| io::Error::other(format!("collector sink failed: {error}")))?;
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
    temperature_path: Option<PathBuf>,
    interval: std::time::Duration,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut args = args.into_iter();
        let mut raw_log = None;
        let mut device = None;
        let mut interval = None;
        let mut temperature_path = None;
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
                "--temperature-path" => temperature_path = Some(PathBuf::from(value)),
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
        if device.is_some() != temperature_path.is_some() {
            return Err(invalid_arguments(
                "--device and --temperature-path must be provided together",
            ));
        }
        Ok(Self {
            raw_log,
            device,
            temperature_path,
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
                temperature_path: None,
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
}
