use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::paths::{archive_path_bytes_to_display, path_to_archive_bytes};
use crate::{ArchiveError, Result};

pub(super) fn collect_input_entry_paths(
    input: &Path,
    follow_symlinks: bool,
) -> Result<BTreeMap<Vec<u8>, PathBuf>> {
    let mut entries = BTreeMap::new();
    reject_symlink(input, follow_symlinks)?;
    if input_metadata(input, follow_symlinks)?.is_file() {
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

    for item in WalkDir::new(input).follow_links(follow_symlinks) {
        let item = item.map_err(|err| ArchiveError::Archive(err.to_string()))?;
        let path = item.path();
        reject_symlink(path, follow_symlinks)?;
        if !item.file_type().is_file() {
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

fn reject_symlink(path: &Path, follow_symlinks: bool) -> Result<()> {
    if follow_symlinks {
        return Ok(());
    }
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(ArchiveError::Archive(format!(
            "refusing to follow symlink input path: {}; pass follow_symlinks to opt in",
            path.display()
        )));
    }
    Ok(())
}

fn input_metadata(path: &Path, follow_symlinks: bool) -> Result<fs::Metadata> {
    if follow_symlinks {
        Ok(fs::metadata(path)?)
    } else {
        Ok(fs::symlink_metadata(path)?)
    }
}

pub(super) fn insert_input_path(
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
