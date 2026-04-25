use std::collections::BTreeMap;
use std::fs;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ba2::prelude::*;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::ArchiveFormat;
use crate::{ArchiveError, Result};

#[derive(Debug, Clone)]
pub struct CreateOptions {
    pub format: ArchiveFormat,
    pub tes4_version: Tes4Version,
    pub fo4_kind: Fo4ArchiveKind,
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
pub struct AddOptions {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub create: CreateOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Tes4Version {
    Oblivion,
    Fallout3,
    Skyrim,
    SkyrimSe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Fo4ArchiveKind {
    Gnrl,
    Dx10,
    Gnmf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Fo4Version {
    Fallout4,
    Starfield,
    Fallout4NextGen,
}

pub fn create_archive(output: &Path, input: &Path, options: &CreateOptions) -> Result<usize> {
    let entries = collect_input_entries(input)?;
    write_entries(output, entries, options)
}

pub fn add_to_archive(archive: &Path, options: &AddOptions) -> Result<usize> {
    let archive = crate::loaded::LoadedArchive::open(archive)?;
    let create_options = archive.create_options();
    let mut entries = BTreeMap::new();
    for entry in archive.list_entries() {
        entries.insert(entry.path.clone(), archive.read_entry_bytes(&entry.path)?);
    }
    for input in &options.inputs {
        entries.extend(collect_input_entries(input)?);
    }
    write_entries(&options.output, entries, &create_options)
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
    let temp = temp_output_path(output);
    let result = write_entries_to_path(&temp, entries, options).and_then(|()| {
        fs::rename(&temp, output)?;
        Ok(count)
    });
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_entries_to_path(
    output: &Path,
    entries: BTreeMap<String, Vec<u8>>,
    options: &CreateOptions,
) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    match options.format {
        ArchiveFormat::Tes3 => write_tes3(&mut file, entries),
        ArchiveFormat::Tes4 => write_tes4(&mut file, entries, options.tes4_version),
        ArchiveFormat::Fo4 => write_fo4(&mut file, entries, options.fo4_kind, options.fo4_version),
    }
    .inspect_err(|_| {
        let _ = file.seek(SeekFrom::Start(0));
    })
}

fn temp_output_path(output: &Path) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output.file_name().unwrap_or_default().to_string_lossy();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    parent.join(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()))
}

fn validate_fo4_entries(entries: &BTreeMap<String, Vec<u8>>, kind: Fo4ArchiveKind) -> Result<()> {
    match kind {
        Fo4ArchiveKind::Gnrl => Ok(()),
        Fo4ArchiveKind::Dx10 => {
            for path in entries.keys() {
                if !path.ends_with(".dds") {
                    return Err(ArchiveError::Archive(format!(
                        "DX10 BA2 archives can only contain .dds files: {path}"
                    )));
                }
            }
            Ok(())
        }
        Fo4ArchiveKind::Gnmf => {
            for path in entries.keys() {
                if !path.ends_with(".gnf") {
                    return Err(ArchiveError::Archive(format!(
                        "GNMF BA2 archives can only contain .gnf files: {path}"
                    )));
                }
            }
            Ok(())
        }
    }
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
    let mut directories: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
    for (path, bytes) in entries {
        let (directory, file_name) = split_archive_path(&path)?;
        directories
            .entry(directory)
            .or_default()
            .push((file_name, bytes));
    }
    let archive: ba2::tes4::Archive = directories
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
        .collect();
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

fn write_fo4(
    output: &mut fs::File,
    entries: BTreeMap<String, Vec<u8>>,
    kind: Fo4ArchiveKind,
    version: Fo4Version,
) -> Result<()> {
    validate_fo4_entries(&entries, kind)?;
    let archive: ba2::fo4::Archive = entries
        .into_iter()
        .map(|(path, bytes)| {
            let chunk = ba2::fo4::Chunk::from_decompressed(bytes.into_boxed_slice());
            let file: ba2::fo4::File = [chunk].into_iter().collect();
            (ba2::fo4::ArchiveKey::from(path.into_bytes()), file)
        })
        .collect();
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
    archive
        .write(output, &options)
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
                create: CreateOptions::default(),
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
}
