use std::{fmt, io};
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Io(io::Error),
    Eof,
    TrailingBytes { context: &'static str, count: usize },
    Invalid(&'static str),
    Limit(&'static str),
    Unsupported(&'static str),
    State(&'static str),
    Disconnect(String),
    Auth(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Eof => f.write_str("incomplete input"),
            Self::TrailingBytes { context, count } => {
                write!(f, "{count} trailing bytes in {context}")
            }
            Self::Invalid(s) => write!(f, "invalid wire data: {s}"),
            Self::Limit(s) => write!(f, "resource limit: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported: {s}"),
            Self::State(s) => write!(f, "invalid state: {s}"),
            Self::Disconnect(s) => write!(f, "server disconnected: {s}"),
            Self::Auth(s) => write!(f, "authentication: {s}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
