use std::collections::{BTreeMap, btree_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use dream_archive::ba2::{ArchiveVersion as Ba2ArchiveVersion, PayloadFormat};
use dream_archive::bsa::tes4::{ArchiveVersion as Tes4ArchiveVersion, NameMode};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

use crate::ArchiveFormat;
use crate::paths::{normalize_archive_path, path_to_archive_string};
use crate::{ArchiveError, Result};

#[derive(Debug, Clone)]
/// Options controlling archive creation.
pub struct CreateOptions {
    /// Archive family to write.
    pub format: ArchiveFormat,
    /// TES4 BSA version used when `format` is [`ArchiveFormat::Tes4`].
    pub tes4_version: Tes4Version,
    /// FO4/Starfield BA2 kind used when `format` is [`ArchiveFormat::Fo4`].
    pub fo4_kind: Fo4ArchiveKind,
    /// FO4/Starfield BA2 version used when `format` is [`ArchiveFormat::Fo4`].
    pub fo4_version: Fo4Version,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            format: ArchiveFormat::Tes3,
            tes4_version: Tes4Version::Oblivion,
            fo4_kind: Fo4ArchiveKind::Gnrl,
            fo4_version: Fo4Version::Fallout4,
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

/// Supported FO4-family BA2 archive kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Fo4ArchiveKind {
    /// General-purpose BA2 archive.
    Gnrl,
    /// DirectX texture BA2 archive. Entries must use `.dds` paths.
    Dx10,
    /// GNMF model BA2 archive. Entries must use `.gnf` paths.
    Gnmf,
}

/// Supported FO4-family BA2 versions for archive creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Fo4Version {
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
    let entries = collect_input_entries(input)?;
    write_entries(output, entries, options)
}

fn reject_unsupported_create_options(options: &CreateOptions) -> Result<()> {
    if options.format == ArchiveFormat::Fo4 && options.fo4_kind == Fo4ArchiveKind::Gnmf {
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
pub fn add_to_archive(archive: &Path, options: &AddOptions) -> Result<usize> {
    let archive = crate::loaded::LoadedArchive::open(archive)?;
    if archive.has_unnameable_entries() {
        return Err(ArchiveError::Archive(
            "archive contains entries without recoverable paths; refusing to rewrite it lossy"
                .to_string(),
        ));
    }
    let mut entries = BTreeMap::new();
    for input in &options.inputs {
        entries.extend(collect_input_entries(input)?);
    }
    for entry in archive.list_entries() {
        let key = normalize_archive_path(&entry.path);
        if let Entry::Vacant(entry_slot) = entries.entry(key) {
            entry_slot.insert(archive.read_entry_bytes(&entry.path)?);
        }
    }
    write_entries_like(&options.output, entries, &archive)
}

fn collect_input_entries(input: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut entries = BTreeMap::new();
    if input.is_file() {
        let name = input
            .file_name()
            .ok_or_else(|| ArchiveError::UnsafePath(input.display().to_string()))?;
        insert_entry(
            &mut entries,
            &path_to_archive_string(Path::new(name))?,
            fs::read(input)?,
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
        insert_entry(
            &mut entries,
            &path_to_archive_string(relative)?,
            fs::read(path)?,
        )?;
    }
    Ok(entries)
}

fn insert_entry(entries: &mut BTreeMap<String, Vec<u8>>, path: &str, bytes: Vec<u8>) -> Result<()> {
    if entries.insert(path.to_string(), bytes).is_some() {
        return Err(ArchiveError::Archive(format!(
            "duplicate archive path after normalization: {path}"
        )));
    }
    Ok(())
}

fn write_entries(
    output: &Path,
    entries: BTreeMap<String, Vec<u8>>,
    options: &CreateOptions,
) -> Result<usize> {
    let count = entries.len();
    with_temp_output(output, |file| write_entries_to_file(file, entries, options))?;
    Ok(count)
}

fn write_entries_like(
    output: &Path,
    entries: BTreeMap<String, Vec<u8>>,
    archive: &crate::loaded::LoadedArchive,
) -> Result<usize> {
    let count = entries.len();
    with_temp_output(output, |file| match archive {
        crate::loaded::LoadedArchive::Tes3(_) => write_tes3(file, entries),
        crate::loaded::LoadedArchive::Tes4(archive) => write_tes4_like(file, entries, archive),
        crate::loaded::LoadedArchive::Fo4(archive) => write_fo4_like(file, entries, archive),
    })?;
    Ok(count)
}

fn write_entries_to_file(
    file: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    options: &CreateOptions,
) -> Result<()> {
    match options.format {
        ArchiveFormat::Tes3 => write_tes3(file, entries),
        ArchiveFormat::Tes4 => write_tes4(file, entries, options.tes4_version),
        ArchiveFormat::Fo4 => write_fo4(file, entries, options.fo4_kind, options.fo4_version),
    }
}

fn with_temp_output(output: &Path, write: impl FnOnce(&mut fs::File) -> Result<()>) -> Result<()> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    temp.as_file_mut().sync_all()?;
    temp.persist(output)
        .map_err(|err| ArchiveError::Io(err.error))?;
    sync_parent_dir(parent)?;
    Ok(())
}

fn sync_parent_dir(parent: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn validate_fo4_entries(entries: &BTreeMap<String, Vec<u8>>, kind: Fo4ArchiveKind) -> Result<()> {
    match kind {
        Fo4ArchiveKind::Gnrl => Ok(()),
        Fo4ArchiveKind::Dx10 => {
            for path in entries.keys() {
                if !has_extension(path, "dds") {
                    return Err(ArchiveError::Archive(format!(
                        "DX10 BA2 archives can only contain .dds files: {path}"
                    )));
                }
            }
            Ok(())
        }
        Fo4ArchiveKind::Gnmf => {
            for path in entries.keys() {
                if !has_extension(path, "gnf") {
                    return Err(ArchiveError::Archive(format!(
                        "GNMF BA2 archives can only contain .gnf files: {path}"
                    )));
                }
            }
            Ok(())
        }
    }
}

fn has_extension(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn write_tes3(output: &mut fs::File, entries: BTreeMap<String, Vec<u8>>) -> Result<()> {
    let mut builder = dream_archive::Tes3BsaBuilder::new();
    for (path, bytes) in entries {
        builder
            .add_bytes(path.as_bytes(), bytes)
            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
    }
    builder
        .write_to(output)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn write_tes4(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
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
    entries: BTreeMap<String, Vec<u8>>,
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
            (false, false) => NameMode::HashOnly,
        },
    );
    write_tes4_builder(output, entries, builder)
}

fn write_tes4_builder(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    mut builder: dream_archive::Tes4BsaBuilder,
) -> Result<()> {
    for (path, bytes) in entries {
        builder
            .add_bytes(path.as_bytes(), bytes)
            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
    }
    builder
        .write_to(output)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn write_fo4(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    kind: Fo4ArchiveKind,
    version: Fo4Version,
) -> Result<()> {
    let format = match kind {
        Fo4ArchiveKind::Gnrl => PayloadFormat::GNRL,
        Fo4ArchiveKind::Dx10 => PayloadFormat::DX10,
        Fo4ArchiveKind::Gnmf => PayloadFormat::GNMF,
    };
    let version = match version {
        Fo4Version::Fallout4 => Ba2ArchiveVersion::v1,
        Fo4Version::Starfield => Ba2ArchiveVersion::v2,
        Fo4Version::Fallout4NextGen => Ba2ArchiveVersion::v7,
    };
    write_fo4_with_format(output, entries, format, version)
}

fn write_fo4_like(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    archive: &dream_archive::ba2::Archive,
) -> Result<()> {
    let info = archive.info();
    write_fo4_with_format(output, entries, info.format, info.version)
}

fn write_fo4_with_format(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    format: PayloadFormat,
    version: Ba2ArchiveVersion,
) -> Result<()> {
    validate_fo4_entries(&entries, fo4_kind_from_payload_format(format))?;
    if format == PayloadFormat::DX10 {
        return write_dx10_fo4(output, entries, version);
    }
    if format == PayloadFormat::GNMF {
        return Err(ArchiveError::Archive(
            "creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive".to_string(),
        ));
    }
    let mut builder = dream_archive::Ba2Builder::new();
    builder.set_version(version);
    for (path, bytes) in entries {
        builder
            .add_bytes(path.as_bytes(), bytes)
            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
    }
    builder
        .write_to(output)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn write_dx10_fo4(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    version: Ba2ArchiveVersion,
) -> Result<()> {
    let mut builder = dream_archive::Ba2Dx10Builder::new();
    builder.set_version(version);
    for (path, bytes) in entries {
        builder
            .add_dds_bytes(path.as_bytes(), bytes)
            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
    }
    builder
        .write_to(output)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn fo4_kind_from_payload_format(format: PayloadFormat) -> Fo4ArchiveKind {
    match format {
        PayloadFormat::GNRL => Fo4ArchiveKind::Gnrl,
        PayloadFormat::DX10 => Fo4ArchiveKind::Dx10,
        PayloadFormat::GNMF => Fo4ArchiveKind::Gnmf,
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
    fn creates_fo4_gnrl_archive() {
        let dir = unique_dir("create-fo4");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        write_input_tree(&input);
        let archive = dir.join("out.ba2");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Fo4,
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
                format: ArchiveFormat::Fo4,
                fo4_kind: Fo4ArchiveKind::Dx10,
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
                format: ArchiveFormat::Fo4,
                fo4_kind: Fo4ArchiveKind::Dx10,
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
                format: ArchiveFormat::Fo4,
                fo4_kind: Fo4ArchiveKind::Gnmf,
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
                format: ArchiveFormat::Fo4,
                fo4_kind: Fo4ArchiveKind::Gnmf,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("GNMF"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn create_fo4_can_write_starfield_version() {
        let dir = unique_dir("create-fo4-starfield");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("out.ba2");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Fo4,
                fo4_version: Fo4Version::Starfield,
                ..Default::default()
            },
        )
        .unwrap();

        let archive = dream_archive::ba2::Archive::open_path(&archive).unwrap();
        assert_eq!(archive.info().version, Ba2ArchiveVersion::v2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn create_fo4_can_write_next_gen_version() {
        let dir = unique_dir("create-fo4-next-gen");
        let input = dir.join("input");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("base.txt"), b"base").unwrap();
        let archive = dir.join("out.ba2");

        create_archive(
            &archive,
            &input,
            &CreateOptions {
                format: ArchiveFormat::Fo4,
                fo4_version: Fo4Version::Fallout4NextGen,
                ..Default::default()
            },
        )
        .unwrap();

        let archive = dream_archive::ba2::Archive::open_path(&archive).unwrap();
        assert_eq!(archive.info().version, Ba2ArchiveVersion::v7);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_preserves_starfield_v3_ba2_version() {
        let dir = unique_dir("preserve-v3");
        let archive = dir.join("base.ba2");
        fs::create_dir_all(&dir).unwrap();
        let entries = BTreeMap::from([("base.txt".to_string(), b"base".to_vec())]);
        with_temp_output(&archive, |file| {
            write_fo4_with_format(file, entries, PayloadFormat::GNRL, Ba2ArchiveVersion::v3)
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
        let entries = BTreeMap::from([("base.txt".to_string(), b"base".to_vec())]);
        with_temp_output(&archive, |file| {
            write_fo4_with_format(file, entries, PayloadFormat::GNRL, Ba2ArchiveVersion::v8)
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
}
