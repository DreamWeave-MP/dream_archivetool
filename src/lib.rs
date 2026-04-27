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
//! paths before handing deferred file/archive-entry sources to `dream_archive` builders.
//!
//! Enable the `lua` feature to register a `dream_archivetool` table that mirrors the public
//! [`ArchiveTool`] API for embedded Lua callers.

pub mod archive;
mod archive_plan;
pub mod create;
pub mod diff;
pub mod entry;
pub mod error;
pub mod extract;
pub mod format;
mod loaded;
mod paths;
mod rewrite_policy;
pub mod verify;

#[cfg(feature = "lua")]
pub mod lua;

pub use archive::{ArchiveInfo, ArchiveTool, Fo4Info, OpenArchive, Tes4Info};
pub use create::{
    AddOptions, AddPlan, ArchivePlanAction, ArchivePlanEntry, ArchivePlanOperation, CreateOptions,
    CreatePlan, Fo4ArchiveKind, Fo4Version, Tes4Version,
};
pub use diff::{DiffChange, DiffEntry, DiffEntryState, DiffOptions, DiffReport, diff_archives};
pub use entry::ArchiveEntry;
pub use error::{ArchiveError, Result};
pub use extract::{
    ExtractAllOptions, ExtractAllPlan, ExtractOptions, ExtractPlanAction, ExtractPlanEntry,
    ExtractSummary, OverwriteMode,
};
pub use format::ArchiveFormat;
pub use verify::{VerifyOptions, VerifyPathIssue, VerifyReport, verify_archive};
