use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::paths::{archive_path_bytes_to_display, path_to_archive_bytes};
use crate::{ArchiveError, Result};

pub(super) fn collect_input_entry_paths(input: &Path) -> Result<BTreeMap<Vec<u8>, PathBuf>> {
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
