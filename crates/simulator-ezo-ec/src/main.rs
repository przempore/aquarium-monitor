use clap::{Parser, Subcommand};

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

    #[command(
        name = "continuous",
        short_flag = 'c',
        long_flag = "continuous"
    )]
    Continuous {        
        #[arg(long, default_value_t = DEFAULT_INTERVAL_SECONDS)]
        interval_seconds: u32,
        #[arg(long, default_value_t = false)]
        status: bool,
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
            Command::Continuous { interval_seconds, status } => {
                if status {
                    format!("?C,{}\n\r", self.interval)
                } else {
                    eprintln!("=== WiP! This should start continuous measurement with that interval ===");
                    dbg!(interval_seconds);
                    self.interval = interval_seconds;
                    format!("")
                }
            },
            _ => format!(""),
        };

        format!("{}*OK\n\r", result)
    }

    fn information(&self) -> String {
        format!("?i,{},{}\n\r", self.name, self.version)
    }
}

fn main() {
    let args = Args::parse();
    let mut simulator = Simulator::new();

    print!("{}", simulator.run(args.cmd))
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
        let result = simulator.run(Command::Continuous { interval_seconds: 1, status: true });
        assert_eq!(result, "?C,1\n\r*OK\n\r");

        let result = simulator.run(Command::Continuous { interval_seconds: 42, status: false });
        assert_eq!(result, "*OK\n\r");

        let result = simulator.run(Command::Continuous { interval_seconds: 1, status: true });
        assert_eq!(result, "?C,42\n\r*OK\n\r");
    }
}
