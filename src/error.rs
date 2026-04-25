use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ArchiveError>;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("unsupported or unrecognized archive format")]
    UnknownFormat,
    #[error("archive entry not found: {0}")]
    EntryNotFound(String),
    #[error("unsafe archive path: {0}")]
    UnsafePath(String),
    #[error("target already exists: {0}")]
    TargetExists(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("archive error: {0}")]
    Archive(String),
}
