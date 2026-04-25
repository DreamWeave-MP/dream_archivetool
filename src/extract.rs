use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ArchiveError, Result};

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
    crate::loaded::LoadedArchive::open(path)?.read_entry_bytes(entry)
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
    let archive = crate::loaded::LoadedArchive::open(path)?;
    let mut summary = ExtractSummary {
        extracted: 0,
        skipped: 0,
    };
    archive.for_each_entry_writer(|path, writer| {
        let target = safe_target_path(&root, path)?;
        let result =
            write_target_with(&target, options.overwrite, |output| writer.write_to(output))?;
        summary.extracted += result.extracted;
        summary.skipped += result.skipped;
        Ok(())
    })?;
    Ok(summary)
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
    write_target_with(target, overwrite, |output| {
        use std::io::Write;

        output.write_all(bytes)?;
        Ok(())
    })
}

fn write_target_with(
    target: &Path,
    overwrite: OverwriteMode,
    write: impl FnOnce(&mut fs::File) -> Result<()>,
) -> Result<ExtractSummary> {
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
    let mut output = fs::File::create(target)?;
    write(&mut output)?;
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

    fn write_multi_tes3_archive(path: &Path) {
        let archive: ba2::tes3::Archive = [
            (
                ba2::tes3::ArchiveKey::from(b"textures/a.dds".as_slice()),
                ba2::tes3::File::from(b"a".as_slice()),
            ),
            (
                ba2::tes3::ArchiveKey::from(b"meshes/b.nif".as_slice()),
                ba2::tes3::File::from(b"b".as_slice()),
            ),
        ]
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

        let err = safe_target_path(Path::new("out"), "/evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));

        let err = safe_target_path(Path::new("out"), "textures/../../evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));

        let err = safe_target_path(Path::new("out"), "textures\\..\\evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));
    }

    #[test]
    fn accepts_current_directory_components() {
        let path = safe_target_path(Path::new("out"), "./textures/./example.dds").unwrap();
        assert_eq!(path, Path::new("out").join("textures/example.dds"));
    }

    #[test]
    fn extract_entry_fails_when_target_exists_by_default() {
        let dir = unique_dir("extract-entry-exists");
        let output_dir = dir.join("out");
        fs::create_dir_all(output_dir.join("textures")).unwrap();
        fs::write(output_dir.join("textures/example.dds"), b"existing").unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);

        let err = extract_entry(
            &archive_path,
            "textures/example.dds",
            &ExtractOptions {
                output: Some(output_dir),
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(err, ArchiveError::TargetExists(_)));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_entry_can_overwrite_existing_file() {
        let dir = unique_dir("extract-entry-overwrite");
        let output_dir = dir.join("out");
        fs::create_dir_all(output_dir.join("textures")).unwrap();
        fs::write(output_dir.join("textures/example.dds"), b"existing").unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);

        let summary = extract_entry(
            &archive_path,
            "textures/example.dds",
            &ExtractOptions {
                output: Some(output_dir.clone()),
                overwrite: OverwriteMode::Overwrite,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(summary.extracted, 1);
        assert_eq!(summary.skipped, 0);
        assert_eq!(
            fs::read(output_dir.join("textures/example.dds")).unwrap(),
            b"payload"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_entry_can_skip_existing_file() {
        let dir = unique_dir("extract-entry-skip");
        let output_dir = dir.join("out");
        fs::create_dir_all(output_dir.join("textures")).unwrap();
        fs::write(output_dir.join("textures/example.dds"), b"existing").unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);

        let summary = extract_entry(
            &archive_path,
            "textures/example.dds",
            &ExtractOptions {
                output: Some(output_dir.clone()),
                overwrite: OverwriteMode::Skip,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(summary.extracted, 0);
        assert_eq!(summary.skipped, 1);
        assert_eq!(
            fs::read(output_dir.join("textures/example.dds")).unwrap(),
            b"existing"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_all_writes_multiple_entries() {
        let dir = unique_dir("extract-all");
        let output_dir = dir.join("out");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_multi_tes3_archive(&archive_path);

        let summary = extract_all(
            &archive_path,
            &ExtractAllOptions {
                output: Some(output_dir.clone()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(summary.extracted, 2);
        assert_eq!(fs::read(output_dir.join("textures/a.dds")).unwrap(), b"a");
        assert_eq!(fs::read(output_dir.join("meshes/b.nif")).unwrap(), b"b");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_all_can_skip_existing_files() {
        let dir = unique_dir("extract-all-skip");
        let output_dir = dir.join("out");
        fs::create_dir_all(output_dir.join("textures")).unwrap();
        let archive_path = dir.join("test.bsa");
        write_multi_tes3_archive(&archive_path);
        fs::write(output_dir.join("textures/a.dds"), b"existing").unwrap();

        let summary = extract_all(
            &archive_path,
            &ExtractAllOptions {
                output: Some(output_dir.clone()),
                overwrite: OverwriteMode::Skip,
            },
        )
        .unwrap();

        assert_eq!(summary.extracted, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(
            fs::read(output_dir.join("textures/a.dds")).unwrap(),
            b"existing"
        );
        assert_eq!(fs::read(output_dir.join("meshes/b.nif")).unwrap(), b"b");
        fs::remove_dir_all(dir).unwrap();
    }
}
