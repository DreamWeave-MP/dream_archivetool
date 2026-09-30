// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io;

use thiserror::Error;

/// Result type used by `dream_archivetool` APIs.
pub type Result<T> = std::result::Result<T, ArchiveError>;

/// Error type returned by archive operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArchiveError {
    /// The file header does not match a supported archive format.
    #[error("unsupported or unrecognized archive format")]
    UnknownFormat,
    /// The requested archive entry was not present.
    #[error("archive entry not found: {0}")]
    EntryNotFound(String),
    /// An archive or filesystem path was rejected as unsafe.
    #[error("unsafe archive path: {0}")]
    UnsafePath(String),
    /// An input to `create` or `add` is a symbolic link, and `follow_symlinks` is off.
    #[error("refusing to follow symlink input path: {0}; set follow_symlinks to opt in")]
    SymlinkInput(String),
    /// Extraction would overwrite an existing target while overwrite mode is `Fail`.
    #[error("target already exists: {0}")]
    TargetExists(String),
    /// Filesystem I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Error reported by the underlying archive reader/writer or serialization layer.
    #[error("archive error: {0}")]
    Archive(String),
}
