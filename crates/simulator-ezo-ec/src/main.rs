use simulator_ezo_ec::{DEFAULT_INTERVAL_SECONDS, EzoEcCore, normalize_command};
use std::io::{self, BufRead, Write};
use std::time::Duration;

fn main() -> io::Result<()> {
    let config = Config::from_args(std::env::args().skip(1))?;
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut core = EzoEcCore::new();

    if config.continuous {
        loop {
            emit_frame(&mut core, &mut stdout)?;
            std::thread::sleep(config.interval);
        }
    }

    for line in stdin.lock().lines() {
        let line = line?;
        let command = normalize_command(&line);
        if command.is_empty() {
            continue;
        }
        stdout.write_all(core.handle_command(command).as_bytes())?;
        stdout.flush()?;
    }
    Ok(())
}

fn emit_frame<W: Write>(core: &mut EzoEcCore, writer: &mut W) -> io::Result<()> {
    writer.write_all(core.periodic_frame().as_bytes())?;
    writer.flush()
}

#[derive(Debug, PartialEq, Eq)]
struct Config {
    continuous: bool,
    interval: Duration,
}

impl Config {
    fn from_args(args: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut args = args.into_iter();
        let mut continuous = false;
        let mut interval_seconds = DEFAULT_INTERVAL_SECONDS as u64;
        let mut interval_was_set = false;

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--continuous" if !continuous => continuous = true,
                "--continuous" => {
                    return Err(invalid_arguments("--continuous may be used only once"));
                }
                "--interval-seconds" if !interval_was_set => {
                    let value = args.next().ok_or_else(|| {
                        invalid_arguments("--interval-seconds requires a positive integer")
                    })?;
                    interval_seconds = value.parse().map_err(|_| {
                        invalid_arguments("--interval-seconds requires a positive integer")
                    })?;
                    if interval_seconds == 0 {
                        return Err(invalid_arguments(
                            "--interval-seconds must be greater than zero",
                        ));
                    }
                    interval_was_set = true;
                }
                "--interval-seconds" => {
                    return Err(invalid_arguments(
                        "--interval-seconds may be used only once",
                    ));
                }
                _ => {
                    return Err(invalid_arguments(&format!(
                        "unexpected argument {argument:?}; expected --continuous [--interval-seconds N]"
                    )));
                }
            }
        }

        if interval_was_set && !continuous {
            return Err(invalid_arguments(
                "--interval-seconds requires --continuous",
            ));
        }

        Ok(Self {
            continuous,
            interval: Duration::from_secs(interval_seconds),
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
    fn defaults_to_stdin_command_mode() {
        assert_eq!(
            Config::from_args(Vec::<String>::new()).expect("default configuration"),
            Config {
                continuous: false,
                interval: Duration::from_secs(1),
            }
        );
    }

    #[test]
    fn accepts_continuous_mode_and_interval() {
        assert_eq!(
            Config::from_args([
                "--continuous".to_owned(),
                "--interval-seconds".to_owned(),
                "5".to_owned(),
            ])
            .expect("continuous configuration"),
            Config {
                continuous: true,
                interval: Duration::from_secs(5),
            }
        );
    }

    #[test]
    fn rejects_invalid_continuous_arguments() {
        for args in [
            vec!["--interval-seconds".to_owned(), "2".to_owned()],
            vec!["--continuous".to_owned(), "--interval-seconds".to_owned()],
            vec![
                "--continuous".to_owned(),
                "--interval-seconds".to_owned(),
                "0".to_owned(),
            ],
            vec![
                "--continuous".to_owned(),
                "--interval-seconds".to_owned(),
                "nope".to_owned(),
            ],
            vec!["--other".to_owned()],
        ] {
            assert!(Config::from_args(args).is_err());
        }
    }

    #[test]
    fn one_emission_is_deterministic_and_flushed() {
        let mut core = EzoEcCore::new();
        let mut output = Vec::new();
        emit_frame(&mut core, &mut output).expect("emit frame");
        assert_eq!(output, b"?R,EC,450.00\n\r");
    }
}
