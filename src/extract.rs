use std::fs;
use std::path::{Component, Path, PathBuf};

use ba2::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{ArchiveError, ArchiveFormat, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverwriteMode {
    #[default]
    Fail,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub output: Option<PathBuf>,
    pub overwrite: OverwriteMode,
    pub preserve_paths: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            output: None,
            overwrite: OverwriteMode::Fail,
            preserve_paths: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtractAllOptions {
    pub output: Option<PathBuf>,
    pub overwrite: OverwriteMode,
}

impl Default for ExtractAllOptions {
    fn default() -> Self {
        Self {
            output: None,
            overwrite: OverwriteMode::Fail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractSummary {
    pub extracted: usize,
    pub skipped: usize,
}

pub fn read_entry_bytes(path: &Path, entry: &str) -> Result<Vec<u8>> {
    match crate::format::guess_format(path)? {
        ArchiveFormat::Tes3 => read_tes3_entry(path, entry),
        ArchiveFormat::Tes4 => read_tes4_entry(path, entry),
        ArchiveFormat::Fo4 => read_fo4_entry(path, entry),
    }
}

pub fn extract_entry(path: &Path, entry: &str, options: &ExtractOptions) -> Result<ExtractSummary> {
    let bytes = read_entry_bytes(path, entry)?;
    let root = options.output.clone().unwrap_or_else(|| PathBuf::from("."));
    let target = if options.preserve_paths {
        safe_target_path(&root, entry)?
    } else {
        let file_name = Path::new(entry)
            .file_name()
            .ok_or_else(|| ArchiveError::UnsafePath(entry.to_string()))?;
        root.join(file_name)
    };
    write_target(&target, &bytes, options.overwrite)
}

pub fn extract_all(path: &Path, options: &ExtractAllOptions) -> Result<ExtractSummary> {
    let root = options.output.clone().unwrap_or_else(|| PathBuf::from("."));
    let mut summary = ExtractSummary {
        extracted: 0,
        skipped: 0,
    };
    for entry in crate::entry::list_entries(path)? {
        let bytes = read_entry_bytes(path, &entry.path)?;
        let target = safe_target_path(&root, &entry.path)?;
        let result = write_target(&target, &bytes, options.overwrite)?;
        summary.extracted += result.extracted;
        summary.skipped += result.skipped;
    }
    Ok(summary)
}

fn read_tes3_entry(path: &Path, entry: &str) -> Result<Vec<u8>> {
    let archive =
        ba2::tes3::Archive::read(path).map_err(|err| ArchiveError::Archive(err.to_string()))?;
    for (key, file) in &archive {
        if archive_path_eq(key.name(), entry) {
            let mut bytes = Vec::new();
            file.write(&mut bytes)
                .map_err(|err| ArchiveError::Archive(err.to_string()))?;
            return Ok(bytes);
        }
    }
    Err(ArchiveError::EntryNotFound(entry.to_string()))
}

fn read_tes4_entry(path: &Path, entry: &str) -> Result<Vec<u8>> {
    let (archive, archive_options) =
        ba2::tes4::Archive::read(path).map_err(|err| ArchiveError::Archive(err.to_string()))?;
    let file_options = ba2::tes4::FileCompressionOptions::from(&archive_options);
    for (directory_key, directory) in &archive {
        for (file_key, file) in directory {
            let candidate = format!("{}/{}", directory_key.name(), file_key.name());
            if archive_path_eq(&candidate, entry) {
                let mut bytes = Vec::new();
                file.write(&mut bytes, &file_options)
                    .map_err(|err| ArchiveError::Archive(err.to_string()))?;
                return Ok(bytes);
            }
        }
    }
    Err(ArchiveError::EntryNotFound(entry.to_string()))
}

fn read_fo4_entry(path: &Path, entry: &str) -> Result<Vec<u8>> {
    let (archive, archive_options) =
        ba2::fo4::Archive::read(path).map_err(|err| ArchiveError::Archive(err.to_string()))?;
    let file_options = ba2::fo4::FileWriteOptions::from(&archive_options);
    for (key, file) in &archive {
        if archive_path_eq(key.name(), entry) {
            let mut bytes = Vec::new();
            file.write(&mut bytes, &file_options)
                .map_err(|err| ArchiveError::Archive(err.to_string()))?;
            return Ok(bytes);
        }
    }
    Err(ArchiveError::EntryNotFound(entry.to_string()))
}

fn archive_path_eq(left: &(impl ToString + ?Sized), right: &str) -> bool {
    normalize_archive_path(&left.to_string()).eq_ignore_ascii_case(&normalize_archive_path(right))
}

fn normalize_archive_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn safe_target_path(root: &Path, archive_path: &str) -> Result<PathBuf> {
    let normalized = normalize_archive_path(archive_path);
    let path = Path::new(&normalized);
    if path.is_absolute() {
        return Err(ArchiveError::UnsafePath(archive_path.to_string()));
    }

    let mut target = PathBuf::from(root);
    for component in path.components() {
        match component {
            Component::Normal(part) => target.push(part),
            Component::CurDir => {}
            _ => return Err(ArchiveError::UnsafePath(archive_path.to_string())),
        }
    }
    Ok(target)
}

fn write_target(target: &Path, bytes: &[u8], overwrite: OverwriteMode) -> Result<ExtractSummary> {
    if target.exists() {
        match overwrite {
            OverwriteMode::Fail => {
                return Err(ArchiveError::TargetExists(target.display().to_string()));
            }
            OverwriteMode::Skip => {
                return Ok(ExtractSummary {
                    extracted: 0,
                    skipped: 1,
                });
            }
            OverwriteMode::Overwrite => {}
        }
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, bytes)?;
    Ok(ExtractSummary {
        extracted: 1,
        skipped: 0,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rome-archivetool-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_tes3_archive(path: &Path) {
        let archive: ba2::tes3::Archive = [(
            ba2::tes3::ArchiveKey::from(b"textures/example.dds".as_slice()),
            ba2::tes3::File::from(b"payload".as_slice()),
        )]
        .into_iter()
        .collect();
        let mut output = fs::File::create(path).unwrap();
        archive.write(&mut output).unwrap();
    }

    #[test]
    fn reads_entry_bytes_case_insensitively() {
        let dir = unique_dir("read-entry");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);

        let bytes = read_entry_bytes(&archive_path, "TEXTURES/EXAMPLE.DDS").unwrap();

        assert_eq!(bytes, b"payload");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extracts_entry_to_safe_path() {
        let dir = unique_dir("extract-entry");
        let output_dir = dir.join("out");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);

        let summary = extract_entry(
            &archive_path,
            "textures/example.dds",
            &ExtractOptions {
                output: Some(output_dir.clone()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(summary.extracted, 1);
        assert_eq!(
            fs::read(output_dir.join("textures/example.dds")).unwrap(),
            b"payload"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_traversal_paths() {
        let err = safe_target_path(Path::new("out"), "../evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));
    }
}
