use common::{Source, TemperatureSample};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Ds18b20Error {
    Io(io::Error),
    MissingCrcMarker,
    CrcFailure,
    MissingTemperature,
    MalformedTemperature,
    NonFiniteTemperature,
}

impl fmt::Display for Ds18b20Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "DS18B20 I/O failed: {error}"),
            Self::MissingCrcMarker => f.write_str("DS18B20 response has no CRC marker"),
            Self::CrcFailure => f.write_str("DS18B20 CRC validation failed"),
            Self::MissingTemperature => f.write_str("DS18B20 response has no temperature value"),
            Self::MalformedTemperature => f.write_str("DS18B20 temperature value is malformed"),
            Self::NonFiniteTemperature => f.write_str("DS18B20 temperature value is non-finite"),
        }
    }
}

impl Error for Ds18b20Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Synchronously reads a Linux w1 `w1_slave` file.
pub struct Ds18b20Source<R> {
    reader: R,
}

impl<R> Ds18b20Source<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

impl Ds18b20Source<Box<dyn FnMut() -> io::Result<String>>> {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path: PathBuf = path.as_ref().to_owned();
        Self::new(Box::new(move || std::fs::read_to_string(&path)))
    }
}

impl<R: FnMut() -> io::Result<String>> Source for Ds18b20Source<R> {
    type Error = Ds18b20Error;

    fn read_frame(&mut self) -> Result<String, Self::Error> {
        (self.reader)().map_err(Ds18b20Error::Io)
    }
}

pub fn parse_ds18b20_frame(frame: &str) -> Result<TemperatureSample, Ds18b20Error> {
    let crc = frame.lines().next().ok_or(Ds18b20Error::MissingCrcMarker)?;
    if !crc.contains(": crc=") {
        return Err(Ds18b20Error::MissingCrcMarker);
    }
    let marker = crc
        .split_whitespace()
        .last()
        .ok_or(Ds18b20Error::MissingCrcMarker)?;
    if marker != "YES" {
        if marker == "NO" {
            return Err(Ds18b20Error::CrcFailure);
        }
        return Err(Ds18b20Error::MissingCrcMarker);
    }

    let value = frame
        .lines()
        .nth(1)
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|field| field.strip_prefix("t="))
        })
        .ok_or(Ds18b20Error::MissingTemperature)?;
    let millidegrees = value
        .parse::<f32>()
        .map_err(|_| Ds18b20Error::MalformedTemperature)?;
    if !millidegrees.is_finite() {
        return Err(Ds18b20Error::NonFiniteTemperature);
    }
    Ok(TemperatureSample {
        temp_c: millidegrees / 1000.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    const VALID: &str =
        "9e 01 4b 46 7f ff 02 10 10 : crc=10 YES\n9e 01 4b 46 7f ff 02 10 10 t=26187\n";

    #[test]
    fn parses_valid_fixture() {
        assert_eq!(parse_ds18b20_frame(VALID).unwrap().temp_c, 26.187);
    }

    #[test]
    fn parses_negative_temperature() {
        let sample = parse_ds18b20_frame(&VALID.replace("26187", "-550")).unwrap();
        assert_eq!(sample.temp_c, -0.55);
    }

    #[test]
    fn rejects_bad_crc() {
        assert!(matches!(
            parse_ds18b20_frame(&VALID.replace("YES", "NO")),
            Err(Ds18b20Error::CrcFailure)
        ));
    }

    #[test]
    fn rejects_missing_and_malformed_values() {
        assert!(matches!(
            parse_ds18b20_frame(": crc=00 YES\nfoo\n"),
            Err(Ds18b20Error::MissingTemperature)
        ));
        assert!(matches!(
            parse_ds18b20_frame(&VALID.replace("26187", "oops")),
            Err(Ds18b20Error::MalformedTemperature)
        ));
        assert!(matches!(
            parse_ds18b20_frame(&VALID.replace("26187", "NaN")),
            Err(Ds18b20Error::NonFiniteTemperature)
        ));
    }

    #[test]
    fn source_reports_io_errors() {
        let mut source = Ds18b20Source::new(|| Err(io::Error::new(ErrorKind::NotFound, "missing")));
        assert!(
            matches!(source.read_frame(), Err(Ds18b20Error::Io(error)) if error.kind() == ErrorKind::NotFound)
        );
    }
}
