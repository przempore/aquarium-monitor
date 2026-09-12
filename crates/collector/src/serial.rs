use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::Path;

/// Minimal synchronous serial transport. The line is configured by systemd's
/// `stty` ExecStartPre; this wrapper intentionally has no serial dependency.
pub struct SerialTransport {
    file: File,
    buffer: Vec<u8>,
    position: usize,
}

impl SerialTransport {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            file: OpenOptions::new().read(true).write(true).open(path)?,
            buffer: Vec::new(),
            position: 0,
        })
    }
}

impl Read for SerialTransport {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.position < self.buffer.len() {
            let available = &self.buffer[self.position..];
            let count = available.len().min(output.len());
            output[..count].copy_from_slice(&available[..count]);
            self.position += count;
            return Ok(count);
        }
        self.buffer.clear();
        self.position = 0;
        self.file.read(output)
    }
}

impl BufRead for SerialTransport {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.position == self.buffer.len() {
            self.buffer.resize(256, 0);
            let count = self.file.read(&mut self.buffer)?;
            self.buffer.truncate(count);
            self.position = 0;
        }
        Ok(&self.buffer[self.position..])
    }

    fn consume(&mut self, amount: usize) {
        self.position = (self.position + amount).min(self.buffer.len());
    }
}

impl Write for SerialTransport {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
