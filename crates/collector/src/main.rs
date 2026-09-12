use collector::collect_reader_with_raw_log;
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

    collect_reader_with_raw_log(
        io::stdin().lock(),
        BufWriter::new(io::stdout().lock()),
        BufWriter::new(raw_file),
    )
    .map(|_| ())
}

#[derive(Debug, PartialEq, Eq)]
struct Config {
    raw_log: PathBuf,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut args = args.into_iter();
        match (args.next().as_deref(), args.next(), args.next()) {
            (Some("--raw-log"), Some(path), None) if !path.is_empty() => Ok(Self {
                raw_log: PathBuf::from(path),
            }),
            (Some("--raw-log"), None, _) => Err(invalid_arguments("--raw-log requires a path")),
            (Some("--raw-log"), Some(path), None) if path.is_empty() => {
                Err(invalid_arguments("--raw-log requires a non-empty path"))
            }
            (Some("--raw-log"), Some(_), Some(_)) => {
                Err(invalid_arguments("unexpected extra argument"))
            }
            (Some(argument), _, _) => Err(invalid_arguments(&format!(
                "unexpected argument {argument:?}; expected --raw-log PATH"
            ))),
            (None, _, _) => Err(invalid_arguments("missing required option --raw-log PATH")),
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
        ] {
            assert!(Config::from_args(args).is_err());
        }
    }
}
