#![allow(clippy::missing_errors_doc)]

//! Library support for inspecting, extracting, creating, and updating Bethesda archives.
//!
//! `dream-archivetool` wraps the [`dream_archive`] crate behind a small, application-oriented API that is
//! shared by the CLI and optional Lua bindings. The main entry point is [`ArchiveTool`]; callers doing
//! repeated operations against one archive can use [`OpenArchive`] to keep the archive loaded.
//! `dream_archive` is re-exported so embedding applications can register the matching lower-level Lua
//! API without adding a second dependency just to spell the same crate name. That re-export is part
//! of this crate's embedding compatibility surface; `dream_archive` upgrades can affect downstream
//! Lua/module integration even when `dream-archivetool` policy APIs do not change.
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
//! Extraction rejects absolute paths, parent-directory traversal, NUL bytes, and colon-containing
//! components. Disk extraction streams payloads into temporary files before replacing the
//! destination, but it is not an `openat`-style jail against pre-existing symlinks in the output
//! tree. Archive creation and update reject input symlinks encountered during collection unless
//! `follow_symlinks` is explicitly enabled; callers must keep input trees trusted and stable during
//! the write. Paths are preflighted before handing file/archive-entry sources to `dream_archive`
//! builders, including BA2 DX10 texture preservation where the backend can copy native chunks.
//!
//! Enable the `lua` feature to compile the Lua module for embedding applications that already
//! choose an `mlua` runtime. This crate deliberately does not select a Lua runtime for normal
//! library consumers.
//!
//! Enable `standalone-lua` only for this crate's tests, examples, and documentation builds. It
//! selects vendored `LuaJIT` 5.2 through `mlua`, which is useful here and rude everywhere else.
//!
#![cfg_attr(
    feature = "standalone-lua",
    doc = "With `standalone-lua` enabled, see the [`lua`] module for the embedded Lua table API."
)]
#![cfg_attr(
    all(feature = "lua", not(feature = "standalone-lua")),
    doc = "With `lua` enabled, see the [`lua`] module. The embedding application must provide the `mlua` runtime feature."
)]

pub mod archive;
mod archive_plan;
pub mod create;
pub mod diff;
pub mod entry;
pub mod error;
pub mod extract;
pub mod format;
mod loaded;
pub mod path;
mod paths;
mod rewrite_policy;
pub mod verify;

#[cfg(feature = "lua")]
pub mod lua;

pub use dream_archive;

pub use archive::{ArchiveInfo, ArchiveTool, Ba2Info, OpenArchive, Tes4Info};
pub use create::{
    AddOptions, AddPlan, ArchivePlanAction, ArchivePlanEntry, ArchivePlanOperation, Ba2ArchiveKind,
    Ba2Version, CreateOptions, CreatePlan, Tes4Version,
};
pub use diff::{
    DiffChange, DiffComparison, DiffEntry, DiffEntryState, DiffOptions, DiffReport, diff_archives,
};
pub use entry::ArchiveEntry;
pub use error::{ArchiveError, Result};
pub use extract::{
    ExtractAllOptions, ExtractAllPlan, ExtractOptions, ExtractPlanAction, ExtractPlanEntry,
    ExtractPlanOperation, ExtractSummary, OverwriteMode,
};
pub use format::ArchiveFormat;
pub use path::{decode_archive_path_hex, encode_archive_path_hex, normalize_archive_path_bytes};
pub use verify::{VerifyOptions, VerifyPathIssue, VerifyReport, verify_archive};
