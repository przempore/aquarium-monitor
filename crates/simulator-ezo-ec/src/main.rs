use anyhow::{Context, Result};
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

mod core;
use crate::core::EzoEcCore;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:5555")
        .await
        .context("failed to bind TCP listener on 127.0.0.1:5555")?;

    loop {
        let (socket, addr) = listener.accept().await.context("accept failed")?;
        println!("New client connected: {addr}");

        handle_client(socket)
            .await
            .with_context(|| format!("client session failed: {addr}"))?;
    }
}

async fn handle_client(socket: TcpStream) -> Result<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);

    let mut core = EzoEcCore::new();
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let cmd = line.trim_matches(|c| c == '\r' || c == '\n');
        let response = core.handle_command(cmd);

        writer.write_all(response.as_bytes()).await?;
    }

    Ok(())
}
