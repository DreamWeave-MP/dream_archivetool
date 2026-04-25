//! Library support for inspecting, extracting, creating, and updating Bethesda archives.
//!
//! `rome-archivetool` wraps the [`ba2`] crate behind a small, application-oriented API that is
//! shared by the CLI and optional Lua bindings. The main entry point is [`ArchiveTool`].
//!
//! # Example
//!
//! ```no_run
//! use rome_archivetool::{ArchiveTool, CreateOptions};
//!
//! # fn main() -> rome_archivetool::Result<()> {
//! let entries = ArchiveTool::list("Morrowind.bsa")?;
//! let bytes = ArchiveTool::read_entry("Morrowind.bsa", "icons/gold.dds")?;
//! let count = ArchiveTool::create("out.bsa", "input_dir", &CreateOptions::default())?;
//! # Ok(())
//! # }
//! ```
//!
//! Extraction rejects absolute paths and parent-directory traversal. Archive creation and update
//! write through a temporary file before replacing the destination.

pub mod archive;
pub mod create;
pub mod entry;
pub mod error;
pub mod extract;
pub mod format;
mod loaded;

#[cfg(feature = "lua")]
pub mod lua;

pub use archive::{ArchiveInfo, ArchiveTool};
pub use create::{AddOptions, CreateOptions, Fo4ArchiveKind, Fo4Version, Tes4Version};
pub use entry::ArchiveEntry;
pub use error::{ArchiveError, Result};
pub use extract::{ExtractAllOptions, ExtractOptions, ExtractSummary, OverwriteMode};
pub use format::ArchiveFormat;
