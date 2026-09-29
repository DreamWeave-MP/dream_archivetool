// SPDX-License-Identifier: GPL-3.0-or-later

//! Luau bindings for the `dream_archivetool` policy layer as an
//! [l3i](https://github.com/DreamWeave-MP/l3i) extension: `dream.archivetool`, module
//! `@dream/archivetool`, requiring `dream.archive`.
//!
//! The extension adds the policy operations to `dream.archive.Archive` itself (`verify`,
//! `diff`, `extract`, `extractMany`, `planExtract`, `extractAll`, `planExtractAll`, and the
//! `*ByPathHex` forms) through the planner's augmentation: there is no second wrapper type,
//! and the host composes both extensions into one `RuntimePlan`:
//!
//! ```no_run
//! use l3i::Runtime;
//! use l3i::extension::{RuntimePlan, RuntimePolicy};
//!
//! # fn main() -> l3i::Result<()> {
//! let plan = RuntimePlan::builder()
//!     .policy(
//!         RuntimePolicy::new()
//!             .compat_global("@dream/archive", "dreamArchive")
//!             .compat_global("@dream/archivetool", "dreamArchivetool"),
//!     )
//!     .extension(dream_archive::luau::ArchiveExtension)
//!     .extension(dream_archivetool::luau::ArchivetoolExtension)
//!     .finalize()?;
//! let runtime = Runtime::from_plan(&plan)?;
//! runtime.exec(r#"
//!     local archive = dreamArchive.openPath("Morrowind.bsa")
//!     local plan = archive:planExtractAll({ output = "out" })
//!     for _, row in plan.entries do print(row.action, row.path) end
//!     print(dreamArchivetool.info("Morrowind.bsa").fileCount)
//! "#)?;
//! # Ok(())
//! # }
//! ```
//!
//! The module `@dream/archivetool` keeps the host-path functions (`info`, `verify`, `diff`,
//! `extract*`, `planExtract*`, `extractAll`, `planExtractAll`, `create`, `planCreate`, `add`,
//! `planAdd`), which open the archive for that one operation.
//!
//! # Reports and plans
//!
//! Reports and plans are userdata whose scalar fields are getters and whose row lists are
//! sequence views of row handles (`#`, `[i]`, `for`); every one has `:toTable()` for the old
//! nested-table shape. Sizes (`size`, `compressedSize`) are Luau integers, payload fingerprints
//! are Luau integers carrying all 64 FNV-1a bits, and counts (`fileCount`, `extracted`, `files`,
//! ...) stay plain numbers.
//!
//! Integers are this extension's choice (through [`l3i::convert::Integer`] and
//! [`l3i::convert::Bits64`]); l3i pushes plain Rust integers as numbers, and `dream_archive`'s
//! own `Entry.size` is a number. Luau integers take `==` and `tostring` but not `<`, `+` or
//! `tonumber`, and never equal a number: scripts compare and convert them with the `integer`
//! library (`integer.lt`, `integer.add`, `integer.tonumber`, `integer.create`) and write literals
//! as `5i`.
//!
//! # Bytes and paths
//!
//! Host paths are UTF-8 strings. Archive entry arguments are byte strings, entry handles from
//! `archive:entries()`, or a whole `dream.archive.Entries` view (`extractMany(archive:entries())`);
//! the `*ByPathHex` forms take `pathBytesHex` keys. Option tables are strict: an unknown key is
//! an error that names it.

use std::path::PathBuf;
use std::rc::Rc;

use dream_archive::luau::{ARCHIVE_KEY, Archive, Entries, Entry};
use l3i::bind::Call;
use l3i::convert::{Bits64, Integer, Push};
use l3i::extension::{Extension, ExtensionDescriptor, TagPolicy};
use l3i::options::Options;
use l3i::sequence::{Sequence, SequenceSource};
use l3i::stack::{Frame, Scope, ValueView};
use l3i::userdata::{Owned, Userdata};
use l3i::value::Table;
use l3i::{Error, Result};

use crate::{
    AddOptions, ArchiveFormat, ArchivePlanAction, ArchivePlanEntry, ArchivePlanOperation,
    ArchiveTool, Ba2ArchiveKind, Ba2Version, CreateOptions, DiffComparison, DiffOptions,
    ExtractAllOptions, ExtractOptions, ExtractPlanAction, ExtractPlanOperation, OverwriteMode,
    Tes4Version, VerifyOptions,
};

/// The extension id.
pub const EXTENSION_ID: &str = "dream.archivetool";
/// The canonical module path.
pub const MODULE: &str = "@dream/archivetool";

/// The `dream.archivetool` extension: requires `dream.archive`, augments its archive type.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArchivetoolExtension;

// ---------------------------------------------------------------------------------------------
// Reports and plans as userdata
// ---------------------------------------------------------------------------------------------

/// A verification report (`dream.archivetool.VerifyReport`).
#[derive(Clone)]
pub struct VerifyReport(Rc<crate::VerifyReport>);

/// A diff report (`dream.archivetool.DiffReport`).
#[derive(Clone)]
pub struct DiffReport(Rc<crate::DiffReport>);

/// An extraction plan (`dream.archivetool.ExtractPlan`).
#[derive(Clone)]
pub struct ExtractPlan(Rc<crate::ExtractAllPlan>);

/// A creation plan (`dream.archivetool.CreatePlan`).
#[derive(Clone)]
pub struct CreatePlan(Rc<crate::CreatePlan>);

/// An add/update plan (`dream.archivetool.AddPlan`).
#[derive(Clone)]
pub struct AddPlan(Rc<crate::AddPlan>);

// SAFETY: every report and plan type here is plain Rust data behind an `Rc`; dropping one never
// touches the Lua API.
unsafe impl Userdata for VerifyReport {
    const NAME: &'static str = "dream.archivetool.VerifyReport";
}
// SAFETY: as `VerifyReport`.
unsafe impl Userdata for DiffReport {
    const NAME: &'static str = "dream.archivetool.DiffReport";
}
// SAFETY: as `VerifyReport`.
unsafe impl Userdata for ExtractPlan {
    const NAME: &'static str = "dream.archivetool.ExtractPlan";
}
// SAFETY: as `VerifyReport`.
unsafe impl Userdata for CreatePlan {
    const NAME: &'static str = "dream.archivetool.CreatePlan";
}
// SAFETY: as `VerifyReport`.
unsafe impl Userdata for AddPlan {
    const NAME: &'static str = "dream.archivetool.AddPlan";
}

impl VerifyReport {
    /// The report.
    #[must_use]
    pub fn report(&self) -> &crate::VerifyReport {
        &self.0
    }
}

impl DiffReport {
    /// The report.
    #[must_use]
    pub fn report(&self) -> &crate::DiffReport {
        &self.0
    }
}

impl ExtractPlan {
    /// The plan.
    #[must_use]
    pub fn plan(&self) -> &crate::ExtractAllPlan {
        &self.0
    }
}

impl CreatePlan {
    /// The plan.
    #[must_use]
    pub fn plan(&self) -> &crate::CreatePlan {
        &self.0
    }
}

impl AddPlan {
    /// The plan.
    #[must_use]
    pub fn plan(&self) -> &crate::AddPlan {
        &self.0
    }
}

// ---------------------------------------------------------------------------------------------
// Row handles and their sequence views
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum IssueList {
    Duplicates,
    Unsafe,
}

/// The `duplicateNormalizedPaths` or `unsafePaths` rows of a verify report.
pub struct PathIssues {
    report: Rc<crate::VerifyReport>,
    list: IssueList,
}

/// One path issue row (`dream.archivetool.PathIssue`).
pub struct PathIssue {
    report: Rc<crate::VerifyReport>,
    list: IssueList,
    index: usize,
}

impl PathIssues {
    fn rows(&self) -> &[crate::VerifyPathIssue] {
        issue_rows(&self.report, self.list)
    }
}

fn issue_rows(report: &crate::VerifyReport, list: IssueList) -> &[crate::VerifyPathIssue] {
    match list {
        IssueList::Duplicates => &report.duplicate_normalized_paths,
        IssueList::Unsafe => &report.unsafe_paths,
    }
}

impl PathIssue {
    fn row(&self) -> &crate::VerifyPathIssue {
        &issue_rows(&self.report, self.list)[self.index]
    }
}

impl SequenceSource for PathIssues {
    const NAME: &'static str = "dream.archivetool.PathIssues";
    type Item = Owned<PathIssue>;
    fn len(&self) -> usize {
        self.rows().len()
    }
    fn get(&self, index: usize) -> Option<Owned<PathIssue>> {
        (index < self.rows().len()).then(|| {
            Owned(PathIssue {
                report: Rc::clone(&self.report),
                list: self.list,
                index,
            })
        })
    }
}

/// The `warnings` of a verify report.
pub struct Warnings(Rc<crate::VerifyReport>);

impl SequenceSource for Warnings {
    const NAME: &'static str = "dream.archivetool.Warnings";
    type Item = String;
    fn len(&self) -> usize {
        self.0.warnings.len()
    }
    fn get(&self, index: usize) -> Option<String> {
        self.0.warnings.get(index).cloned()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffSide {
    Added,
    Removed,
}

/// The `added` or `removed` rows of a diff report.
pub struct DiffEntries {
    report: Rc<crate::DiffReport>,
    side: DiffSide,
}

/// One added or removed entry (`dream.archivetool.DiffEntry`).
pub struct DiffEntry {
    report: Rc<crate::DiffReport>,
    side: DiffSide,
    index: usize,
}

fn diff_rows(report: &crate::DiffReport, side: DiffSide) -> &[crate::DiffEntry] {
    match side {
        DiffSide::Added => &report.added,
        DiffSide::Removed => &report.removed,
    }
}

impl DiffEntry {
    fn row(&self) -> &crate::DiffEntry {
        &diff_rows(&self.report, self.side)[self.index]
    }
}

impl SequenceSource for DiffEntries {
    const NAME: &'static str = "dream.archivetool.DiffEntries";
    type Item = Owned<DiffEntry>;
    fn len(&self) -> usize {
        diff_rows(&self.report, self.side).len()
    }
    fn get(&self, index: usize) -> Option<Owned<DiffEntry>> {
        (index < self.len()).then(|| {
            Owned(DiffEntry {
                report: Rc::clone(&self.report),
                side: self.side,
                index,
            })
        })
    }
}

/// The `changed` rows of a diff report.
pub struct DiffChanges(Rc<crate::DiffReport>);

/// One changed entry (`dream.archivetool.DiffChange`) with its `old` and `new` states.
pub struct DiffChange {
    report: Rc<crate::DiffReport>,
    index: usize,
}

/// One side of a changed entry (`dream.archivetool.DiffState`).
#[derive(Clone)]
pub struct DiffState {
    report: Rc<crate::DiffReport>,
    index: usize,
    old: bool,
}

impl DiffChange {
    fn row(&self) -> &crate::DiffChange {
        &self.report.changed[self.index]
    }
}

impl DiffState {
    fn state(&self) -> &crate::DiffEntryState {
        let change = &self.report.changed[self.index];
        if self.old { &change.old } else { &change.new }
    }
}

impl SequenceSource for DiffChanges {
    const NAME: &'static str = "dream.archivetool.DiffChanges";
    type Item = Owned<DiffChange>;
    fn len(&self) -> usize {
        self.0.changed.len()
    }
    fn get(&self, index: usize) -> Option<Owned<DiffChange>> {
        (index < self.0.changed.len()).then(|| {
            Owned(DiffChange {
                report: Rc::clone(&self.0),
                index,
            })
        })
    }
}

/// The rows of an extraction plan.
pub struct ExtractPlanRows(Rc<crate::ExtractAllPlan>);

/// One planned extraction (`dream.archivetool.ExtractPlanRow`).
pub struct ExtractPlanRow {
    plan: Rc<crate::ExtractAllPlan>,
    index: usize,
}

impl ExtractPlanRow {
    fn row(&self) -> &crate::ExtractPlanEntry {
        &self.plan.entries[self.index]
    }
}

impl SequenceSource for ExtractPlanRows {
    const NAME: &'static str = "dream.archivetool.ExtractPlanRows";
    type Item = Owned<ExtractPlanRow>;
    fn len(&self) -> usize {
        self.0.entries.len()
    }
    fn get(&self, index: usize) -> Option<Owned<ExtractPlanRow>> {
        (index < self.0.entries.len()).then(|| {
            Owned(ExtractPlanRow {
                plan: Rc::clone(&self.0),
                index,
            })
        })
    }
}

/// The plan an archive-plan row belongs to.
#[derive(Clone)]
enum ArchivePlan {
    Create(Rc<crate::CreatePlan>),
    Add(Rc<crate::AddPlan>),
}

impl ArchivePlan {
    fn entries(&self) -> &[ArchivePlanEntry] {
        match self {
            ArchivePlan::Create(plan) => &plan.entries,
            ArchivePlan::Add(plan) => &plan.entries,
        }
    }
}

/// The rows of a create or add plan.
pub struct ArchivePlanRows(ArchivePlan);

/// One planned archive member (`dream.archivetool.ArchivePlanRow`).
pub struct ArchivePlanRow {
    plan: ArchivePlan,
    index: usize,
}

impl ArchivePlanRow {
    fn row(&self) -> &ArchivePlanEntry {
        &self.plan.entries()[self.index]
    }
}

impl SequenceSource for ArchivePlanRows {
    const NAME: &'static str = "dream.archivetool.ArchivePlanRows";
    type Item = Owned<ArchivePlanRow>;
    fn len(&self) -> usize {
        self.0.entries().len()
    }
    fn get(&self, index: usize) -> Option<Owned<ArchivePlanRow>> {
        (index < self.0.entries().len()).then(|| {
            Owned(ArchivePlanRow {
                plan: self.0.clone(),
                index,
            })
        })
    }
}

// SAFETY: row handles are an `Rc` and an index; dropping one never touches the Lua API.
unsafe impl Userdata for PathIssue {
    const NAME: &'static str = "dream.archivetool.PathIssue";
}
// SAFETY: as `PathIssue`.
unsafe impl Userdata for DiffEntry {
    const NAME: &'static str = "dream.archivetool.DiffEntry";
}
// SAFETY: as `PathIssue`.
unsafe impl Userdata for DiffChange {
    const NAME: &'static str = "dream.archivetool.DiffChange";
}
// SAFETY: as `PathIssue`.
unsafe impl Userdata for DiffState {
    const NAME: &'static str = "dream.archivetool.DiffState";
}
// SAFETY: as `PathIssue`.
unsafe impl Userdata for ExtractPlanRow {
    const NAME: &'static str = "dream.archivetool.ExtractPlanRow";
}
// SAFETY: as `PathIssue`.
unsafe impl Userdata for ArchivePlanRow {
    const NAME: &'static str = "dream.archivetool.ArchivePlanRow";
}

// ---------------------------------------------------------------------------------------------
// Value helpers
// ---------------------------------------------------------------------------------------------

/// A tool error in Luau's words: the library names its options by their Rust fields.
fn tool_error(error: crate::ArchiveError) -> Error {
    match error {
        crate::ArchiveError::SymlinkInput(path) => Error::runtime(format!(
            "refusing to follow symlink input path: {path}; set followSymlinks to opt in"
        )),
        error => Error::runtime(error.to_string()),
    }
}

fn format_name(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tes3 => "bsaTes3",
        ArchiveFormat::Tes4 => "bsaTes4",
        ArchiveFormat::Ba2 => "ba2",
    }
}

/// A byte count as a Luau integer (a size is an exact 64-bit quantity).
fn size(value: u64) -> Integer {
    Integer(i64::try_from(value).unwrap_or(i64::MAX))
}

/// A count as an exact Luau number.
fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// A payload fingerprint (16 hex digits of FNV-1a) as the integer it is.
fn fingerprint(text: Option<&str>) -> Option<Bits64> {
    text.and_then(|text| u64::from_str_radix(text, 16).ok())
        .map(Bits64)
}

fn diff_comparison_name(comparison: DiffComparison) -> &'static str {
    match comparison {
        DiffComparison::MetadataOnly => "metadataOnly",
        DiffComparison::PayloadFingerprint => "payloadFingerprint",
    }
}

fn archive_plan_operation_name(operation: ArchivePlanOperation) -> &'static str {
    match operation {
        ArchivePlanOperation::Create => "create",
        ArchivePlanOperation::Add => "add",
    }
}

fn archive_plan_action_name(action: ArchivePlanAction) -> &'static str {
    match action {
        ArchivePlanAction::Add => "add",
        ArchivePlanAction::Replace => "replace",
        ArchivePlanAction::Preserve => "preserve",
    }
}

fn extract_plan_operation_name(operation: ExtractPlanOperation) -> &'static str {
    match operation {
        ExtractPlanOperation::Extract => "extract",
        ExtractPlanOperation::ExtractAll => "extractAll",
    }
}

fn extract_plan_action_name(action: ExtractPlanAction) -> &'static str {
    match action {
        ExtractPlanAction::Extract => "extract",
        ExtractPlanAction::Skip => "skip",
        ExtractPlanAction::Overwrite => "overwrite",
        ExtractPlanAction::Conflict => "conflict",
    }
}

/// Spell a kebab-case policy name (as the CLI reports it) the Luau way: `"hash-only"` becomes
/// `"hashOnly"`, matching `dreamArchive`'s own enum strings.
fn kebab_to_camel(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut upper = false;
    for ch in value.chars() {
        if ch == '-' {
            upper = true;
        } else if upper {
            out.push(ch.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// The label a handle-based operation reports for the archive.
fn archive_label(archive: &Archive) -> String {
    archive
        .path()
        .map_or_else(|| "<memory>".to_owned(), |path| path.display().to_string())
}

fn loaded(archive: &Archive) -> crate::loaded::LoadedArchiveRef<'_> {
    crate::loaded::LoadedArchiveRef::from_archive(archive.archive())
}

/// An array table of `items`.
fn array<T: Push>(scope: &impl Scope, items: &[T]) -> Result<Table> {
    let table = Table::new(scope, items.len(), 0)?;
    scope.with_frame(|frame| {
        let view = table.push_to(frame)?;
        for (index, item) in items.iter().enumerate() {
            item.push_into(frame)?;
            view.raw_set_index(frame, count(index) + 1)?;
        }
        Ok(())
    })?;
    Ok(table)
}

/// An array table built row by row; `row_table` fills each row on the innermost frame.
fn array_of<R>(
    scope: &impl Scope,
    rows: &[R],
    row_table: impl Fn(&Frame<'_>, &Table, &R) -> Result<()>,
) -> Result<Table> {
    let table = Table::new(scope, rows.len(), 0)?;
    scope.with_frame(|frame| {
        let view = table.push_to(frame)?;
        for (index, row) in rows.iter().enumerate() {
            let entry = Table::new(frame, 0, 5)?;
            row_table(frame, &entry, row)?;
            entry.push_into(frame)?;
            view.raw_set_index(frame, count(index) + 1)?;
        }
        Ok(())
    })?;
    Ok(table)
}

// ---------------------------------------------------------------------------------------------
// Option tables
// ---------------------------------------------------------------------------------------------

fn overwrite_mode(value: &str) -> Result<OverwriteMode> {
    match value {
        "fail" => Ok(OverwriteMode::Fail),
        "overwrite" => Ok(OverwriteMode::Overwrite),
        "skip" => Ok(OverwriteMode::Skip),
        value => Err(Error::runtime(format!("unknown overwrite mode: {value}"))),
    }
}

fn optional_path(options: &mut Options<'_, '_>, key: &str) -> Result<Option<PathBuf>> {
    options.optional_str(key, |text| Ok(PathBuf::from(text)))
}

/// An optional option table field read as an enum: `None` when absent, the parse deferred so
/// the caller can refuse an irrelevant field before it judges its value.
fn optional_enum<T>(
    options: &mut Options<'_, '_>,
    key: &str,
    parse: impl FnOnce(&str) -> Result<T>,
) -> Result<Option<Result<T>>> {
    options.optional_str(key, |text| Ok(parse(text)))
}

fn tes4_version(value: &str) -> Result<Tes4Version> {
    match value {
        "oblivion" => Ok(Tes4Version::Oblivion),
        "fallout3" => Ok(Tes4Version::Fallout3),
        "skyrim" => Ok(Tes4Version::Skyrim),
        "skyrimSe" | "sse" => Ok(Tes4Version::SkyrimSe),
        value => Err(Error::runtime(format!("unknown TES4 version: {value}"))),
    }
}

fn ba2_kind(value: &str) -> Result<Ba2ArchiveKind> {
    match value {
        "gnrl" => Ok(Ba2ArchiveKind::Gnrl),
        "dx10" => Ok(Ba2ArchiveKind::Dx10),
        "gnmf" => Ok(Ba2ArchiveKind::Gnmf),
        value => Err(Error::runtime(format!("unknown BA2 kind: {value}"))),
    }
}

fn ba2_version(value: &str) -> Result<Ba2Version> {
    match value {
        "fallout4" => Ok(Ba2Version::Fallout4),
        "starfield" => Ok(Ba2Version::Starfield),
        "fallout4NextGen" => Ok(Ba2Version::Fallout4NextGen),
        value => Err(Error::runtime(format!("unknown BA2 version: {value}"))),
    }
}

/// Reads an optional option table; `nil` or absent means the defaults.
fn read_options<T: Default>(
    scope: &impl Scope,
    view: Option<ValueView<'_>>,
    context: &str,
    body: impl FnOnce(&mut Options<'_, '_>) -> Result<T>,
) -> Result<T> {
    match view {
        None => Ok(T::default()),
        Some(view) if view.is_nil() => Ok(T::default()),
        Some(view) => Options::read(scope, view, context, body),
    }
}

fn extract_options(
    scope: &impl Scope,
    view: Option<ValueView<'_>>,
    context: &str,
) -> Result<ExtractOptions> {
    read_options(scope, view, context, |o| {
        Ok(ExtractOptions {
            output: optional_path(o, "output")?,
            overwrite: o
                .optional_str("overwrite", overwrite_mode)?
                .unwrap_or(OverwriteMode::Fail),
            preserve_paths: o.or("preservePaths", true)?,
            fsync: o.or("fsync", false)?,
        })
    })
}

fn extract_all_options(
    scope: &impl Scope,
    view: Option<ValueView<'_>>,
    context: &str,
) -> Result<ExtractAllOptions> {
    read_options(scope, view, context, |o| {
        Ok(ExtractAllOptions {
            output: optional_path(o, "output")?,
            overwrite: o
                .optional_str("overwrite", overwrite_mode)?
                .unwrap_or(OverwriteMode::Fail),
            fsync: o.or("fsync", false)?,
        })
    })
}

fn verify_options(
    scope: &impl Scope,
    view: Option<ValueView<'_>>,
    context: &str,
) -> Result<VerifyOptions> {
    read_options(scope, view, context, |o| {
        Ok(VerifyOptions {
            read_payloads: o.or("readPayloads", false)?,
        })
    })
}

fn diff_options(
    scope: &impl Scope,
    view: Option<ValueView<'_>>,
    context: &str,
) -> Result<DiffOptions> {
    read_options(scope, view, context, |o| {
        Ok(DiffOptions {
            fingerprint_payloads: o.or("fingerprintPayloads", false)?,
        })
    })
}

fn create_options(
    scope: &impl Scope,
    view: Option<ValueView<'_>>,
    context: &str,
) -> Result<CreateOptions> {
    read_options(scope, view, context, |o| {
        let format = o
            .optional_str("format", |value| match value {
                "tes3" | "bsaTes3" => Ok(ArchiveFormat::Tes3),
                "tes4" | "bsaTes4" => Ok(ArchiveFormat::Tes4),
                "ba2" => Ok(ArchiveFormat::Ba2),
                value => Err(Error::runtime(format!("unknown archive format: {value}"))),
            })?
            .unwrap_or(ArchiveFormat::Tes3);
        let tes4_version = optional_enum(o, "tes4Version", tes4_version)?;
        let ba2_kind = optional_enum(o, "ba2Kind", ba2_kind)?;
        let ba2_version = optional_enum(o, "ba2Version", ba2_version)?;
        let compress = o.or("compress", false)?;
        let irrelevant = |option: &str, supplied: bool| {
            if supplied {
                Err(Error::runtime(format!(
                    "{option} is not valid with format {}",
                    format_name(format)
                )))
            } else {
                Ok(())
            }
        };
        match format {
            ArchiveFormat::Tes3 => {
                irrelevant("compress", compress)?;
                irrelevant("tes4Version", tes4_version.is_some())?;
                irrelevant("ba2Kind", ba2_kind.is_some())?;
                irrelevant("ba2Version", ba2_version.is_some())?;
            }
            ArchiveFormat::Tes4 => {
                irrelevant("ba2Kind", ba2_kind.is_some())?;
                irrelevant("ba2Version", ba2_version.is_some())?;
            }
            ArchiveFormat::Ba2 => irrelevant("tes4Version", tes4_version.is_some())?,
        }
        Ok(CreateOptions {
            format,
            tes4_version: tes4_version.transpose()?.unwrap_or(Tes4Version::Oblivion),
            ba2_kind: ba2_kind.transpose()?.unwrap_or(Ba2ArchiveKind::Gnrl),
            ba2_version: ba2_version.transpose()?.unwrap_or(Ba2Version::Fallout4),
            compress,
            fsync: o.or("fsync", false)?,
            follow_symlinks: o.or("followSymlinks", false)?,
        })
    })
}

fn add_options(scope: &impl Scope, view: ValueView<'_>, context: &str) -> Result<AddOptions> {
    Options::read(scope, view, context, |o| {
        let inputs = o.required_table("inputs", |frame, inputs| {
            let len = dense_len(frame, &inputs, "inputs")?;
            let mut paths = Vec::with_capacity(len);
            inputs.for_each_array(frame, |_, index, value| {
                let text = value.read::<&str>().map_err(|_| {
                    value.field_type_error(&format!("inputs[{index}]"), "a UTF-8 host path string")
                })?;
                paths.push(PathBuf::from(text));
                Ok(())
            })?;
            Ok(paths)
        })?;
        if inputs.is_empty() {
            return Err(Error::runtime(format!(
                "{context} requires at least one input path"
            )));
        }
        Ok(AddOptions {
            inputs,
            output: optional_path(o, "output")?,
            fsync: o.or("fsync", false)?,
            follow_symlinks: o.or("followSymlinks", false)?,
        })
    })
}

// ---------------------------------------------------------------------------------------------
// Entry arguments
// ---------------------------------------------------------------------------------------------

/// The archive path bytes one `entry` argument names: a byte string or an entry handle.
fn entry_bytes(value: ValueView<'_>, context: &str) -> Result<Vec<u8>> {
    if let Some(entry) = l3i::userdata::receiver::<Entry>(value) {
        return entry
            .facade()
            .path()
            .map(|path| path.to_vec())
            .ok_or_else(|| {
                Error::runtime(format!(
                    "{context}: entry {} has no path; hash-only entries are not extraction targets",
                    entry.index() + 1
                ))
            });
    }
    if value.is_string() {
        return Ok(value.read::<&[u8]>()?.to_vec());
    }
    Err(value.field_type_error(context, "an archive path or an entry handle"))
}

/// The archive path bytes of an `entries` argument: an array of byte strings or entry handles,
/// or a whole entries view.
fn entry_list(scope: &impl Scope, value: ValueView<'_>, context: &str) -> Result<Vec<Vec<u8>>> {
    if let Some(entries) = l3i::userdata::receiver::<Sequence<Entries>>(value) {
        let mut paths = Vec::with_capacity(entries.0.len());
        for index in 0..entries.0.len() {
            let Some(Owned(entry)) = entries.0.get(index) else {
                break;
            };
            let Some(path) = entry.facade().path() else {
                return Err(Error::runtime(format!(
                    "{context}: entry {} has no path; hash-only entries are not extraction targets",
                    index + 1
                )));
            };
            paths.push(path.to_vec());
        }
        return Ok(paths);
    }
    let Ok(table) = value.as_table() else {
        return Err(Error::runtime(format!(
            "{context}: entries must be an array of archive paths or an entries view, got {}",
            value.type_of().name()
        )));
    };
    scope.with_frame(|frame| {
        let path = format!("{context}.entries");
        let len = dense_len(frame, &table, &path)?;
        let mut paths = Vec::with_capacity(len);
        table.for_each_array(frame, |_, position, item| {
            paths.push(entry_bytes(item, &format!("{path}[{position}]"))?);
            Ok(())
        })?;
        Ok(paths)
    })
}

/// The length of `view` as a dense 1-based array; any other key is an error naming `path`, the
/// field path the caller's context gives the array (`archive:extractMany.entries`, or `inputs`
/// under an option reader that prefixes its own context).
fn dense_len(frame: &Frame<'_>, view: &l3i::stack::TableView<'_>, path: &str) -> Result<usize> {
    let len = view.raw_len();
    let mut seen = 0usize;
    view.for_each(frame, |_, key, _| {
        let index = key
            .read::<l3i::convert::Exact<i64>>()
            .ok()
            .map(|index| index.0);
        match index {
            Some(index) if index >= 1 && usize::try_from(index).is_ok_and(|index| index <= len) => {
                seen += 1;
                Ok(())
            }
            _ => Err(Error::runtime(format!(
                "{path} must be a dense 1-based array"
            ))),
        }
    })?;
    if seen != len {
        return Err(Error::runtime(format!("{path} must not contain holes")));
    }
    Ok(len)
}

fn hex_entry(value: &str, context: &str) -> Result<Vec<u8>> {
    crate::path::decode_archive_path_hex(value)
        .map_err(|error| Error::runtime(format!("{context}: invalid pathBytesHex: {error}")))
}

/// The archive path bytes of a `pathBytesHex` array argument, read from its slot.
fn hex_entry_list(scope: &impl Scope, value: ValueView<'_>, context: &str) -> Result<Vec<Vec<u8>>> {
    let table = value.as_table()?;
    scope.with_frame(|frame| {
        let path = format!("{context}.entries");
        let len = dense_len(frame, &table, &path)?;
        let mut paths = Vec::with_capacity(len);
        table.for_each_array(frame, |_, position, item| {
            let path = format!("{path}[{position}]");
            let text = item
                .read::<&str>()
                .map_err(|_| item.field_type_error(&path, "a UTF-8 pathBytesHex string"))?;
            paths.push(hex_entry(text, &path)?);
            Ok(())
        })?;
        Ok(paths)
    })
}

fn summary_table(scope: &impl Scope, summary: &crate::ExtractSummary) -> Result<Table> {
    let table = Table::new(scope, 0, 2)?;
    table.set(scope, "extracted", &count(summary.extracted))?;
    table.set(scope, "skipped", &count(summary.skipped))?;
    Ok(table)
}

fn files_table(scope: &impl Scope, files: usize) -> Result<Table> {
    let table = Table::new(scope, 0, 1)?;
    table.set(scope, "files", &count(files))?;
    Ok(table)
}

fn info_table(scope: &impl Scope, info: &crate::ArchiveInfo) -> Result<Table> {
    let table = Table::new(scope, 0, 9)?;
    table.set(scope, "path", &info.path)?;
    table.set(scope, "format", format_name(info.format))?;
    table.set(scope, "fileCount", &count(info.file_count))?;
    table.set(scope, "namedEntryCount", &count(info.named_entry_count))?;
    table.set(scope, "hasUnnameableEntries", &info.has_unnameable_entries)?;
    table.set(scope, "rewritable", &info.rewritable)?;
    if let Some(blocker) = &info.rewrite_blocker {
        table.set(scope, "rewriteBlocker", blocker)?;
    }
    if let Some(tes4) = &info.tes4 {
        let nested = Table::new(scope, 0, 7)?;
        nested.set(scope, "version", &tes4.version)?;
        nested.set(scope, "archiveTypes", &tes4.archive_types)?;
        nested.set(
            scope,
            "archiveTypesBits",
            &i64::from(tes4.archive_types_bits),
        )?;
        let flags: Vec<String> = tes4
            .archive_flags
            .iter()
            .map(|flag| kebab_to_camel(flag))
            .collect();
        nested.set(scope, "archiveFlags", &array(scope, &flags)?)?;
        nested.set(
            scope,
            "archiveFlagsBits",
            &i64::from(tes4.archive_flags_bits),
        )?;
        nested.set(
            scope,
            "unsupportedArchiveFlagsBits",
            &i64::from(tes4.unsupported_archive_flags_bits),
        )?;
        nested.set(scope, "nameMode", &kebab_to_camel(&tes4.name_mode))?;
        table.set(scope, "tes4", &nested)?;
    }
    if let Some(ba2) = &info.ba2 {
        let nested = Table::new(scope, 0, 4)?;
        nested.set(scope, "version", &ba2.version)?;
        nested.set(scope, "payloadFormat", &ba2.payload_format)?;
        nested.set(scope, "compressionFormat", &ba2.compression_format)?;
        nested.set(scope, "strings", &ba2.strings)?;
        table.set(scope, "ba2", &nested)?;
    }
    Ok(table)
}

// ---------------------------------------------------------------------------------------------
// toTable shapes
// ---------------------------------------------------------------------------------------------

fn issue_table(scope: &impl Scope, table: &Table, issue: &crate::VerifyPathIssue) -> Result<()> {
    table.set(scope, "path", &issue.path)?;
    table.set(scope, "pathBytesHex", &issue.path_bytes_hex)?;
    if let Some(raw) = &issue.raw_path_bytes_hex {
        table.set(scope, "rawPathBytesHex", raw)?;
    }
    if let Some(colliding) = &issue.colliding_raw_path_bytes_hex {
        table.set(scope, "collidingRawPathBytesHex", colliding)?;
    }
    Ok(())
}

fn verify_table(scope: &impl Scope, report: &crate::VerifyReport) -> Result<Table> {
    let table = Table::new(scope, 0, 11)?;
    table.set(scope, "path", &report.path)?;
    table.set(scope, "format", format_name(report.format))?;
    table.set(scope, "fileCount", &count(report.file_count))?;
    table.set(scope, "namedEntryCount", &count(report.named_entry_count))?;
    table.set(
        scope,
        "unnameableEntries",
        &count(report.unnameable_entries),
    )?;
    table.set(scope, "rewritable", &report.rewritable)?;
    if let Some(blocker) = &report.rewrite_blocker {
        table.set(scope, "rewriteBlocker", blocker)?;
    }
    table.set(
        scope,
        "duplicateNormalizedPaths",
        &array_of(scope, &report.duplicate_normalized_paths, |f, t, issue| {
            issue_table(f, t, issue)
        })?,
    )?;
    table.set(
        scope,
        "unsafePaths",
        &array_of(scope, &report.unsafe_paths, |f, t, issue| {
            issue_table(f, t, issue)
        })?,
    )?;
    if let Some(read) = report.payloads_read {
        table.set(scope, "payloadsRead", &count(read))?;
    }
    table.set(scope, "warnings", &array(scope, &report.warnings)?)?;
    Ok(table)
}

fn state_fields(
    scope: &impl Scope,
    table: &Table,
    size_: Option<u64>,
    compressed: Option<u64>,
    print: Option<&str>,
) -> Result<()> {
    if let Some(value) = size_ {
        table.set(scope, "size", &size(value))?;
    }
    if let Some(value) = compressed {
        table.set(scope, "compressedSize", &size(value))?;
    }
    if let Some(value) = fingerprint(print) {
        table.set(scope, "payloadFingerprint", &value)?;
    }
    Ok(())
}

fn diff_entry_table(scope: &impl Scope, table: &Table, entry: &crate::DiffEntry) -> Result<()> {
    table.set(scope, "path", &entry.path)?;
    table.set(scope, "pathBytesHex", &entry.path_bytes_hex)?;
    state_fields(
        scope,
        table,
        entry.size,
        entry.compressed_size,
        entry.payload_fingerprint.as_deref(),
    )
}

fn diff_state_table(scope: &impl Scope, state: &crate::DiffEntryState) -> Result<Table> {
    let table = Table::new(scope, 0, 3)?;
    state_fields(
        scope,
        &table,
        state.size,
        state.compressed_size,
        state.payload_fingerprint.as_deref(),
    )?;
    Ok(table)
}

fn diff_table(scope: &impl Scope, report: &crate::DiffReport) -> Result<Table> {
    let table = Table::new(scope, 0, 8)?;
    table.set(scope, "old", &report.old)?;
    table.set(scope, "new", &report.new)?;
    table.set(scope, "comparison", diff_comparison_name(report.comparison))?;
    table.set(scope, "fingerprintPayloads", &report.fingerprint_payloads)?;
    table.set(
        scope,
        "added",
        &array_of(scope, &report.added, |f, t, e| diff_entry_table(f, t, e))?,
    )?;
    table.set(
        scope,
        "removed",
        &array_of(scope, &report.removed, |f, t, e| diff_entry_table(f, t, e))?,
    )?;
    table.set(
        scope,
        "changed",
        &array_of(scope, &report.changed, |f, t, change| {
            t.set(f, "path", &change.path)?;
            t.set(f, "pathBytesHex", &change.path_bytes_hex)?;
            t.set(f, "old", &diff_state_table(f, &change.old)?)?;
            t.set(f, "new", &diff_state_table(f, &change.new)?)
        })?,
    )?;
    table.set(scope, "unchanged", &count(report.unchanged))?;
    Ok(table)
}

fn extract_plan_table(scope: &impl Scope, plan: &crate::ExtractAllPlan) -> Result<Table> {
    let table = Table::new(scope, 0, 4)?;
    table.set(
        scope,
        "operation",
        extract_plan_operation_name(plan.operation),
    )?;
    table.set(scope, "archive", &plan.archive)?;
    table.set(scope, "output", &plan.output)?;
    table.set(
        scope,
        "entries",
        &array_of(scope, &plan.entries, |f, t, row| {
            t.set(f, "action", extract_plan_action_name(row.action))?;
            t.set(f, "path", &row.path)?;
            t.set(f, "pathBytesHex", &row.path_bytes_hex)?;
            t.set(f, "target", &row.target)
        })?,
    )?;
    Ok(table)
}

fn archive_plan_rows_table(scope: &impl Scope, rows: &[ArchivePlanEntry]) -> Result<Table> {
    array_of(scope, rows, |f, t, row| {
        t.set(f, "action", archive_plan_action_name(row.action))?;
        if let Some(source) = &row.source {
            t.set(f, "source", source)?;
        }
        t.set(f, "path", &row.path)?;
        t.set(f, "pathBytesHex", &row.path_bytes_hex)?;
        if let Some(value) = row.size {
            t.set(f, "size", &size(value))?;
        }
        Ok(())
    })
}

fn create_plan_table(scope: &impl Scope, plan: &crate::CreatePlan) -> Result<Table> {
    let table = Table::new(scope, 0, 5)?;
    table.set(
        scope,
        "operation",
        archive_plan_operation_name(plan.operation),
    )?;
    table.set(scope, "format", format_name(plan.format))?;
    table.set(scope, "output", &plan.output)?;
    table.set(scope, "files", &count(plan.files))?;
    table.set(
        scope,
        "entries",
        &archive_plan_rows_table(scope, &plan.entries)?,
    )?;
    Ok(table)
}

fn add_plan_table(scope: &impl Scope, plan: &crate::AddPlan) -> Result<Table> {
    let table = Table::new(scope, 0, 9)?;
    table.set(
        scope,
        "operation",
        archive_plan_operation_name(plan.operation),
    )?;
    table.set(scope, "archive", &plan.archive)?;
    table.set(scope, "output", &plan.output)?;
    table.set(scope, "format", format_name(plan.format))?;
    table.set(scope, "files", &count(plan.files))?;
    table.set(scope, "added", &count(plan.added))?;
    table.set(scope, "replaced", &count(plan.replaced))?;
    table.set(scope, "preserved", &count(plan.preserved))?;
    table.set(
        scope,
        "entries",
        &archive_plan_rows_table(scope, &plan.entries)?,
    )?;
    Ok(table)
}

// ---------------------------------------------------------------------------------------------
// The extension
// ---------------------------------------------------------------------------------------------

const EXTRACT_OPTIONS: &str =
    "{ output: string?, overwrite: string?, preservePaths: boolean?, fsync: boolean? }?";
const EXTRACT_ALL_OPTIONS: &str = "{ output: string?, overwrite: string?, fsync: boolean? }?";
const VERIFY_OPTIONS: &str = "{ readPayloads: boolean? }?";
const DIFF_OPTIONS: &str = "{ fingerprintPayloads: boolean? }?";
const CREATE_OPTIONS: &str = "{ format: string?, tes4Version: string?, ba2Kind: string?, ba2Version: string?, compress: boolean?, fsync: boolean?, followSymlinks: boolean? }?";
const ADD_OPTIONS: &str =
    "{ inputs: { string }, output: string?, fsync: boolean?, followSymlinks: boolean? }";
const ENTRIES: &str = "{ string | dream_archive_Entry } | dream_archive_Entries";
const SUMMARY: &str = "{ extracted: number, skipped: number }";
const FILES: &str = "{ files: number }";
const INFO: &str = "{ path: string, format: string, fileCount: number, namedEntryCount: number, hasUnnameableEntries: boolean, rewritable: boolean, rewriteBlocker: string?, tes4: { version: string, archiveTypes: string, archiveTypesBits: number, archiveFlags: { string }, archiveFlagsBits: number, unsupportedArchiveFlagsBits: number, nameMode: string }?, ba2: { version: string, payloadFormat: string, compressionFormat: string, strings: boolean }? }";

impl Extension for ArchivetoolExtension {
    fn id(&self) -> &'static str {
        EXTENSION_ID
    }

    fn describe(&self, d: &mut ExtensionDescriptor) -> Result<()> {
        d.requires(dream_archive::luau::EXTENSION_ID);
        describe_reports(d);
        describe_rows(d);
        describe_archive_methods(d);
        describe_module(d);
        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
fn describe_reports(d: &mut ExtensionDescriptor) {
    let mut verify = d.userdata::<VerifyReport>("dream.archivetool.VerifyReport");
    verify
        .tag(TagPolicy::Never)
        .doc("Archive index health: duplicate and unsafe paths, optional payload reads.");
    verify
        .getter("path", |r: &VerifyReport| r.0.path.clone())
        .signature("string");
    verify
        .getter("format", |r: &VerifyReport| format_name(r.0.format))
        .signature("string");
    verify
        .getter("fileCount", |r: &VerifyReport| count(r.0.file_count))
        .signature("number");
    verify
        .getter("namedEntryCount", |r: &VerifyReport| {
            count(r.0.named_entry_count)
        })
        .signature("number");
    verify
        .getter("unnameableEntries", |r: &VerifyReport| {
            count(r.0.unnameable_entries)
        })
        .signature("number");
    verify
        .getter("rewritable", |r: &VerifyReport| r.0.rewritable)
        .signature("boolean");
    verify
        .getter("rewriteBlocker", |r: &VerifyReport| {
            r.0.rewrite_blocker.clone()
        })
        .signature("string?");
    verify
        .getter("duplicateNormalizedPaths", |r: &VerifyReport| {
            Owned(Sequence(PathIssues {
                report: Rc::clone(&r.0),
                list: IssueList::Duplicates,
            }))
        })
        .signature("dream_archivetool_PathIssues");
    verify
        .getter("unsafePaths", |r: &VerifyReport| {
            Owned(Sequence(PathIssues {
                report: Rc::clone(&r.0),
                list: IssueList::Unsafe,
            }))
        })
        .signature("dream_archivetool_PathIssues");
    verify
        .getter("payloadsRead", |r: &VerifyReport| {
            r.0.payloads_read.map(count)
        })
        .signature("number?");
    verify
        .getter("warnings", |r: &VerifyReport| {
            Owned(Sequence(Warnings(Rc::clone(&r.0))))
        })
        .signature("dream_archivetool_Warnings");
    verify
        .method("toTable", |r: &VerifyReport, call: &Call| {
            verify_table(call, &r.0)
        })
        .signature("(self): { [string]: any }")
        .doc("The report as the nested table 0.2 returned (sizes and fingerprints as integers).");
    verify.metamethod("__tostring", |r: &VerifyReport| {
        format!(
            "dream.archivetool.VerifyReport({}, {} entries)",
            r.0.path, r.0.file_count
        )
    });

    let mut diff = d.userdata::<DiffReport>("dream.archivetool.DiffReport");
    diff.tag(TagPolicy::Never)
        .doc("Two archives compared by normalized path and metadata.");
    diff.getter("old", |r: &DiffReport| r.0.old.clone())
        .signature("string");
    diff.getter("new", |r: &DiffReport| r.0.new.clone())
        .signature("string");
    diff.getter("comparison", |r: &DiffReport| {
        diff_comparison_name(r.0.comparison)
    })
    .signature("string");
    diff.getter("fingerprintPayloads", |r: &DiffReport| {
        r.0.fingerprint_payloads
    })
    .signature("boolean");
    diff.getter("added", |r: &DiffReport| {
        Owned(Sequence(DiffEntries {
            report: Rc::clone(&r.0),
            side: DiffSide::Added,
        }))
    })
    .signature("dream_archivetool_DiffEntries");
    diff.getter("removed", |r: &DiffReport| {
        Owned(Sequence(DiffEntries {
            report: Rc::clone(&r.0),
            side: DiffSide::Removed,
        }))
    })
    .signature("dream_archivetool_DiffEntries");
    diff.getter("changed", |r: &DiffReport| {
        Owned(Sequence(DiffChanges(Rc::clone(&r.0))))
    })
    .signature("dream_archivetool_DiffChanges");
    diff.getter("unchanged", |r: &DiffReport| count(r.0.unchanged))
        .signature("number");
    diff.method("toTable", |r: &DiffReport, call: &Call| {
        diff_table(call, &r.0)
    })
    .signature("(self): { [string]: any }");
    diff.metamethod("__tostring", |r: &DiffReport| {
        format!(
            "dream.archivetool.DiffReport({} -> {}, +{} -{} ~{})",
            r.0.old,
            r.0.new,
            r.0.added.len(),
            r.0.removed.len(),
            r.0.changed.len()
        )
    });

    let mut extract = d.userdata::<ExtractPlan>("dream.archivetool.ExtractPlan");
    extract
        .tag(TagPolicy::Never)
        .doc("What an extraction would write, per entry.");
    extract
        .getter("operation", |p: &ExtractPlan| {
            extract_plan_operation_name(p.0.operation)
        })
        .signature("string");
    extract
        .getter("archive", |p: &ExtractPlan| p.0.archive.clone())
        .signature("string");
    extract
        .getter("output", |p: &ExtractPlan| p.0.output.clone())
        .signature("string");
    extract
        .getter("entries", |p: &ExtractPlan| {
            Owned(Sequence(ExtractPlanRows(Rc::clone(&p.0))))
        })
        .signature("dream_archivetool_ExtractPlanRows");
    extract
        .method("toTable", |p: &ExtractPlan, call: &Call| {
            extract_plan_table(call, &p.0)
        })
        .signature("(self): { [string]: any }");
    extract.metamethod("__tostring", |p: &ExtractPlan| {
        format!(
            "dream.archivetool.ExtractPlan({}, {} entries)",
            p.0.archive,
            p.0.entries.len()
        )
    });

    let mut create = d.userdata::<CreatePlan>("dream.archivetool.CreatePlan");
    create
        .tag(TagPolicy::Never)
        .doc("What creating an archive would write.");
    create
        .getter("operation", |p: &CreatePlan| {
            archive_plan_operation_name(p.0.operation)
        })
        .signature("string");
    create
        .getter("format", |p: &CreatePlan| format_name(p.0.format))
        .signature("string");
    create
        .getter("output", |p: &CreatePlan| p.0.output.clone())
        .signature("string");
    create
        .getter("files", |p: &CreatePlan| count(p.0.files))
        .signature("number");
    create
        .getter("entries", |p: &CreatePlan| {
            Owned(Sequence(ArchivePlanRows(ArchivePlan::Create(Rc::clone(
                &p.0,
            )))))
        })
        .signature("dream_archivetool_ArchivePlanRows");
    create
        .method("toTable", |p: &CreatePlan, call: &Call| {
            create_plan_table(call, &p.0)
        })
        .signature("(self): { [string]: any }");
    create.metamethod("__tostring", |p: &CreatePlan| {
        format!(
            "dream.archivetool.CreatePlan({}, {} files)",
            p.0.output, p.0.files
        )
    });

    let mut add = d.userdata::<AddPlan>("dream.archivetool.AddPlan");
    add.tag(TagPolicy::Never)
        .doc("What adding to an archive would rewrite.");
    add.getter("operation", |p: &AddPlan| {
        archive_plan_operation_name(p.0.operation)
    })
    .signature("string");
    add.getter("archive", |p: &AddPlan| p.0.archive.clone())
        .signature("string");
    add.getter("output", |p: &AddPlan| p.0.output.clone())
        .signature("string");
    add.getter("format", |p: &AddPlan| format_name(p.0.format))
        .signature("string");
    add.getter("files", |p: &AddPlan| count(p.0.files))
        .signature("number");
    add.getter("added", |p: &AddPlan| count(p.0.added))
        .signature("number");
    add.getter("replaced", |p: &AddPlan| count(p.0.replaced))
        .signature("number");
    add.getter("preserved", |p: &AddPlan| count(p.0.preserved))
        .signature("number");
    add.getter("entries", |p: &AddPlan| {
        Owned(Sequence(ArchivePlanRows(ArchivePlan::Add(Rc::clone(&p.0)))))
    })
    .signature("dream_archivetool_ArchivePlanRows");
    add.method("toTable", |p: &AddPlan, call: &Call| {
        add_plan_table(call, &p.0)
    })
    .signature("(self): { [string]: any }");
    add.metamethod("__tostring", |p: &AddPlan| {
        format!(
            "dream.archivetool.AddPlan({}, {} files)",
            p.0.archive, p.0.files
        )
    });
}

#[allow(clippy::too_many_lines)]
fn describe_rows(d: &mut ExtensionDescriptor) {
    d.sequence::<PathIssues>("dream.archivetool.PathIssues")
        .item_type("dream_archivetool_PathIssue")
        .tag(TagPolicy::Never);
    d.sequence::<Warnings>("dream.archivetool.Warnings")
        .item_type("string")
        .tag(TagPolicy::Never);
    d.sequence::<DiffEntries>("dream.archivetool.DiffEntries")
        .item_type("dream_archivetool_DiffEntry")
        .tag(TagPolicy::Never);
    d.sequence::<DiffChanges>("dream.archivetool.DiffChanges")
        .item_type("dream_archivetool_DiffChange")
        .tag(TagPolicy::Never);
    d.sequence::<ExtractPlanRows>("dream.archivetool.ExtractPlanRows")
        .item_type("dream_archivetool_ExtractPlanRow")
        .tag(TagPolicy::Never);
    d.sequence::<ArchivePlanRows>("dream.archivetool.ArchivePlanRows")
        .item_type("dream_archivetool_ArchivePlanRow")
        .tag(TagPolicy::Never);

    let mut issue = d.userdata::<PathIssue>("dream.archivetool.PathIssue");
    issue
        .tag(TagPolicy::Preferred)
        .doc("A duplicate or unsafe archive path.");
    issue
        .getter("path", |i: &PathIssue| i.row().path.clone())
        .signature("string");
    issue
        .getter("pathBytesHex", |i: &PathIssue| {
            i.row().path_bytes_hex.clone()
        })
        .signature("string");
    issue
        .getter("rawPathBytesHex", |i: &PathIssue| {
            i.row().raw_path_bytes_hex.clone()
        })
        .signature("string?");
    issue
        .getter("collidingRawPathBytesHex", |i: &PathIssue| {
            i.row().colliding_raw_path_bytes_hex.clone()
        })
        .signature("string?");

    let mut entry = d.userdata::<DiffEntry>("dream.archivetool.DiffEntry");
    entry
        .tag(TagPolicy::Preferred)
        .doc("An entry present on one side of a diff.");
    entry
        .getter("path", |e: &DiffEntry| e.row().path.clone())
        .signature("string");
    entry
        .getter("pathBytesHex", |e: &DiffEntry| {
            e.row().path_bytes_hex.clone()
        })
        .signature("string");
    entry
        .getter("size", |e: &DiffEntry| e.row().size.map(size))
        .signature("integer?");
    entry
        .getter("compressedSize", |e: &DiffEntry| {
            e.row().compressed_size.map(size)
        })
        .signature("integer?");
    entry
        .getter("payloadFingerprint", |e: &DiffEntry| {
            fingerprint(e.row().payload_fingerprint.as_deref())
        })
        .signature("integer?");

    let mut change = d.userdata::<DiffChange>("dream.archivetool.DiffChange");
    change
        .tag(TagPolicy::Preferred)
        .doc("An entry present on both sides with a difference.");
    change
        .getter("path", |c: &DiffChange| c.row().path.clone())
        .signature("string");
    change
        .getter("pathBytesHex", |c: &DiffChange| {
            c.row().path_bytes_hex.clone()
        })
        .signature("string");
    change
        .getter("old", |c: &DiffChange| {
            Owned(DiffState {
                report: Rc::clone(&c.report),
                index: c.index,
                old: true,
            })
        })
        .signature("dream_archivetool_DiffState");
    change
        .getter("new", |c: &DiffChange| {
            Owned(DiffState {
                report: Rc::clone(&c.report),
                index: c.index,
                old: false,
            })
        })
        .signature("dream_archivetool_DiffState");

    let mut state = d.userdata::<DiffState>("dream.archivetool.DiffState");
    state
        .tag(TagPolicy::Never)
        .doc("One side's metadata for a changed entry.");
    state
        .getter("size", |s: &DiffState| s.state().size.map(size))
        .signature("integer?");
    state
        .getter("compressedSize", |s: &DiffState| {
            s.state().compressed_size.map(size)
        })
        .signature("integer?");
    state
        .getter("payloadFingerprint", |s: &DiffState| {
            fingerprint(s.state().payload_fingerprint.as_deref())
        })
        .signature("integer?");

    let mut row = d.userdata::<ExtractPlanRow>("dream.archivetool.ExtractPlanRow");
    row.tag(TagPolicy::Preferred)
        .doc("One planned extraction target.");
    row.getter("action", |r: &ExtractPlanRow| {
        extract_plan_action_name(r.row().action)
    })
    .signature("string");
    row.getter("path", |r: &ExtractPlanRow| r.row().path.clone())
        .signature("string");
    row.getter("pathBytesHex", |r: &ExtractPlanRow| {
        r.row().path_bytes_hex.clone()
    })
    .signature("string");
    row.getter("target", |r: &ExtractPlanRow| r.row().target.clone())
        .signature("string");

    let mut row = d.userdata::<ArchivePlanRow>("dream.archivetool.ArchivePlanRow");
    row.tag(TagPolicy::Preferred)
        .doc("One planned archive member of a create or add plan.");
    row.getter("action", |r: &ArchivePlanRow| {
        archive_plan_action_name(r.row().action)
    })
    .signature("string");
    row.getter("source", |r: &ArchivePlanRow| r.row().source.clone())
        .signature("string?");
    row.getter("path", |r: &ArchivePlanRow| r.row().path.clone())
        .signature("string");
    row.getter("pathBytesHex", |r: &ArchivePlanRow| {
        r.row().path_bytes_hex.clone()
    })
    .signature("string");
    row.getter("size", |r: &ArchivePlanRow| r.row().size.map(size))
        .signature("integer?");
}

/// The policy operations on `dream.archive.Archive`, running against the opened handle.
#[allow(clippy::too_many_lines)]
fn describe_archive_methods(d: &mut ExtensionDescriptor) {
    let mut archive = d.augment_userdata::<Archive>(ARCHIVE_KEY);
    archive
        .method("toolInfo", |a: &Archive, call: &Call| {
            info_table(
                call,
                &crate::archive::archive_info_ref(&archive_label(a), loaded(a)),
            )
        })
        .signature(format!("(self): {INFO}"));
    archive
        .method(
            "verify",
            |a: &Archive, call: &Call, options: Option<ValueView>| {
                let options = verify_options(call, options, "archive:verify")?;
                crate::verify::verify_loaded_archive(&archive_label(a), loaded(a), options)
                    .map(|report| Owned(VerifyReport(Rc::new(report))))
                    .map_err(tool_error)
            },
        )
        .signature(format!(
            "(self, options: {VERIFY_OPTIONS}): dream_archivetool_VerifyReport"
        ));
    archive
        .method(
            "diff",
            |a: &Archive, call: &Call, other: &Archive, options: Option<ValueView>| {
                let options = diff_options(call, options, "archive:diff")?;
                crate::diff::diff_loaded_archives(
                    &archive_label(a),
                    loaded(a),
                    &archive_label(other),
                    loaded(other),
                    options,
                )
                .map(|report| Owned(DiffReport(Rc::new(report))))
                .map_err(tool_error)
            },
        )
        .signature(format!(
            "(self, other: dream_archive_Archive, options: {DIFF_OPTIONS}): dream_archivetool_DiffReport"
        ));
    archive
        .method(
            "extract",
            |a: &Archive, call: &Call, entry: ValueView, options: Option<ValueView>| {
                let entry = entry_bytes(entry, "archive:extract")?;
                let options = extract_options(call, options, "archive:extract")?;
                let summary = crate::extract::extract_entry_by_path_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &entry,
                    &options,
                )
                .map_err(tool_error)?;
                summary_table(call, &summary)
            },
        )
        .signature(format!(
            "(self, entry: string | dream_archive_Entry, options: {EXTRACT_OPTIONS}): {SUMMARY}"
        ));
    archive
        .method(
            "extractMany",
            |a: &Archive, call: &Call, entries: ValueView, options: Option<ValueView>| {
                let entries = entry_list(call, entries, "archive:extractMany")?;
                let options = extract_options(call, options, "archive:extractMany")?;
                let summary = crate::extract::extract_entries_by_path_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &entries,
                    &options,
                )
                .map_err(tool_error)?;
                summary_table(call, &summary)
            },
        )
        .signature(format!(
            "(self, entries: {ENTRIES}, options: {EXTRACT_OPTIONS}): {SUMMARY}"
        ));
    archive
        .method(
            "planExtract",
            |a: &Archive, call: &Call, entries: ValueView, options: Option<ValueView>| {
                let entries = entry_list(call, entries, "archive:planExtract")?;
                let options = extract_options(call, options, "archive:planExtract")?;
                crate::extract::plan_extract_entries_by_path_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &entries,
                    &options,
                )
                .map(|plan| Owned(ExtractPlan(Rc::new(plan))))
                .map_err(tool_error)
            },
        )
        .signature(format!(
            "(self, entries: {ENTRIES}, options: {EXTRACT_OPTIONS}): dream_archivetool_ExtractPlan"
        ));
    archive
        .method(
            "extractByPathHex",
            |a: &Archive, call: &Call, entry: &str, options: Option<ValueView>| {
                let entry = hex_entry(entry, "archive:extractByPathHex")?;
                let options = extract_options(call, options, "archive:extractByPathHex")?;
                let summary = crate::extract::extract_entry_by_path_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &entry,
                    &options,
                )
                .map_err(tool_error)?;
                summary_table(call, &summary)
            },
        )
        .signature(format!(
            "(self, pathBytesHex: string, options: {EXTRACT_OPTIONS}): {SUMMARY}"
        ));
    archive
        .method(
            "extractManyByPathHex",
            |a: &Archive, call: &Call, entries: ValueView, options: Option<ValueView>| {
                let entries = hex_entry_list(call, entries, "archive:extractManyByPathHex")?;
                let options = extract_options(call, options, "archive:extractManyByPathHex")?;
                let summary = crate::extract::extract_entries_by_path_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &entries,
                    &options,
                )
                .map_err(tool_error)?;
                summary_table(call, &summary)
            },
        )
        .signature(format!(
            "(self, pathBytesHex: {{ string }}, options: {EXTRACT_OPTIONS}): {SUMMARY}"
        ));
    archive
        .method(
            "planExtractByPathHex",
            |a: &Archive, call: &Call, entries: ValueView, options: Option<ValueView>| {
                let entries = hex_entry_list(call, entries, "archive:planExtractByPathHex")?;
                let options = extract_options(call, options, "archive:planExtractByPathHex")?;
                crate::extract::plan_extract_entries_by_path_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &entries,
                    &options,
                )
                .map(|plan| Owned(ExtractPlan(Rc::new(plan))))
                .map_err(tool_error)
            },
        )
        .signature(format!(
            "(self, pathBytesHex: {{ string }}, options: {EXTRACT_OPTIONS}): dream_archivetool_ExtractPlan"
        ));
    archive
        .method(
            "extractAll",
            |a: &Archive, call: &Call, options: Option<ValueView>| {
                let options = extract_all_options(call, options, "archive:extractAll")?;
                let summary = crate::extract::extract_all_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &options,
                )
                .map_err(tool_error)?;
                summary_table(call, &summary)
            },
        )
        .signature(format!("(self, options: {EXTRACT_ALL_OPTIONS}): {SUMMARY}"));
    archive
        .method(
            "planExtractAll",
            |a: &Archive, call: &Call, options: Option<ValueView>| {
                let options = extract_all_options(call, options, "archive:planExtractAll")?;
                crate::extract::plan_extract_all_from_loaded_archive(
                    &archive_label(a),
                    loaded(a),
                    &options,
                )
                .map(|plan| Owned(ExtractPlan(Rc::new(plan))))
                .map_err(tool_error)
            },
        )
        .signature(format!(
            "(self, options: {EXTRACT_ALL_OPTIONS}): dream_archivetool_ExtractPlan"
        ));
}

#[allow(clippy::too_many_lines)]
fn describe_module(d: &mut ExtensionDescriptor) {
    d.module(MODULE)
        .doc("Archive policy: safe extraction, plans, verification, diffs, creation, and updates by host path.")
        .function("info", |call: &Call, path: &str| {
            info_table(call, &ArchiveTool::info(path).map_err(tool_error)?)
        })
        .signature(format!("(path: string) -> {INFO}"))
        .function("verify", |call: &Call, path: &str, options: Option<ValueView>| {
            let options = verify_options(call, options, "verify")?;
            ArchiveTool::verify(path, &options)
                .map(|report| Owned(VerifyReport(Rc::new(report))))
                .map_err(tool_error)
        })
        .signature(format!("(path: string, options: {VERIFY_OPTIONS}) -> dream_archivetool_VerifyReport"))
        .function("diff", |call: &Call, old: &str, new: &str, options: Option<ValueView>| {
            let options = diff_options(call, options, "diff")?;
            ArchiveTool::diff(old, new, &options)
                .map(|report| Owned(DiffReport(Rc::new(report))))
                .map_err(tool_error)
        })
        .signature(format!("(old: string, new: string, options: {DIFF_OPTIONS}) -> dream_archivetool_DiffReport"))
        .function("extract", |call: &Call, path: &str, entry: ValueView, options: Option<ValueView>| {
            let entry = entry_bytes(entry, "extract")?;
            let options = extract_options(call, options, "extract")?;
            let summary = ArchiveTool::extract_by_path_bytes(path, &entry, &options).map_err(tool_error)?;
            summary_table(call, &summary)
        })
        .signature(format!("(path: string, entry: string | dream_archive_Entry, options: {EXTRACT_OPTIONS}) -> {SUMMARY}"))
        .function("extractHex", |call: &Call, path: &str, entry: &str, options: Option<ValueView>| {
            let entry = hex_entry(entry, "extractHex")?;
            let options = extract_options(call, options, "extractHex")?;
            let summary = ArchiveTool::extract_by_path_bytes(path, &entry, &options).map_err(tool_error)?;
            summary_table(call, &summary)
        })
        .signature(format!("(path: string, pathBytesHex: string, options: {EXTRACT_OPTIONS}) -> {SUMMARY}"))
        .doc("Same as extractByPathHex.")
        .function("extractByPathHex", |call: &Call, path: &str, entry: &str, options: Option<ValueView>| {
            let entry = hex_entry(entry, "extractByPathHex")?;
            let options = extract_options(call, options, "extractByPathHex")?;
            let summary = ArchiveTool::extract_by_path_bytes(path, &entry, &options).map_err(tool_error)?;
            summary_table(call, &summary)
        })
        .signature(format!("(path: string, pathBytesHex: string, options: {EXTRACT_OPTIONS}) -> {SUMMARY}"))
        .function("extractMany", |call: &Call, path: &str, entries: ValueView, options: Option<ValueView>| {
            let entries = entry_list(call, entries, "extractMany")?;
            let options = extract_options(call, options, "extractMany")?;
            let summary = ArchiveTool::extract_many_by_path_bytes(path, &entries, &options).map_err(tool_error)?;
            summary_table(call, &summary)
        })
        .signature(format!("(path: string, entries: {ENTRIES}, options: {EXTRACT_OPTIONS}) -> {SUMMARY}"))
        .function("extractManyByPathHex", |call: &Call, path: &str, entries: ValueView, options: Option<ValueView>| {
            let entries = hex_entry_list(call, entries, "extractManyByPathHex")?;
            let options = extract_options(call, options, "extractManyByPathHex")?;
            let summary = ArchiveTool::extract_many_by_path_bytes(path, &entries, &options).map_err(tool_error)?;
            summary_table(call, &summary)
        })
        .signature(format!("(path: string, pathBytesHex: {{ string }}, options: {EXTRACT_OPTIONS}) -> {SUMMARY}"))
        .function("planExtract", |call: &Call, path: &str, entries: ValueView, options: Option<ValueView>| {
            let entries = entry_list(call, entries, "planExtract")?;
            let options = extract_options(call, options, "planExtract")?;
            ArchiveTool::plan_extract_many_by_path_bytes(path, &entries, &options)
                .map(|plan| Owned(ExtractPlan(Rc::new(plan))))
                .map_err(tool_error)
        })
        .signature(format!("(path: string, entries: {ENTRIES}, options: {EXTRACT_OPTIONS}) -> dream_archivetool_ExtractPlan"))
        .function("planExtractByPathHex", |call: &Call, path: &str, entries: ValueView, options: Option<ValueView>| {
            let entries = hex_entry_list(call, entries, "planExtractByPathHex")?;
            let options = extract_options(call, options, "planExtractByPathHex")?;
            ArchiveTool::plan_extract_many_by_path_bytes(path, &entries, &options)
                .map(|plan| Owned(ExtractPlan(Rc::new(plan))))
                .map_err(tool_error)
        })
        .signature(format!("(path: string, pathBytesHex: {{ string }}, options: {EXTRACT_OPTIONS}) -> dream_archivetool_ExtractPlan"))
        .function("extractAll", |call: &Call, path: &str, options: Option<ValueView>| {
            let options = extract_all_options(call, options, "extractAll")?;
            let summary = ArchiveTool::extract_all(path, &options).map_err(tool_error)?;
            summary_table(call, &summary)
        })
        .signature(format!("(path: string, options: {EXTRACT_ALL_OPTIONS}) -> {SUMMARY}"))
        .function("planExtractAll", |call: &Call, path: &str, options: Option<ValueView>| {
            let options = extract_all_options(call, options, "planExtractAll")?;
            ArchiveTool::plan_extract_all(path, &options)
                .map(|plan| Owned(ExtractPlan(Rc::new(plan))))
                .map_err(tool_error)
        })
        .signature(format!("(path: string, options: {EXTRACT_ALL_OPTIONS}) -> dream_archivetool_ExtractPlan"))
        .function("create", |call: &Call, output: &str, input: &str, options: Option<ValueView>| {
            let options = create_options(call, options, "create")?;
            let files = ArchiveTool::create(output, input, &options).map_err(tool_error)?;
            files_table(call, files)
        })
        .signature(format!("(output: string, input: string, options: {CREATE_OPTIONS}) -> {FILES}"))
        .function("planCreate", |call: &Call, output: &str, input: &str, options: Option<ValueView>| {
            let options = create_options(call, options, "planCreate")?;
            ArchiveTool::plan_create(output, input, &options)
                .map(|plan| Owned(CreatePlan(Rc::new(plan))))
                .map_err(tool_error)
        })
        .signature(format!("(output: string, input: string, options: {CREATE_OPTIONS}) -> dream_archivetool_CreatePlan"))
        .function("add", |call: &Call, archive: &str, options: ValueView| {
            let options = add_options(call, options, "add")?;
            let files = ArchiveTool::add(archive, &options).map_err(tool_error)?;
            files_table(call, files)
        })
        .signature(format!("(archive: string, options: {ADD_OPTIONS}) -> {FILES}"))
        .function("planAdd", |call: &Call, archive: &str, options: ValueView| {
            let options = add_options(call, options, "planAdd")?;
            ArchiveTool::plan_add(archive, &options)
                .map(|plan| Owned(AddPlan(Rc::new(plan))))
                .map_err(tool_error)
        })
        .signature(format!("(archive: string, options: {ADD_OPTIONS}) -> dream_archivetool_AddPlan"));
}
