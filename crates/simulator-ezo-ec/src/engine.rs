use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, Duration, Instant, Interval, MissedTickBehavior};

use crate::core::EzoEcCore;

const DEFAULT_CHANNEL_CAPACITY: usize = 32;

#[derive(Debug)]
pub struct CommandRequest {
    pub command: String,
    pub response_tx: oneshot::Sender<String>,
}

#[derive(Debug)]
pub struct SimulatorHandle {
    pub command_tx: mpsc::Sender<CommandRequest>,
    pub output_rx: mpsc::Receiver<String>,
}

pub fn spawn_simulator() -> SimulatorHandle {
    spawn_simulator_with_capacity(DEFAULT_CHANNEL_CAPACITY)
}

pub fn spawn_simulator_with_capacity(channel_capacity: usize) -> SimulatorHandle {
    let (command_tx, mut command_rx) = mpsc::channel::<CommandRequest>(channel_capacity);
    let (output_tx, output_rx) = mpsc::channel::<String>(channel_capacity);

    tokio::spawn(async move {
        let mut core = EzoEcCore::new();
        let mut interval_seconds = core.interval_seconds();
        let mut ticker = build_interval(interval_seconds.max(1));

        loop {
            tokio::select! {
                Some(request) = command_rx.recv() => {
                    let response = core.handle_command(&request.command);
                    let _ = request.response_tx.send(response);

                    let next_interval = core.interval_seconds();
                    if next_interval != interval_seconds {
                        interval_seconds = next_interval;
                        ticker = build_interval(interval_seconds.max(1));
                    }
                }
                _ = ticker.tick(), if interval_seconds > 0 => {
                    let frame = core.periodic_frame();
                    if output_tx.send(frame).await.is_err() {
                        break;
                    }
                }
                else => {
                    break;
                }
            }
        }
    });

    SimulatorHandle {
        command_tx,
        output_rx,
    }
}

fn build_interval(interval_seconds: u32) -> Interval {
    let period = Duration::from_secs(interval_seconds as u64);
    let mut interval = time::interval_at(Instant::now() + period, period);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
}
