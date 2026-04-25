use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArchiveError>;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("unsupported or unrecognized archive format")]
    UnknownFormat,
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("archive error: {0}")]
    Archive(String),
}
