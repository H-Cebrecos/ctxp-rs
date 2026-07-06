use thiserror::Error;

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
}

pub type Result<T> = std::result::Result<T, Error>;
