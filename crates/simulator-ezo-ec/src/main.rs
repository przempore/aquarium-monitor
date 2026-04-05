use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[derive(Parser, Debug)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

const DEFAULT_INTERVAL_SECONDS: u32 = 1;

#[derive(Debug, Subcommand)]
#[command(version, about, long_about = None)]
enum Command {
    #[command(
        name = "information",
        visible_alias = "info",
        short_flag = 'i',
        long_flag = "information"
    )]
    Information,

    #[command(name = "continuous", short_flag = 'c', long_flag = "continuous")]
    Continuous {
        #[arg(value_name = "SECONDS")]
        interval_seconds: Option<u32>,
        #[arg(long, default_value_t = false, conflicts_with = "interval_seconds")]
        status: bool,
        // #[arg(long, default_value_t = DEFAULT_INTERVAL_SECONDS)]
        // interval_seconds: u32,
        // #[arg(long, default_value_t = false)]
        // status: bool,
    },
}

#[derive(Debug)]
struct Simulator {
    name: &'static str,
    version: &'static str,
    interval: u32,
}

impl Simulator {
    pub fn new() -> Self {
        Self {
            name: "EZO-EC",
            version: "2.16",
            interval: DEFAULT_INTERVAL_SECONDS,
        }
    }

    fn run(&mut self, command: Command) -> String {
        let result = match command {
            Command::Information => self.information(),
            Command::Continuous {
                interval_seconds,
                status,
            } => {
                if status {
                    format!("?C,{}\n\r", self.interval)
                } else {
                    eprintln!(
                        "=== WiP! This should start continuous measurement with that interval ==="
                    );
                    dbg!(interval_seconds);
                    self.interval = interval_seconds.unwrap_or(DEFAULT_INTERVAL_SECONDS);
                    format!("")
                }
            }
            _ => format!(""),
        };

        format!("{}*OK\n\r", result)
    }

    fn information(&self) -> String {
        format!("?i,{},{}\n\r", self.name, self.version)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:5555").await.context("failed to bind TCP listener on 127.0.0.1:5555")?;

    loop {
        let (socket, addr) = listener.accept().await.context("accept failed")?;
        println!("New client connected: {addr}");

        handle_client(socket).await.with_context(|| format!("client session failed: {addr}"))?;
    }
}

async fn handle_client(socket: TcpStream) -> Result<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);

    let mut simulator = Simulator::new();
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let cmd = line.trim_matches(|c| c == '\r' || c == '\n');
        let response = simulator.run_text_command(cmd);

        writer.write_all(response.as_bytes()).await?;
    }

    Ok(())
}

impl Simulator {
    fn run_text_command(&mut self, cmd: &str) -> String {
        match cmd {
            "i" | "info" => self.information_with_ok(),
            "C,?" => format!("?C,{}\n\r*OK\n\r", self.interval),
            _ if cmd.starts_with("C,") =>
            // TODO: parse interval and update self.interval
            // return "*OK\n\r" or error frame
            {
                "*OK\n\r".to_string()
            }
            _ => "*ER".to_string(),
        }
    }

    fn information_with_ok(&self) -> String {
        format!("{}*OK\n\r", self.information())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        Simulator::new();
    }

    #[test]
    fn get_information() {
        let mut simulator = Simulator::new();
        let information = simulator.run(Command::Information);
        assert_eq!(information, "?i,EZO-EC,2.16\n\r*OK\n\r");
    }

    #[test]
    fn set_interval() {
        let mut simulator = Simulator::new();
        let result = simulator.run(Command::Continuous {
            interval_seconds: None,
            status: true,
        });
        assert_eq!(result, "?C,1\n\r*OK\n\r");

        let result = simulator.run(Command::Continuous {
            interval_seconds: Some(42),
            status: false,
        });
        assert_eq!(result, "*OK\n\r");

        let result = simulator.run(Command::Continuous {
            interval_seconds: None,
            status: true,
        });
        assert_eq!(result, "?C,42\n\r*OK\n\r");
    }
}
