#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use dream_path::{ByteSlice, NormalizedPath};

use crate::{ArchiveError, Result};

pub(crate) fn normalize_archive_path(path: &str) -> String {
    let normalized = NormalizedPath::new(path.as_bytes());
    normalized.as_bstr().to_str_lossy().into_owned()
}

pub(crate) fn normalize_archive_path_bytes(path: impl AsRef<[u8]>) -> Vec<u8> {
    NormalizedPath::new(path).into()
}

pub(crate) fn archive_path_bytes_to_hex(path: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(path.len() * 2);
    for byte in path {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
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

pub(crate) fn safe_target_path_normalized(root: &Path, normalized: &[u8]) -> Result<PathBuf> {
    validate_virtual_path_bytes(normalized)?;
    ensure_platform_target_path_bytes(normalized);
    let mut target = PathBuf::from(root);
    for component in normalized.split(|byte| *byte == b'/') {
        if component == b"." {
            continue;
        }
        push_component_bytes(&mut target, component);
    }
    Ok(target)
}

pub(crate) fn flat_target_path_normalized(root: &Path, normalized: &[u8]) -> Result<PathBuf> {
    validate_virtual_path_bytes(normalized)?;
    ensure_platform_target_path_bytes(normalized);
    let normalized_path = NormalizedPath::new(normalized);
    let file_name = normalized_path
        .file_name()
        .ok_or_else(|| ArchiveError::UnsafePath(archive_path_bytes_to_display(normalized)))?;
    let mut target = PathBuf::from(root);
    push_component_bytes(&mut target, file_name.as_bytes());
    Ok(target)
}

#[cfg(not(unix))]
fn ensure_platform_target_path_bytes(path: &[u8]) {
    assert!(
        std::str::from_utf8(path).is_ok(),
        "validated UTF-8 archive path"
    );
}

#[cfg(unix)]
fn ensure_platform_target_path_bytes(path: &[u8]) {
    let _ = path;
}

fn push_component_bytes(target: &mut PathBuf, component: &[u8]) {
    #[cfg(unix)]
    target.push(OsStr::from_bytes(component));
    #[cfg(not(unix))]
    target.push(std::str::from_utf8(component).expect("validated UTF-8 archive path component"));
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
