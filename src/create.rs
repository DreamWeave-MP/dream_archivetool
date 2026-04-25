use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ba2::prelude::*;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

use crate::ArchiveFormat;
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
    let entries = collect_input_entries(input)?;
    write_entries(output, entries, options)
}

/// Add or replace entries in an existing archive by writing a new archive.
///
/// Existing archive entries are preserved unless replaced by an input path. The source archive is
/// opened once and is not modified in place.
pub fn add_to_archive(archive: &Path, options: &AddOptions) -> Result<usize> {
    let archive = crate::loaded::LoadedArchive::open(archive)?;
    let mut entries = BTreeMap::new();
    for input in &options.inputs {
        entries.extend(collect_input_entries(input)?);
    }
    archive.for_each_entry_writer(|path, writer| {
        if !entries.contains_key(path) {
            let mut bytes = Vec::new();
            writer.write_to(&mut bytes)?;
            entries.insert(path.to_string(), bytes);
        }
        Ok(())
    })?;
    write_entries_like(&options.output, entries, &archive)
}

fn collect_input_entries(input: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut entries = BTreeMap::new();
    if input.is_file() {
        let name = input
            .file_name()
            .ok_or_else(|| ArchiveError::UnsafePath(input.display().to_string()))?;
        entries.insert(path_to_archive_string(Path::new(name))?, fs::read(input)?);
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
        entries.insert(path_to_archive_string(relative)?, fs::read(path)?);
    }
    Ok(entries)
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
        crate::loaded::LoadedArchive::Tes4(_, options) => {
            write_tes4_with_options(file, entries, options)
        }
        crate::loaded::LoadedArchive::Fo4(_, options) => {
            write_fo4_with_options(file, entries, options)
        }
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
    let archive: ba2::tes3::Archive = entries
        .into_iter()
        .map(|(path, bytes)| {
            (
                ba2::tes3::ArchiveKey::from(path.into_bytes()),
                ba2::tes3::File::from(bytes.into_boxed_slice()),
            )
        })
        .collect();
    archive
        .write(output)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn write_tes4(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    version: Tes4Version,
) -> Result<()> {
    let archive = make_tes4_archive(entries)?;
    let options = ba2::tes4::ArchiveOptions::builder()
        .version(match version {
            Tes4Version::Oblivion => ba2::tes4::Version::TES4,
            Tes4Version::Fallout3 => ba2::tes4::Version::FO3,
            Tes4Version::Skyrim => ba2::tes4::Version::TES5,
            Tes4Version::SkyrimSe => ba2::tes4::Version::SSE,
        })
        .types(ba2::tes4::ArchiveTypes::MISC)
        .build();
    archive
        .write(output, &options)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn write_tes4_with_options(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    options: &ba2::tes4::ArchiveOptions,
) -> Result<()> {
    make_tes4_archive(entries)?
        .write(output, options)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn make_tes4_archive(entries: BTreeMap<String, Vec<u8>>) -> Result<ba2::tes4::Archive<'static>> {
    let mut directories: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
    for (path, bytes) in entries {
        let (directory, file_name) = split_archive_path(&path)?;
        directories
            .entry(directory)
            .or_default()
            .push((file_name, bytes));
    }
    Ok(directories
        .into_iter()
        .map(|(directory_path, files)| {
            let directory: ba2::tes4::Directory = files
                .into_iter()
                .map(|(path, bytes)| {
                    (
                        ba2::tes4::DirectoryKey::from(path.into_bytes()),
                        ba2::tes4::File::from_decompressed(bytes.into_boxed_slice()),
                    )
                })
                .collect();
            (
                ba2::tes4::ArchiveKey::from(directory_path.into_bytes()),
                directory,
            )
        })
        .collect())
}

fn write_fo4(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    kind: Fo4ArchiveKind,
    version: Fo4Version,
) -> Result<()> {
    let format = match kind {
        Fo4ArchiveKind::Gnrl => ba2::fo4::Format::GNRL,
        Fo4ArchiveKind::Dx10 => ba2::fo4::Format::DX10,
        Fo4ArchiveKind::Gnmf => ba2::fo4::Format::GNMF,
    };
    let version = match version {
        Fo4Version::Fallout4 => ba2::fo4::Version::v1,
        Fo4Version::Starfield => ba2::fo4::Version::v2,
        Fo4Version::Fallout4NextGen => ba2::fo4::Version::v7,
    };
    let options = ba2::fo4::ArchiveOptions::builder()
        .format(format)
        .version(version)
        .strings(true)
        .build();
    write_fo4_with_options(output, entries, &options)
}

fn write_fo4_with_options(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    options: &ba2::fo4::ArchiveOptions,
) -> Result<()> {
    validate_fo4_entries(
        &entries,
        match options.format() {
            ba2::fo4::Format::GNRL => Fo4ArchiveKind::Gnrl,
            ba2::fo4::Format::DX10 => Fo4ArchiveKind::Dx10,
            ba2::fo4::Format::GNMF => Fo4ArchiveKind::Gnmf,
        },
    )?;
    let archive: ba2::fo4::Archive = entries
        .into_iter()
        .map(|(path, bytes)| {
            let chunk = ba2::fo4::Chunk::from_decompressed(bytes.into_boxed_slice());
            let file: ba2::fo4::File = [chunk].into_iter().collect();
            (ba2::fo4::ArchiveKey::from(path.into_bytes()), file)
        })
        .collect();
    archive
        .write(output, options)
        .map_err(|err| ArchiveError::Archive(err.to_string()))
}

fn path_to_archive_string(path: &Path) -> Result<String> {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.is_empty() || value.starts_with('/') || value.split('/').any(|part| part == "..") {
        return Err(ArchiveError::UnsafePath(value));
    }
    Ok(value)
}

fn split_archive_path(path: &str) -> Result<(String, String)> {
    let Some((directory, file_name)) = path.rsplit_once('/') else {
        return Ok((String::new(), path.to_string()));
    };
    if file_name.is_empty() {
        return Err(ArchiveError::UnsafePath(path.to_string()));
    }
    Ok((directory.to_string(), file_name.to_string()))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::ArchiveTool;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rome-archivetool-{name}-{}",
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
    fn rejects_non_gnf_gnmf_inputs_without_clobbering_output() {
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
    fn accepts_uppercase_gnmf_extensions() {
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

        assert!(!err.to_string().contains("can only contain .gnf"));
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

        let (_, options) = ba2::fo4::Archive::read(archive.as_path()).unwrap();
        assert_eq!(options.version(), ba2::fo4::Version::v2);
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

        let (_, options) = ba2::fo4::Archive::read(archive.as_path()).unwrap();
        assert_eq!(options.version(), ba2::fo4::Version::v7);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_preserves_starfield_v3_ba2_version() {
        let dir = unique_dir("preserve-v3");
        let archive = dir.join("base.ba2");
        fs::create_dir_all(&dir).unwrap();
        let entries = BTreeMap::from([("base.txt".to_string(), b"base".to_vec())]);
        let options = ba2::fo4::ArchiveOptions::builder()
            .format(ba2::fo4::Format::GNRL)
            .version(ba2::fo4::Version::v3)
            .strings(true)
            .build();
        with_temp_output(&archive, |file| {
            write_fo4_with_options(file, entries, &options)
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

        let (_, updated_options) = ba2::fo4::Archive::read(output.as_path()).unwrap();
        assert_eq!(updated_options.version(), ba2::fo4::Version::v3);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn add_preserves_next_gen_v8_ba2_version() {
        let dir = unique_dir("preserve-v8");
        let archive = dir.join("base.ba2");
        fs::create_dir_all(&dir).unwrap();
        let entries = BTreeMap::from([("base.txt".to_string(), b"base".to_vec())]);
        let options = ba2::fo4::ArchiveOptions::builder()
            .format(ba2::fo4::Format::GNRL)
            .version(ba2::fo4::Version::v8)
            .strings(true)
            .build();
        with_temp_output(&archive, |file| {
            write_fo4_with_options(file, entries, &options)
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

        let (_, updated_options) = ba2::fo4::Archive::read(output.as_path()).unwrap();
        assert_eq!(updated_options.version(), ba2::fo4::Version::v8);
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
}
