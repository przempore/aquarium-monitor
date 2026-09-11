use simulator_ezo_ec::{EzoEcCore, normalize_command};
use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut core = EzoEcCore::new();

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
