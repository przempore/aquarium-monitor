pub const DEFAULT_INTERVAL_SECONDS: u32 = 1;

#[derive(Debug)]
pub struct EzoEcCore {
    name: &'static str,
    version: &'static str,
    interval_seconds: u32,
    sample_index: u64,
}

impl EzoEcCore {
    pub fn new() -> Self {
        Self {
            name: "EZO-EC",
            version: "2.16",
            interval_seconds: DEFAULT_INTERVAL_SECONDS,
            sample_index: 0,
        }
    }

    pub fn handle_command(&mut self, cmd: &str) -> String {
        match cmd {
            "i" | "info" => self.information_with_ok(),
            "C,?" => format!("?C,{}\n\r*OK\n\r", self.interval_seconds),
            _ if cmd.starts_with("C,") => self.set_interval(cmd),
            _ => "*ER\r".to_string(),
        }
    }

    pub fn run(&mut self, cmd: &str) -> String {
        self.handle_command(cmd)
    }

    pub fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    pub fn periodic_frame(&mut self) -> String {
        let conductivity = 450.0 + (self.sample_index % 20) as f32 * 2.5;
        self.sample_index = self.sample_index.wrapping_add(1);

        format!("?R,EC,{conductivity:.2}\n\r")
    }

    fn set_interval(&mut self, cmd: &str) -> String {
        let Some(raw_interval) = cmd.strip_prefix("C,") else {
            return "*ER\r".to_string();
        };

        let Ok(interval_seconds) = raw_interval.parse::<u32>() else {
            return "*ER\r".to_string();
        };

        self.interval_seconds = interval_seconds;
        "*OK\n\r".to_string()
    }

    fn information_with_ok(&self) -> String {
        format!("?i,{},{}\n\r*OK\n\r", self.name, self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::EzoEcCore;

    #[test]
    fn command_i_returns_information() {
        let mut core = EzoEcCore::new();
        let response = core.handle_command("i");

        assert_eq!(response, "?i,EZO-EC,2.16\n\r*OK\n\r");
    }

    #[test]
    fn command_status_returns_interval() {
        let mut core = EzoEcCore::new();
        let response = core.handle_command("C,?");

        assert_eq!(response, "?C,1\n\r*OK\n\r");
    }

    #[test]
    fn command_set_interval_updates_interval() {
        let mut core = EzoEcCore::new();

        let set_response = core.handle_command("C,42");
        let status_response = core.handle_command("C,?");

        assert_eq!(set_response, "*OK\n\r");
        assert_eq!(status_response, "?C,42\n\r*OK\n\r");
    }

    #[test]
    fn invalid_command_returns_error() {
        let mut core = EzoEcCore::new();
        let response = core.handle_command("bad");

        assert_eq!(response, "*ER\r");
    }

    #[test]
    fn periodic_frame_is_deterministic() {
        let mut core = EzoEcCore::new();

        let first = core.periodic_frame();
        let second = core.periodic_frame();

        assert_eq!(first, "?R,EC,450.00\n\r");
        assert_eq!(second, "?R,EC,452.50\n\r");
    }
}
