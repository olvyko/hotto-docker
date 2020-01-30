use async_trait::async_trait;
use futures_util::pin_mut;
use std::process::Stdio;
use tokio::{
    io::{self, AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, Command},
    stream::{self, Stream, StreamExt},
};

/// Defines error cases when waiting for a message in a stream.
#[derive(Debug)]
pub enum WaitError {
    EndOfStream,
    IO(io::Error),
}

impl From<io::Error> for WaitError {
    fn from(e: io::Error) -> Self {
        WaitError::IO(e)
    }
}

pub struct WaitForMessage;

impl WaitForMessage {
    pub async fn wait_for_message<S: Stream<Item = String>>(stream: S, message: &str) -> Result<(), WaitError> {
        pin_mut!(stream);
        let mut number_of_compared_lines = 0;
        while let Some(line) = stream.next().await {
            number_of_compared_lines += 1;
            if line.contains(message) {
                log::info!("Found message after comparing {} lines", number_of_compared_lines);
                return Ok(());
            }
        }
        log::error!(
            "Failed to find message in stream after comparing {} lines.",
            number_of_compared_lines
        );
        Err(WaitError::EndOfStream)
    }
}
