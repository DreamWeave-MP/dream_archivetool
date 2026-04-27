use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::paths::{
    flat_target_path_normalized, normalize_archive_path_bytes, safe_target_path_normalized,
};
use crate::{ArchiveError, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Policy for handling extraction targets that already exist.
pub enum OverwriteMode {
    /// Fail if the target path already exists.
    #[default]
    Fail,
    /// Replace existing files.
    Overwrite,
    /// Leave existing files untouched and count them as skipped.
    Skip,
}

#[derive(Debug, Clone)]
/// Options for extracting one archive entry.
pub struct ExtractOptions {
    /// Output directory. Defaults to the current working directory.
    pub output: Option<PathBuf>,
    /// Existing-file handling policy.
    pub overwrite: OverwriteMode,
    /// Preserve archive directories. When false, only the entry basename is written.
    pub preserve_paths: bool,
    /// Sync file contents and parent directory after writing extracted files.
    pub fsync: bool,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            output: None,
            overwrite: OverwriteMode::Fail,
            preserve_paths: true,
            fsync: false,
        }
    }
}

#[derive(Debug, Clone)]
/// Options for extracting every archive entry.
pub struct ExtractAllOptions {
    /// Output directory. Defaults to the current working directory.
    pub output: Option<PathBuf>,
    /// Existing-file handling policy.
    pub overwrite: OverwriteMode,
    /// Sync file contents and parent directory after writing extracted files.
    pub fsync: bool,
}

impl Default for ExtractAllOptions {
    fn default() -> Self {
        Self {
            output: None,
            overwrite: OverwriteMode::Fail,
            fsync: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Summary returned by extraction operations.
pub struct ExtractSummary {
    /// Number of files written.
    pub extracted: usize,
    /// Number of existing files left untouched because overwrite mode was `Skip`.
    pub skipped: usize,
}

/// Read a single archive entry into memory.
pub fn read_entry_bytes(path: &Path, entry: &str) -> Result<Vec<u8>> {
    crate::loaded::LoadedArchive::open(path)?.read_entry_bytes(entry)
}

/// Extract a single archive entry into a writer.
pub fn extract_entry_to_writer(
    path: &Path,
    entry: &str,
    out: &mut dyn std::io::Write,
) -> Result<u64> {
    crate::loaded::LoadedArchive::open(path)?.extract_entry_to_writer(entry, out)
}

/// Extract a single archive entry to disk.
pub fn extract_entry(path: &Path, entry: &str, options: &ExtractOptions) -> Result<ExtractSummary> {
    let root = options.output.clone().unwrap_or_else(|| PathBuf::from("."));
    let archive = crate::loaded::LoadedArchive::open(path)?;
    let archive_path = normalize_archive_path_bytes(entry.as_bytes());
    let target = if options.preserve_paths {
        safe_target_path_normalized(&root, &archive_path)?
    } else {
        flat_target_path_normalized(&root, &archive_path)?
    };
    write_target_with(&target, options.overwrite, options.fsync, |output| {
        archive
            .extract_normalized_entry_path_to_writer(&archive_path, output)
            .map(|_| ())
    })
}

/// Extract every archive entry to disk.
///
/// In skip-existing mode, target existence is checked before entry bytes are decoded.
pub fn extract_all(path: &Path, options: &ExtractAllOptions) -> Result<ExtractSummary> {
    let root = options.output.clone().unwrap_or_else(|| PathBuf::from("."));
    let archive = crate::loaded::LoadedArchive::open(path)?;
    let mut summary = ExtractSummary {
        extracted: 0,
        skipped: 0,
    };
    let entries = archive.list_loaded_entries()?;
    if entries.len() != archive.file_count() {
        return Err(ArchiveError::Archive(
            "archive contains entries without recoverable paths; refusing to extract it lossy"
                .to_string(),
        ));
    }
    let targets = planned_extract_targets(&root, entries, options.overwrite)?;
    for target in targets {
        let result = write_target_with(&target.path, options.overwrite, options.fsync, |output| {
            archive
                .extract_normalized_entry_path_to_writer(&target.archive_path, output)
                .map(|_| ())
        })?;
        summary.extracted += result.extracted;
        summary.skipped += result.skipped;
    }
    Ok(summary)
}

#[derive(Debug)]
struct PlannedExtractTarget {
    archive_path: Vec<u8>,
    path: PathBuf,
}

fn planned_extract_targets(
    root: &Path,
    entries: Vec<crate::loaded::LoadedEntry>,
    overwrite: OverwriteMode,
) -> Result<Vec<PlannedExtractTarget>> {
    let mut targets = Vec::with_capacity(entries.len());
    let mut seen = BTreeSet::new();
    for entry in entries {
        let path = safe_target_path_normalized(root, &entry.path)?;
        if !seen.insert(path.clone()) {
            return Err(ArchiveError::Archive(format!(
                "duplicate extraction target after normalization: {}",
                path.display()
            )));
        }
        if overwrite == OverwriteMode::Fail && path.exists() {
            return Err(ArchiveError::TargetExists(path.display().to_string()));
        }
        targets.push(PlannedExtractTarget {
            archive_path: entry.path,
            path,
        });
    }
    Ok(targets)
}

fn write_target_with(
    target: &Path,
    overwrite: OverwriteMode,
    fsync: bool,
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

    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let directory_sync_targets = if fsync {
        directory_sync_targets_for_create(parent)
    } else {
        Vec::new()
    };
    fs::create_dir_all(parent)?;
    let mut temp = NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    if fsync {
        temp.as_file_mut().sync_all()?;
    }
    persist_temp(
        temp,
        target,
        parent,
        overwrite,
        fsync,
        &directory_sync_targets,
    )
}

fn persist_temp(
    temp: NamedTempFile,
    target: &Path,
    parent: &Path,
    overwrite: OverwriteMode,
    fsync: bool,
    directory_sync_targets: &[PathBuf],
) -> Result<ExtractSummary> {
    match overwrite {
        OverwriteMode::Overwrite => {
            temp.persist(target)
                .map_err(|err| ArchiveError::Io(err.error))?;
        }
        OverwriteMode::Fail => {
            temp.persist_noclobber(target).map_err(|err| {
                if err.error.kind() == std::io::ErrorKind::AlreadyExists {
                    ArchiveError::TargetExists(target.display().to_string())
                } else {
                    ArchiveError::Io(err.error)
                }
            })?;
        }
        OverwriteMode::Skip => {
            if let Err(err) = temp.persist_noclobber(target) {
                if err.error.kind() == std::io::ErrorKind::AlreadyExists {
                    return Ok(ExtractSummary {
                        extracted: 0,
                        skipped: 1,
                    });
                }
                return Err(ArchiveError::Io(err.error));
            }
        }
    }
    if fsync {
        sync_parent_dir(parent)?;
        for directory_parent in directory_sync_targets {
            sync_parent_dir(directory_parent)?;
        }
    }
    Ok(ExtractSummary {
        extracted: 1,
        skipped: 0,
    })
}

fn directory_sync_targets_for_create(parent: &Path) -> Vec<PathBuf> {
    let mut sync_targets = Vec::new();
    let mut cursor = Some(parent);
    while let Some(path) = cursor {
        if path.exists() {
            break;
        }
        if let Some(parent) = path.parent() {
            sync_targets.push(parent.to_path_buf());
            cursor = Some(parent);
        } else {
            break;
        }
    }
    sync_targets
}

fn sync_parent_dir(parent: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dream-archivetool-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_tes3_archive(path: &Path) {
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder
            .add_bytes("textures/example.dds", b"payload")
            .unwrap();
        builder.write_path(path).unwrap();
    }

    fn write_multi_tes3_archive(path: &Path) {
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder.add_bytes("textures/a.dds", b"a").unwrap();
        builder.add_bytes("meshes/b.nif", b"b").unwrap();
        builder.write_path(path).unwrap();
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
        let err = safe_target_path_normalized(Path::new("out"), b"../evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));

        let err = safe_target_path_normalized(Path::new("out"), b"/evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));

        let err =
            safe_target_path_normalized(Path::new("out"), b"textures/../../evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));

        let err =
            safe_target_path_normalized(Path::new("out"), br"textures/../evil.txt").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));
    }

    #[test]
    fn accepts_current_directory_components() {
        let path =
            safe_target_path_normalized(Path::new("out"), b"./textures/./example.dds").unwrap();
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
    fn flat_extraction_uses_virtual_path_basename() {
        let dir = unique_dir("extract-flat-backslash");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let output = dir.join("out");

        extract_entry(
            &archive_path,
            "textures\\example.dds",
            &ExtractOptions {
                output: Some(output.clone()),
                overwrite: OverwriteMode::Fail,
                preserve_paths: false,
                fsync: false,
            },
        )
        .unwrap();

        assert_eq!(fs::read(output.join("example.dds")).unwrap(), b"payload");
        assert!(!output.join("textures\\example.dds").exists());
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
                fsync: false,
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

    #[test]
    fn extract_all_preflights_existing_targets_before_writing() {
        let dir = unique_dir("extract-all-preflight");
        let output_dir = dir.join("out");
        fs::create_dir_all(output_dir.join("meshes")).unwrap();
        let archive_path = dir.join("test.bsa");
        write_multi_tes3_archive(&archive_path);
        fs::write(output_dir.join("meshes/b.nif"), b"existing").unwrap();

        let err = extract_all(
            &archive_path,
            &ExtractAllOptions {
                output: Some(output_dir.clone()),
                overwrite: OverwriteMode::Fail,
                fsync: false,
            },
        )
        .unwrap_err();

        assert!(matches!(err, ArchiveError::TargetExists(_)));
        assert!(!output_dir.join("textures/a.dds").exists());
        assert_eq!(
            fs::read(output_dir.join("meshes/b.nif")).unwrap(),
            b"existing"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn extract_all_rejects_duplicate_planned_targets() {
        let dir = unique_dir("extract-all-duplicate-target");
        let entries = vec![
            crate::loaded::LoadedEntry {
                path: b"textures/example.dds".to_vec(),
                size: None,
                compressed_size: None,
            },
            crate::loaded::LoadedEntry {
                path: b"textures/example.dds".to_vec(),
                size: None,
                compressed_size: None,
            },
        ];

        let err = planned_extract_targets(&dir, entries, OverwriteMode::Overwrite).unwrap_err();

        assert!(err.to_string().contains("duplicate extraction target"));
    }

    #[test]
    fn extract_entry_uses_stored_path_for_output_target() {
        let dir = unique_dir("extract-canonical-target");
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        write_tes3_archive(&archive_path);
        let output = dir.join("out");

        extract_entry(
            &archive_path,
            "TEXTURES//EXAMPLE.DDS",
            &ExtractOptions {
                output: Some(output.clone()),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            fs::read(output.join("textures/example.dds")).unwrap(),
            b"payload"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn fsync_directory_plan_includes_new_directory_parents() {
        let dir = unique_dir("fsync-plan");
        fs::create_dir_all(&dir).unwrap();
        let parent = dir.join("a/b/c");

        let plan = directory_sync_targets_for_create(&parent);

        assert!(plan.contains(&dir.join("a/b")));
        assert!(plan.contains(&dir.join("a")));
        assert!(plan.contains(&dir));
        fs::remove_dir_all(dir).unwrap();
    }
}
