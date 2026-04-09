use anyhow::{Context, Result};
use simulator_ezo_ec::{CommandRequest, normalize_command, spawn_simulator};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

#[tokio::main]
async fn main() -> Result<()> {
    let mut stdin = BufReader::new(io::stdin());
    let mut stdout = io::stdout();
    let mut simulator = spawn_simulator();
    let mut line = String::new();

    loop {
        tokio::select! {
            read_result = stdin.read_line(&mut line) => {
                let read_count = read_result.context("failed to read from stdin")?;
                if read_count == 0 {
                    break;
                }

                let command = normalize_command(&line);
                line.clear();

                if command.is_empty() {
                    continue;
                }

                let (response_tx, response_rx) = oneshot::channel();
                simulator
                    .command_tx
                    .send(CommandRequest {
                        command,
                        response_tx,
                    })
                    .await
                    .context("simulator command channel closed")?;

                let response = response_rx
                    .await
                    .context("simulator did not return command response")?;

                stdout
                    .write_all(response.as_bytes())
                    .await
                    .context("failed to write command response to stdout")?;
                stdout.flush().await.context("failed to flush stdout")?;
            }
            Some(frame) = simulator.output_rx.recv() => {
                stdout
                    .write_all(frame.as_bytes())
                    .await
                    .context("failed to write periodic frame to stdout")?;
                stdout.flush().await.context("failed to flush stdout")?;
            }
            else => {
                break;
            }
        }
    }

    Ok(())
}
