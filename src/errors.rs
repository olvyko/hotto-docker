use std::fmt::{self, Display};
use tokio::io;

/// Defines error cases when waiting for a message in a stream.
#[derive(Debug)]
pub enum WaitError {
    EndOfStream,
    WaitDurationExpired,
    Io(io::Error),
}

impl From<io::Error> for WaitError {
    fn from(e: io::Error) -> Self {
        WaitError::Io(e)
    }
}

impl Display for WaitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WaitError::EndOfStream => f.write_fmt(format_args!("dockerust > end of stream error")),
            WaitError::WaitDurationExpired => f.write_fmt(format_args!("dockerust > wait duration expired")),
            WaitError::Io(err) => f.write_fmt(format_args!("dockerust > tokio-io error: {}", err)),
        }
    }
}
