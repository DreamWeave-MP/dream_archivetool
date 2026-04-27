#![allow(clippy::missing_errors_doc)]

//! Library support for inspecting, extracting, creating, and updating Bethesda archives.
//!
//! `dream-archivetool` wraps the [`dream_archive`] crate behind a small, application-oriented API that is
//! shared by the CLI and optional Lua bindings. The main entry point is [`ArchiveTool`]; callers doing
//! repeated operations against one archive can use [`OpenArchive`] to keep the archive loaded.
//!
//! # Example
//!
//! ```no_run
//! use dream_archivetool::{ArchiveTool, CreateOptions};
//!
//! # fn main() -> dream_archivetool::Result<()> {
//! let archive = ArchiveTool::open("Morrowind.bsa")?;
//! let entries = archive.list()?;
//! let bytes = archive.read_entry("icons/gold.dds")?;
//! let count = ArchiveTool::create("out.bsa", "input_dir", &CreateOptions::default())?;
//! # Ok(())
//! # }
//! ```
//!
//! Extraction rejects absolute paths and parent-directory traversal. Disk extraction streams payloads
//! into temporary files before replacing the destination. Archive creation and update preflight
//! paths before reading payloads, but currently buffer archive entries in memory because the
//! backend builder APIs require owned bytes.
//!
//! Enable the `lua` feature to register a `dream_archivetool` table that mirrors the public
//! [`ArchiveTool`] API for embedded Lua callers.

pub mod archive;
pub mod create;
pub mod entry;
pub mod error;
pub mod extract;
pub mod format;
mod loaded;
mod paths;

#[cfg(feature = "lua")]
pub mod lua;

pub use archive::{ArchiveInfo, ArchiveTool, OpenArchive};
pub use create::{AddOptions, CreateOptions, Fo4ArchiveKind, Fo4Version, Tes4Version};
pub use entry::ArchiveEntry;
pub use error::{ArchiveError, Result};
pub use extract::{ExtractAllOptions, ExtractOptions, ExtractSummary, OverwriteMode};
pub use format::ArchiveFormat;
