use std::path::{Component, Path, PathBuf};

use dream_archive::ByteSlice;
use dream_path::NormalizedPath;

use crate::{ArchiveError, Result};

pub(crate) fn normalize_archive_path(path: &str) -> String {
    let normalized = NormalizedPath::new(path.as_bytes());
    normalized.as_bstr().to_str_lossy().into_owned()
}

pub(crate) fn normalize_archive_path_bytes(path: impl AsRef<[u8]>) -> String {
    let normalized = NormalizedPath::new(path.as_ref());
    normalized.as_bstr().to_str_lossy().into_owned()
}

pub(crate) fn path_to_archive_string(path: &Path) -> Result<String> {
    let value = path.to_string_lossy();
    let normalized = normalize_archive_path(&value);
    validate_virtual_path(&normalized)?;
    Ok(normalized)
}

pub(crate) fn safe_target_path(root: &Path, archive_path: &str) -> Result<PathBuf> {
    if archive_path.starts_with('/') || archive_path.starts_with('\\') {
        return Err(ArchiveError::UnsafePath(archive_path.to_string()));
    }

    let normalized = normalize_archive_path(archive_path);
    validate_virtual_path(&normalized)?;

    let mut target = PathBuf::from(root);
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => target.push(part),
            Component::CurDir => {}
            _ => return Err(ArchiveError::UnsafePath(archive_path.to_string())),
        }
    }
    Ok(target)
}

pub(crate) fn flat_target_path(root: &Path, archive_path: &str) -> Result<PathBuf> {
    let normalized = normalize_archive_path(archive_path);
    validate_virtual_path(&normalized)?;
    let file_name = NormalizedPath::new(normalized.as_bytes())
        .file_name()
        .ok_or_else(|| ArchiveError::UnsafePath(archive_path.to_string()))?
        .to_str_lossy()
        .into_owned();
    Ok(root.join(file_name))
}

fn validate_virtual_path(path: &str) -> Result<()> {
    if path.is_empty() || path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(ArchiveError::UnsafePath(path.to_string()));
    }
    Ok(())
}
