pub const DEFAULT_INTERVAL_SECONDS: u32 = 1;

#[derive(Debug)]
pub struct EzoEcCore {
    interval_seconds: u32,
    sample_index: u64,
}

impl Default for EzoEcCore {
    fn default() -> Self {
        Self::new()
    }
}

impl EzoEcCore {
    pub fn new() -> Self {
        Self {
            interval_seconds: DEFAULT_INTERVAL_SECONDS,
            sample_index: 0,
        }
    }

    pub fn handle_command(&mut self, command: &str) -> String {
        let command = normalize_command(command);
        match command {
            "i" | "info" => "?i,EZO-EC,2.16\n\r*OK\n\r".to_string(),
            "C,?" => format!("?C,{}\n\r*OK\n\r", self.interval_seconds),
            "R" => format!("{}*OK\n\r", self.periodic_frame()),
            command if command.starts_with("C,") => self.set_interval(command),
            _ => "*ER\r".to_string(),
        }
    }

    pub fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    pub fn periodic_frame(&mut self) -> String {
        let conductivity = 450.0 + (self.sample_index % 20) as f32 * 2.5;
        self.sample_index = self.sample_index.wrapping_add(1);
        format!("?R,EC,{conductivity:.2}\n\r")
    }

    fn set_interval(&mut self, command: &str) -> String {
        match command
            .strip_prefix("C,")
            .and_then(|value| value.parse::<u32>().ok())
        {
            Some(seconds) => {
                self.interval_seconds = seconds;
                "*OK\n\r".to_string()
            }
            None => "*ER\r".to_string(),
        }
    }
}

pub fn normalize_command(command: &str) -> &str {
    command.trim_end_matches(['\r', '\n'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_deterministic_and_stateful() {
        let mut core = EzoEcCore::new();
        assert_eq!(core.handle_command("i"), "?i,EZO-EC,2.16\n\r*OK\n\r");
        assert_eq!(core.handle_command("C,42\n"), "*OK\n\r");
        assert_eq!(core.handle_command("C,?"), "?C,42\n\r*OK\n\r");
        assert_eq!(core.handle_command("R"), "?R,EC,450.00\n\r*OK\n\r");
    }

    #[test]
    fn invalid_commands_return_error() {
        let mut core = EzoEcCore::new();
        for command in ["bad", "C,", "C,nope"] {
            assert_eq!(core.handle_command(command), "*ER\r");
        }
    }

    #[test]
    fn readings_are_deterministic() {
        let mut core = EzoEcCore::new();
        assert_eq!(core.periodic_frame(), "?R,EC,450.00\n\r");
        assert_eq!(core.periodic_frame(), "?R,EC,452.50\n\r");
    }
}
