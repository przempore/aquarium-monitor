mod core;
mod engine;

pub use core::{EzoEcCore, DEFAULT_INTERVAL_SECONDS};
pub use engine::{CommandRequest, SimulatorHandle, spawn_simulator, spawn_simulator_with_capacity};

pub fn normalize_command(cmd: &str) -> String {
    cmd.trim().trim_end_matches(|c| c == '\r' || c == '\n').to_string()
}
