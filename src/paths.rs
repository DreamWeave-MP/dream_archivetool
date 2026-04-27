use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use dream_archive::ByteSlice;
use dream_path::NormalizedPath;

use crate::{ArchiveError, Result};

pub(crate) fn normalize_archive_path(path: &str) -> String {
    let normalized = NormalizedPath::new(path.as_bytes());
    normalized.as_bstr().to_str_lossy().into_owned()
}

pub(crate) fn normalize_archive_path_bytes(path: impl AsRef<[u8]>) -> Vec<u8> {
    NormalizedPath::new(path).into()
}

pub(crate) fn archive_path_bytes_to_display(path: &[u8]) -> String {
    path.as_bstr().to_str_lossy().into_owned()
}

pub(crate) fn path_to_archive_bytes(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for component in path.components() {
        if !bytes.is_empty() {
            bytes.push(b'/');
        }
        let Component::Normal(part) = component else {
            return Err(ArchiveError::UnsafePath(path.display().to_string()));
        };
        #[cfg(unix)]
        bytes.extend_from_slice(part.as_bytes());
        #[cfg(not(unix))]
        bytes.extend_from_slice(
            part.to_str()
                .ok_or_else(|| ArchiveError::UnsafePath(path.display().to_string()))?
                .as_bytes(),
        );
    }
    let normalized = normalize_archive_path_bytes(bytes);
    validate_virtual_path_bytes(&normalized)?;
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
    validate_virtual_path_bytes(path.as_bytes())
}

fn validate_virtual_path_bytes(path: &[u8]) -> Result<()> {
    if path.is_empty()
        || path.starts_with(b"/")
        || path.split(|byte| *byte == b'/').any(|part| part == b"..")
    {
        return Err(ArchiveError::UnsafePath(archive_path_bytes_to_display(
            path,
        )));
    }
    Ok(())
}
