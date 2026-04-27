use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::ValueEnum;
use dream_archive::ba2::{ArchiveVersion as Ba2ArchiveVersion, PayloadFormat};
use dream_archive::bsa::tes4::{ArchiveVersion as Tes4ArchiveVersion, NameMode};
use dream_path::ByteSlice as _;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

use crate::ArchiveFormat;
pub use crate::archive_plan::{
    AddPlan, ArchivePlanAction, ArchivePlanEntry, ArchivePlanOperation, CreatePlan,
};
use crate::paths::{
    archive_path_bytes_to_display, archive_path_bytes_to_hex, path_to_archive_bytes,
};
use crate::{ArchiveError, Result};

#[derive(Debug, Clone)]
/// Options controlling archive creation.
pub struct CreateOptions {
    /// Archive family to write.
    pub format: ArchiveFormat,
    /// TES4 BSA version used when `format` is [`ArchiveFormat::Tes4`].
    pub tes4_version: Tes4Version,
    /// BA2/Starfield BA2 kind used when `format` is [`ArchiveFormat::Ba2`].
    pub ba2_kind: Ba2ArchiveKind,
    /// BA2/Starfield BA2 version used when `format` is [`ArchiveFormat::Ba2`].
    pub ba2_version: Ba2Version,
    /// Sync file contents and parent directory after writing the archive.
    pub fsync: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            format: ArchiveFormat::Tes3,
            tes4_version: Tes4Version::Oblivion,
            ba2_kind: Ba2ArchiveKind::Gnrl,
            ba2_version: Ba2Version::Fallout4,
            fsync: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
/// Options for adding or replacing entries in an archive.
pub struct AddOptions {
    /// Files or directories to add. Directory entries are stored relative to the directory root.
    pub inputs: Vec<PathBuf>,
    /// Output archive path. The input archive is never modified in place.
    pub output: PathBuf,
    /// Sync file contents and parent directory after writing the archive.
    pub fsync: bool,
}

/// Supported TES4-family BSA versions for archive creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Tes4Version {
    /// The Elder Scrolls IV: Oblivion BSA format.
    Oblivion,
    /// Fallout 3 BSA format.
    Fallout3,
    /// The Elder Scrolls V: Skyrim BSA format.
    Skyrim,
    /// Skyrim Special Edition BSA format.
    SkyrimSe,
}

/// Supported BA2-family BA2 archive kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Ba2ArchiveKind {
    /// General-purpose BA2 archive.
    Gnrl,
    /// DirectX texture BA2 archive. Entries must use `.dds` paths.
    Dx10,
    /// GNMF model BA2 archive. Entries must use `.gnf` paths.
    Gnmf,
}

/// Supported BA2-family BA2 versions for archive creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Ba2Version {
    /// Fallout 4 BA2 version.
    Fallout4,
    /// Starfield BA2 version.
    Starfield,
    /// Fallout 4 next-generation update BA2 version.
    Fallout4NextGen,
}

/// Create a new archive from a file or directory.
///
/// Returns the number of archive entries written. Writes go through a temporary file in the output
/// directory before replacing `output`.
pub fn create_archive(output: &Path, input: &Path, options: &CreateOptions) -> Result<usize> {
    reject_unsupported_create_options(options)?;
    let input_entries = collect_input_entry_paths(input)?;
    preflight_create_paths(input_entries.keys(), options)?;
    write_entries(output, input_entries, options)
}

/// Plan archive creation without writing output.
pub fn plan_create_archive(
    output: &Path,
    input: &Path,
    options: &CreateOptions,
) -> Result<CreatePlan> {
    reject_unsupported_create_options(options)?;
    let input_entries = collect_input_entry_paths(input)?;
    preflight_create_paths(input_entries.keys(), options)?;
    let entries = input_entries
        .iter()
        .map(|(path, source)| plan_entry(ArchivePlanAction::Add, path, Some(source)))
        .collect::<Result<Vec<_>>>()?;
    Ok(CreatePlan {
        operation: ArchivePlanOperation::Create,
        format: options.format,
        output: output.display().to_string(),
        files: entries.len(),
        entries,
    })
}

fn reject_unsupported_create_options(options: &CreateOptions) -> Result<()> {
    if options.format == ArchiveFormat::Ba2 && options.ba2_kind == Ba2ArchiveKind::Gnmf {
        return Err(ArchiveError::Archive(
            "creating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive".to_string(),
        ));
    }
    Ok(())
}

/// Add or replace entries in an existing archive by writing a new archive.
///
/// Existing archive entries are preserved unless replaced by an input path. The source archive is
/// opened once and is not modified in place.
pub fn add_to_archive(archive_path: &Path, options: &AddOptions) -> Result<usize> {
    if options.inputs.is_empty() {
        return Err(ArchiveError::Archive("no input files supplied".to_string()));
    }
    reject_same_archive_output(archive_path, &options.output)?;
    let archive = crate::loaded::LoadedArchive::open(archive_path)?;
    crate::rewrite_policy::ensure_rewritable(&archive)?;
    let mut input_entries = BTreeMap::new();
    for input in &options.inputs {
        for (path, source) in collect_input_entry_paths(input)? {
            insert_input_path(&mut input_entries, &path, source)?;
        }
    }
    preflight_add_paths(input_entries.keys(), &archive)?;
    write_entries_like(&options.output, input_entries, &archive, options.fsync)
}

/// Plan archive add/update without writing output.
pub fn plan_add_to_archive(archive_path: &Path, options: &AddOptions) -> Result<AddPlan> {
    if options.inputs.is_empty() {
        return Err(ArchiveError::Archive("no input files supplied".to_string()));
    }
    reject_same_archive_output(archive_path, &options.output)?;
    let archive = crate::loaded::LoadedArchive::open(archive_path)?;
    crate::rewrite_policy::ensure_rewritable(&archive)?;
    let mut input_entries = BTreeMap::new();
    for input in &options.inputs {
        for (path, source) in collect_input_entry_paths(input)? {
            insert_input_path(&mut input_entries, &path, source)?;
        }
    }
    preflight_add_paths(input_entries.keys(), &archive)?;

    let existing = existing_archive_paths(&archive)?;
    let mut entries = Vec::new();
    let mut added = 0;
    let mut replaced = 0;
    let mut preserved = 0;
    for path in &existing {
        if input_entries.contains_key(path) {
            replaced += 1;
        } else {
            preserved += 1;
            entries.push(plan_entry(ArchivePlanAction::Preserve, path, None)?);
        }
    }
    for (path, source) in &input_entries {
        let action = if existing.contains(path) {
            ArchivePlanAction::Replace
        } else {
            added += 1;
            ArchivePlanAction::Add
        };
        entries.push(plan_entry(action, path, Some(source))?);
    }
    Ok(AddPlan {
        operation: ArchivePlanOperation::Add,
        archive: archive_path.display().to_string(),
        output: options.output.display().to_string(),
        format: archive.format(),
        files: entries.len(),
        added,
        replaced,
        preserved,
        entries,
    })
}

fn reject_same_archive_output(input: &Path, output: &Path) -> Result<()> {
    let input = comparable_path(input)?;
    let output = comparable_path(output)?;
    if input == output {
        return Err(ArchiveError::Archive(
            "output archive path must differ from input archive path".to_string(),
        ));
    }
    Ok(())
}

fn comparable_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent = parent.canonicalize()?;
    Ok(path
        .file_name()
        .map_or(parent.clone(), |name| parent.join(name)))
}

fn collect_input_entry_paths(input: &Path) -> Result<BTreeMap<Vec<u8>, PathBuf>> {
    let mut entries = BTreeMap::new();
    if input.is_file() {
        let name = input
            .file_name()
            .ok_or_else(|| ArchiveError::UnsafePath(input.display().to_string()))?;
        insert_input_path(
            &mut entries,
            &path_to_archive_bytes(Path::new(name))?,
            input.to_path_buf(),
        )?;
        return Ok(entries);
    }

    for item in WalkDir::new(input) {
        let item = item.map_err(|err| ArchiveError::Archive(err.to_string()))?;
        let path = item.path();
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(input)
            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
        insert_input_path(
            &mut entries,
            &path_to_archive_bytes(relative)?,
            path.to_path_buf(),
        )?;
    }
    Ok(entries)
}

fn insert_input_path(
    entries: &mut BTreeMap<Vec<u8>, PathBuf>,
    path: &[u8],
    source: PathBuf,
) -> Result<()> {
    if entries.insert(path.to_vec(), source).is_some() {
        return Err(ArchiveError::Archive(format!(
            "duplicate archive path after normalization: {}",
            archive_path_bytes_to_display(path)
        )));
    }
    Ok(())
}

fn write_entries(
    output: &Path,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    options: &CreateOptions,
) -> Result<usize> {
    let count = entries.len();
    with_temp_output(output, options.fsync, |file| {
        write_entries_to_file(file, entries, options)
    })?;
    Ok(count)
}

fn write_entries_like(
    output: &Path,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    archive: &crate::loaded::LoadedArchive,
    fsync: bool,
) -> Result<usize> {
    let count = count_rewritten_entries(&entries, archive)?;
    with_temp_output(output, fsync, |file| match archive {
        crate::loaded::LoadedArchive::Tes3(archive) => write_tes3_like(file, entries, archive),
        crate::loaded::LoadedArchive::Tes4(archive) => write_tes4_like(file, entries, archive),
        crate::loaded::LoadedArchive::Ba2(archive) => write_ba2_like(file, entries, archive),
    })?;
    Ok(count)
}

fn count_rewritten_entries(
    inputs: &BTreeMap<Vec<u8>, PathBuf>,
    archive: &crate::loaded::LoadedArchive,
) -> Result<usize> {
    let existing = existing_archive_paths(archive)?;
    Ok(existing
        .iter()
        .filter(|path| !inputs.contains_key(*path))
        .count()
        + inputs.len())
}

fn existing_archive_paths(archive: &crate::loaded::LoadedArchive) -> Result<BTreeSet<Vec<u8>>> {
    let mut existing = BTreeSet::new();
    match archive {
        crate::loaded::LoadedArchive::Tes3(archive) => {
            for entry in archive.entries() {
                insert_existing_archive_path(
                    &mut existing,
                    &crate::paths::normalize_archive_path_bytes(entry.path().as_bytes()),
                )?;
            }
        }
        crate::loaded::LoadedArchive::Tes4(archive) => {
            for entry in archive.entries() {
                let Some(path) = entry.path() else { continue };
                insert_existing_archive_path(
                    &mut existing,
                    &crate::paths::normalize_archive_path_bytes(path.as_bytes()),
                )?;
            }
        }
        crate::loaded::LoadedArchive::Ba2(archive) => {
            for entry in archive.entries() {
                if entry.name().is_empty() {
                    continue;
                }
                insert_existing_archive_path(
                    &mut existing,
                    &crate::paths::normalize_archive_path_bytes(entry.name().as_bytes()),
                )?;
            }
        }
    }
    Ok(existing)
}

fn plan_entry(
    action: ArchivePlanAction,
    path: &[u8],
    source: Option<&PathBuf>,
) -> Result<ArchivePlanEntry> {
    let size = source
        .map(fs::metadata)
        .transpose()?
        .map(|metadata| metadata.len());
    Ok(ArchivePlanEntry {
        action,
        source: source.map(|path| path.display().to_string()),
        path: archive_path_bytes_to_display(path),
        path_bytes_hex: archive_path_bytes_to_hex(path),
        size,
    })
}

fn write_entries_to_file(
    file: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    options: &CreateOptions,
) -> Result<()> {
    match options.format {
        ArchiveFormat::Tes3 => write_tes3(file, entries),
        ArchiveFormat::Tes4 => write_tes4(file, entries, options.tes4_version),
        ArchiveFormat::Ba2 => write_ba2(file, entries, options.ba2_kind, options.ba2_version),
    }
}

fn with_temp_output(
    output: &Path,
    fsync: bool,
    write: impl FnOnce(&mut fs::File) -> Result<()>,
) -> Result<()> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    if fsync {
        temp.as_file_mut().sync_all()?;
    }
    temp.persist(output)
        .map_err(|err| ArchiveError::Io(err.error))?;
    if fsync {
        sync_parent_dir(parent)?;
    }
    Ok(())
}

fn sync_parent_dir(parent: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn preflight_create_paths<'a>(
    paths: impl IntoIterator<Item = &'a Vec<u8>>,
    options: &CreateOptions,
) -> Result<()> {
    if options.format == ArchiveFormat::Ba2 {
        validate_ba2_paths(paths, options.ba2_kind)?;
    }
    Ok(())
}

fn preflight_add_paths<'a>(
    paths: impl IntoIterator<Item = &'a Vec<u8>>,
    archive: &crate::loaded::LoadedArchive,
) -> Result<()> {
    if let crate::loaded::LoadedArchive::Ba2(archive) = archive {
        let kind = ba2_kind_from_payload_format(archive.info().format);
        if kind == Ba2ArchiveKind::Gnmf {
            return Err(ArchiveError::Archive(
                "creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive".to_string(),
            ));
        }
        validate_ba2_paths(paths, kind)?;
    }
    Ok(())
}

fn validate_ba2_paths<'a>(
    paths: impl IntoIterator<Item = &'a Vec<u8>>,
    kind: Ba2ArchiveKind,
) -> Result<()> {
    let (extension, label) = match kind {
        Ba2ArchiveKind::Gnrl => return Ok(()),
        Ba2ArchiveKind::Dx10 => (
            b"dds".as_slice(),
            "DX10 BA2 archives can only contain .dds files",
        ),
        Ba2ArchiveKind::Gnmf => (
            b"gnf".as_slice(),
            "GNMF BA2 archives can only contain .gnf files",
        ),
    };
    for path in paths {
        if !has_extension(path, extension) {
            return Err(ArchiveError::Archive(format!(
                "{}: {}",
                label,
                archive_path_bytes_to_display(path)
            )));
        }
    }
    Ok(())
}

fn has_extension(path: &[u8], expected: &[u8]) -> bool {
    dream_path::NormalizedPath::new(path)
        .extension()
        .is_some_and(|extension| extension == expected)
}

fn insert_existing_archive_path(existing: &mut BTreeSet<Vec<u8>>, path: &[u8]) -> Result<()> {
    if !existing.insert(path.to_vec()) {
        return Err(ArchiveError::Archive(format!(
            "archive contains duplicate normalized path: {}",
            archive_path_bytes_to_display(path)
        )));
    }
    Ok(())
}

fn write_tes3(output: &mut fs::File, entries: BTreeMap<Vec<u8>, PathBuf>) -> Result<()> {
    let mut builder = dream_archive::Tes3BsaBuilder::new();
    for (path, source) in entries {
        builder.add_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_tes3_like(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    archive: &dream_archive::bsa::tes3::Archive,
) -> Result<()> {
    let mut builder = dream_archive::Tes3BsaBuilder::new();
    let source_archive = Arc::new(archive.clone());
    let mut existing_keys = BTreeSet::new();
    for (id, entry) in archive.entries_with_ids() {
        let key = crate::paths::normalize_archive_path_bytes(entry.path().as_bytes());
        insert_existing_archive_path(&mut existing_keys, &key)?;
        if !entries.contains_key(&key) {
            builder
                .add_archive_entry(&key, Arc::clone(&source_archive), id)
                .map_err(archive_error)?;
        }
    }
    for (path, source) in entries {
        builder.add_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_tes4(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    version: Tes4Version,
) -> Result<()> {
    let mut builder = dream_archive::Tes4BsaBuilder::new();
    builder.set_version(match version {
        Tes4Version::Oblivion => Tes4ArchiveVersion::v103,
        Tes4Version::Fallout3 | Tes4Version::Skyrim => Tes4ArchiveVersion::v104,
        Tes4Version::SkyrimSe => Tes4ArchiveVersion::v105,
    });
    write_tes4_builder(output, entries, builder)
}

fn write_tes4_like(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    archive: &dream_archive::bsa::tes4::Archive,
) -> Result<()> {
    let info = archive.info();
    let mut builder = dream_archive::Tes4BsaBuilder::new();
    builder.set_version(info.version);
    builder.set_archive_types(info.archive_types);
    builder.set_compressed(
        info.archive_flags
            .contains(dream_archive::bsa::tes4::ArchiveFlags::COMPRESSED),
    );
    let has_directory_strings = info
        .archive_flags
        .contains(dream_archive::bsa::tes4::ArchiveFlags::DIRECTORY_STRINGS);
    let has_file_strings = info
        .archive_flags
        .contains(dream_archive::bsa::tes4::ArchiveFlags::FILE_STRINGS);
    let has_embedded_names = info
        .archive_flags
        .contains(dream_archive::bsa::tes4::ArchiveFlags::EMBEDDED_FILE_NAMES);
    builder.set_name_mode(
        match (
            has_directory_strings && has_file_strings,
            has_embedded_names,
        ) {
            (true, true) => NameMode::StringsAndEmbedded,
            (true, false) => NameMode::Strings,
            (false, true) => NameMode::Embedded,
            (false, false) => {
                return Err(ArchiveError::Archive(
                    "TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy".to_string(),
                ));
            }
        },
    );
    let source_archive = Arc::new(archive.clone());
    let mut existing_keys = BTreeSet::new();
    for (id, entry) in archive.entries_with_ids() {
        let Some(path) = entry.path() else { continue };
        let key = crate::paths::normalize_archive_path_bytes(path.as_bytes());
        insert_existing_archive_path(&mut existing_keys, &key)?;
        if !entries.contains_key(&key) {
            builder
                .add_archive_entry(&key, Arc::clone(&source_archive), id)
                .map_err(archive_error)?;
        }
    }
    for (path, source) in entries {
        builder.add_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_tes4_builder(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    mut builder: dream_archive::Tes4BsaBuilder,
) -> Result<()> {
    for (path, source) in entries {
        builder.add_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_ba2(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    kind: Ba2ArchiveKind,
    version: Ba2Version,
) -> Result<()> {
    let format = match kind {
        Ba2ArchiveKind::Gnrl => PayloadFormat::GNRL,
        Ba2ArchiveKind::Dx10 => PayloadFormat::DX10,
        Ba2ArchiveKind::Gnmf => PayloadFormat::GNMF,
    };
    let version = match version {
        Ba2Version::Fallout4 => Ba2ArchiveVersion::v1,
        Ba2Version::Starfield => Ba2ArchiveVersion::v2,
        Ba2Version::Fallout4NextGen => Ba2ArchiveVersion::v8,
    };
    write_ba2_with_format(output, entries, format, version)
}

fn write_ba2_like(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    archive: &dream_archive::ba2::Archive,
) -> Result<()> {
    let info = archive.info();
    if info.format == PayloadFormat::DX10 {
        return write_dx10_ba2_like(output, entries, archive, info.version);
    }
    if info.format == PayloadFormat::GNMF {
        return Err(ArchiveError::Archive(
            "creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive".to_string(),
        ));
    }
    let mut builder = dream_archive::Ba2Builder::new();
    builder.set_version(info.version);
    let source_archive = Arc::new(archive.clone());
    let mut existing_keys = BTreeSet::new();
    for (id, entry) in archive.entries_with_ids() {
        if entry.name().is_empty() {
            continue;
        }
        let key = crate::paths::normalize_archive_path_bytes(entry.name().as_bytes());
        insert_existing_archive_path(&mut existing_keys, &key)?;
        if !entries.contains_key(&key) {
            builder
                .add_archive_entry(&key, Arc::clone(&source_archive), id)
                .map_err(archive_error)?;
        }
    }
    for (path, source) in entries {
        builder.add_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_ba2_with_format(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    format: PayloadFormat,
    version: Ba2ArchiveVersion,
) -> Result<()> {
    validate_ba2_paths(entries.keys(), ba2_kind_from_payload_format(format))?;
    if format == PayloadFormat::DX10 {
        return write_dx10_ba2(output, entries, version);
    }
    if format == PayloadFormat::GNMF {
        return Err(ArchiveError::Archive(
            "creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive".to_string(),
        ));
    }
    let mut builder = dream_archive::Ba2Builder::new();
    builder.set_version(version);
    for (path, source) in entries {
        builder.add_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_dx10_ba2(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    version: Ba2ArchiveVersion,
) -> Result<()> {
    let mut builder = dream_archive::Ba2Dx10Builder::new();
    builder.set_version(version);
    for (path, source) in entries {
        builder.add_dds_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn write_dx10_ba2_like(
    output: &mut fs::File,
    entries: BTreeMap<Vec<u8>, PathBuf>,
    archive: &dream_archive::ba2::Archive,
    version: Ba2ArchiveVersion,
) -> Result<()> {
    let mut builder = dream_archive::Ba2Dx10Builder::new();
    builder.set_version(version);
    let mut existing_keys = BTreeSet::new();
    for (id, entry) in archive.entries_with_ids() {
        if entry.name().is_empty() {
            continue;
        }
        let key = crate::paths::normalize_archive_path_bytes(entry.name().as_bytes());
        insert_existing_archive_path(&mut existing_keys, &key)?;
        if !entries.contains_key(&key) {
            let mut bytes = Vec::new();
            archive
                .extract_entry_by_id(id, &mut bytes)
                .map_err(archive_error)?;
            builder.add_dds_bytes(&key, bytes).map_err(archive_error)?;
        }
    }
    for (path, source) in entries {
        builder.add_dds_file(&path, source).map_err(archive_error)?;
    }
    builder.write_seek(output).map_err(archive_error)
}

fn archive_error(err: impl std::fmt::Display) -> ArchiveError {
    ArchiveError::Archive(err.to_string())
}

fn ba2_kind_from_payload_format(format: PayloadFormat) -> Ba2ArchiveKind {
    match format {
        PayloadFormat::GNRL => Ba2ArchiveKind::Gnrl,
        PayloadFormat::DX10 => Ba2ArchiveKind::Dx10,
        PayloadFormat::GNMF => Ba2ArchiveKind::Gnmf,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::ArchiveTool;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dream-archivetool-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_input_tree(dir: &Path) {
        fs::create_dir_all(dir.join("textures")).unwrap();
        fs::write(dir.join("textures/example.dds"), b"payload").unwrap();
    }

    #[test]
    fn creates_tes3_archive() {
        let dir = unique_dir("create-tes3");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        write_input_tree(&input);
        let archive = dir.join("out.bsa");

        let count = create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(count, 1);
        assert_eq!(
            ArchiveTool::read_entry(&archive, "textures/example.dds").unwrap(),
            b"payload"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creates_tes4_archive() {
        let dir = unique_dir("create-tes4");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        write_input_tree(&input);
        let archive = dir.join("out.bsa");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes4,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            ArchiveTool::read_entry(&archive, "textures/example.dds").unwrap(),
            b"payload"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creates_ba2_gnrl_archive() {
        let dir = unique_dir("create-ba2");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        write_input_tree(&input);
        let archive = dir.join("out.ba2");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            ArchiveTool::read_entry(&archive, "textures/example.dds").unwrap(),
            b"payload"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_non_dds_dx10_inputs_without_clobbering_output() {
        let dir = unique_dir("create-dx10-invalid");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("not-a-texture.txt"), b"payload").unwrap();
        let archive = dir.join("out.ba2");
        fs::write(&archive, b"existing").unwrap();

        let err = create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ba2_kind: Ba2ArchiveKind::Dx10,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("DX10"));
        assert_eq!(fs::read(&archive).unwrap(), b"existing");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn accepts_uppercase_dx10_extensions() {
        let dir = unique_dir("create-dx10-uppercase");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("TEXTURE.DDS"), b"payload").unwrap();
        let archive = dir.join("out.ba2");

        let err = create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ba2_kind: Ba2ArchiveKind::Dx10,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(!err.to_string().contains("can only contain .dds"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_gnmf_creation_without_clobbering_output() {
        let dir = unique_dir("create-gnmf-invalid");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("not-a-model.txt"), b"payload").unwrap();
        let archive = dir.join("out.ba2");
        fs::write(&archive, b"existing").unwrap();

        let err = create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ba2_kind: Ba2ArchiveKind::Gnmf,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("GNMF"));
        assert_eq!(fs::read(&archive).unwrap(), b"existing");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_gnmf_creation_before_extension_policy_matters() {
        let dir = unique_dir("create-gnmf-uppercase");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("MODEL.GNF"), b"payload").unwrap();
        let archive = dir.join("out.ba2");

        let err = create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ba2_kind: Ba2ArchiveKind::Gnmf,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("GNMF"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn create_ba2_can_write_starfield_version() {
        let dir = unique_dir("create-ba2-starfield");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("out.ba2");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ba2_version: Ba2Version::Starfield,
                ..Default::default()
            },
        )
        .unwrap();

        let archive = dream_archive::ba2::Archive::open_path(&archive).unwrap();
        assert_eq!(archive.info().version, Ba2ArchiveVersion::v2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn create_ba2_can_write_next_gen_version() {
        let dir = unique_dir("create-ba2-next-gen");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("out.ba2");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Ba2,
                ba2_version: Ba2Version::Fallout4NextGen,
                ..Default::default()
            },
        )
        .unwrap();

        let archive = dream_archive::ba2::Archive::open_path(&archive).unwrap();
        assert_eq!(archive.info().version, Ba2ArchiveVersion::v8);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_preserves_starfield_v3_ba2_version() {
        let dir = unique_dir("preserve-v3");
        let archive = dir.join("base.ba2");
        fs::create_dir_all(&dir).unwrap();
        let mut builder = dream_archive::Ba2Builder::new();
        builder.set_version(Ba2ArchiveVersion::v3);
        builder.add_bytes(b"base.txt", b"base").unwrap();
        with_temp_output(&archive, false, |file| {
            builder.write_seek(file).map_err(archive_error)
        })
        .unwrap();
        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();
        let output = dir.join("updated.ba2");

        add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![added],
                output: output.clone(),
                fsync: false,
            },
        )
        .unwrap();

        let updated = dream_archive::ba2::Archive::open_path(&output).unwrap();
        assert_eq!(updated.info().version, Ba2ArchiveVersion::v3);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_preserves_next_gen_v8_ba2_version() {
        let dir = unique_dir("preserve-v8");
        let archive = dir.join("base.ba2");
        fs::create_dir_all(&dir).unwrap();
        let mut builder = dream_archive::Ba2Builder::new();
        builder.set_version(Ba2ArchiveVersion::v8);
        builder.add_bytes(b"base.txt", b"base").unwrap();
        with_temp_output(&archive, false, |file| {
            builder.write_seek(file).map_err(archive_error)
        })
        .unwrap();
        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();
        let output = dir.join("updated.ba2");

        add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![added],
                output: output.clone(),
                fsync: false,
            },
        )
        .unwrap();

        let updated = dream_archive::ba2::Archive::open_path(&output).unwrap();
        assert_eq!(updated.info().version, Ba2ArchiveVersion::v8);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_writes_updated_archive() {
        let dir = unique_dir("add");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("base.bsa");
        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();
        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();
        let output = dir.join("updated.bsa");

        let count = add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![added],
                output: output.clone(),
                fsync: false,
            },
        )
        .unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            ArchiveTool::read_entry(&output, "base.txt").unwrap(),
            b"base"
        );
        assert_eq!(
            ArchiveTool::read_entry(&output, "added.txt").unwrap(),
            b"added"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_replaces_existing_archive_entries() {
        let dir = unique_dir("add-replace");
        let input = dir.join("input");
        fs::create_dir_all(input.join("textures")).unwrap();
        fs::write(input.join("textures/example.dds"), b"old").unwrap();
        fs::write(input.join("keep.txt"), b"keep").unwrap();
        let archive = dir.join("base.bsa");
        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();
        let replacement = dir.join("replacement");
        fs::create_dir_all(replacement.join("textures")).unwrap();
        fs::write(replacement.join("textures/example.dds"), b"new").unwrap();
        let output = dir.join("updated.bsa");

        let count = add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![replacement],
                output: output.clone(),
                fsync: false,
            },
        )
        .unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            ArchiveTool::read_entry(&output, "textures/example.dds").unwrap(),
            b"new"
        );
        assert_eq!(
            ArchiveTool::read_entry(&output, "keep.txt").unwrap(),
            b"keep"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_duplicate_paths_after_normalization() {
        let dir = unique_dir("duplicate-normalized");
        let input = dir.join("input");
        fs::create_dir_all(input.join("textures")).unwrap();
        fs::write(input.join("textures/example.dds"), b"lower").unwrap();
        fs::write(input.join("textures/EXAMPLE.DDS"), b"upper").unwrap();
        let archive = dir.join("out.bsa");

        let err = create_archive(&archive, &input, &CreateOptions::default()).unwrap_err();

        assert!(err.to_string().contains("duplicate archive path"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_rejects_duplicate_paths_across_inputs() {
        let dir = unique_dir("add-duplicate-across-inputs");
        fs::create_dir_all(&dir).unwrap();
        let base_input = dir.join("base-input");
        fs::create_dir_all(&base_input).unwrap();
        fs::write(base_input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("base.bsa");
        create_archive(
            &archive,
            &base_input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();
        let first = dir.join("first");
        let second = dir.join("second");
        fs::create_dir_all(first.join("textures")).unwrap();
        fs::create_dir_all(second.join("textures")).unwrap();
        fs::write(first.join("textures/example.dds"), b"first").unwrap();
        fs::write(second.join("textures/EXAMPLE.DDS"), b"second").unwrap();

        let err = add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![first, second],
                output: dir.join("updated.bsa"),
                fsync: false,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("duplicate archive path"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_rejects_empty_tes4_hash_only_archive() {
        let dir = unique_dir("add-hash-only");
        fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("hash-only.bsa");
        let mut builder = dream_archive::Tes4BsaBuilder::new();
        builder.set_name_mode(NameMode::HashOnly);
        builder.write_path(&archive).unwrap();
        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();

        let err = add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![added],
                output: dir.join("updated.bsa"),
                fsync: false,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("hash-only"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn input_collection_preserves_non_utf8_archive_path_bytes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let dir = unique_dir("non-utf8-path");
        fs::create_dir_all(&dir).unwrap();
        let file_name = OsString::from_vec(vec![b'F', b'O', b'O', 0xff, b'.', b'T', b'X', b'T']);
        let file_path = dir.join(file_name);
        fs::write(&file_path, b"payload").unwrap();

        let entries = collect_input_entry_paths(&dir).unwrap();

        assert!(entries.contains_key(b"foo\xff.txt".as_slice()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_rejects_output_matching_input_archive() {
        let dir = unique_dir("add-same-output");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("base.bsa");
        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();
        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();

        let err = add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![added],
                output: archive.clone(),
                fsync: false,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("must differ"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn created_non_utf8_archive_can_be_listed_extracted_and_rewritten() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let dir = unique_dir("non-utf8-roundtrip");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        let file_name = OsString::from_vec(vec![b'F', b'O', b'O', 0xff, b'.', b'T', b'X', b'T']);
        fs::write(input.join(&file_name), b"payload").unwrap();
        let archive = dir.join("out.bsa");
        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
        )
        .unwrap();

        let entries = ArchiveTool::list(&archive).unwrap();
        assert_eq!(entries.len(), 1);
        let extract_dir = dir.join("extract");
        crate::extract::extract_all(
            &archive,
            &crate::ExtractAllOptions {
                output: Some(extract_dir.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        let normalized_name =
            OsString::from_vec(vec![b'f', b'o', b'o', 0xff, b'.', b't', b'x', b't']);
        assert_eq!(
            fs::read(extract_dir.join(normalized_name)).unwrap(),
            b"payload"
        );

        let added = dir.join("added.txt");
        fs::write(&added, b"added").unwrap();
        let updated = dir.join("updated.bsa");
        add_to_archive(
            &archive,
            &AddOptions {
                inputs: vec![added],
                output: updated.clone(),
                fsync: false,
            },
        )
        .unwrap();
        assert_eq!(ArchiveTool::list(&updated).unwrap().len(), 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
