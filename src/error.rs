//! TODO: document this.

use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unknown event kind: '{0}'")]
    UnknownEventKind(String),

    #[error("invalid memory width: {0}")]
    InvalidMemWidth(u8),

    #[error("invalid info index: {0}")]
    InvalidInfoIndex(u8),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("invalid number: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),

    #[error("unknown source id: {0}")]
    UnknownSource(u8),

    #[error("Invalid file name")]
    InvalidFileName,
    #[error("Invalid file extension: {0}")]
    InvalidFileExtension(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn is_eof(&self) -> bool {
        matches!(self, Error::Io(e) if e.kind() == std::io::ErrorKind::UnexpectedEof
                                    || e.kind() == std::io::ErrorKind::Interrupted)
    }
}
