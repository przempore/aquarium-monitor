mod core;

pub use core::{EzoEcCore, DEFAULT_INTERVAL_SECONDS};

pub fn normalize_command(cmd: &str) -> String {
    cmd.trim().trim_end_matches(|c| c == '\r' || c == '\n').to_string()
}
