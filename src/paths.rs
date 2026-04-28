// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use dream_archive::{ByteSlice, dream_path::NormalizedPath};

use crate::{ArchiveError, Result};

pub(crate) fn normalize_archive_path(path: &str) -> String {
    let normalized = NormalizedPath::new(path.as_bytes());
    normalized.as_bstr().to_str_lossy().into_owned()
}

pub(crate) fn normalize_archive_path_bytes(path: impl AsRef<[u8]>) -> Vec<u8> {
    NormalizedPath::new(path).into()
}

pub(crate) fn normalize_lookup_archive_path_bytes(path: impl AsRef<[u8]>) -> Result<Vec<u8>> {
    let normalized = normalize_archive_path_bytes(path);
    if normalized.is_empty() || normalized.contains(&b'\0') {
        return Err(ArchiveError::UnsafePath(archive_path_bytes_to_display(
            &normalized,
        )));
    }
    Ok(normalized)
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
    validate_archive_path_bytes_for_extraction(&normalized)?;
    Ok(normalized)
}

pub(crate) fn normalize_safe_archive_path_bytes(path: impl AsRef<[u8]>) -> Result<Vec<u8>> {
    validate_archive_path_bytes_for_extraction(path.as_ref())?;
    let normalized = normalize_lookup_archive_path_bytes(path)?;
    validate_archive_path_bytes_for_extraction(&normalized)?;
    Ok(normalized)
}

pub(crate) fn safe_target_path_normalized(root: &Path, normalized: &[u8]) -> Result<PathBuf> {
    validate_archive_path_bytes_for_extraction(normalized)?;
    ensure_platform_target_path_bytes(normalized)?;
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
    validate_archive_path_bytes_for_extraction(normalized)?;
    ensure_platform_target_path_bytes(normalized)?;
    let normalized_path = NormalizedPath::new(normalized);
    let file_name = normalized_path
        .file_name()
        .ok_or_else(|| ArchiveError::UnsafePath(archive_path_bytes_to_display(normalized)))?;
    let mut target = PathBuf::from(root);
    push_component_bytes(&mut target, file_name.as_bytes());
    Ok(target)
}

fn ensure_platform_target_path_bytes(path: &[u8]) -> Result<()> {
    if !target_paths_require_utf8_components() {
        return Ok(());
    }
    for component in path.split(|byte| *byte == b'/') {
        let component = std::str::from_utf8(component)
            .map_err(|_| ArchiveError::UnsafePath(archive_path_bytes_to_display(path)))?;
        if Path::new(component).components().count() != 1
            || !matches!(
                Path::new(component).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err(ArchiveError::UnsafePath(archive_path_bytes_to_display(
                path,
            )));
        }
    }
    Ok(())
}

const fn target_paths_require_utf8_components() -> bool {
    cfg!(any(not(unix), target_os = "macos"))
}

fn push_component_bytes(target: &mut PathBuf, component: &[u8]) {
    #[cfg(unix)]
    target.push(OsStr::from_bytes(component));
    #[cfg(not(unix))]
    target.push(std::str::from_utf8(component).expect("validated UTF-8 archive path component"));
}

pub(crate) fn validate_archive_path_bytes_for_extraction(path: &[u8]) -> Result<()> {
    let has_filename_component = path
        .split(|byte| *byte == b'/' || *byte == b'\\')
        .any(|part| !part.is_empty() && part != b".");
    if path.is_empty()
        || path.starts_with(b"/")
        || path.starts_with(b"\\")
        || path.contains(&b'\0')
        || !has_filename_component
        || path
            .split(|byte| *byte == b'/' || *byte == b'\\')
            .any(|part| part == b".." || part.contains(&b':'))
    {
        return Err(ArchiveError::UnsafePath(archive_path_bytes_to_display(
            path,
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_that_are_not_safe_on_common_targets() {
        for path in [
            b"".as_slice(),
            b"/absolute.txt",
            b"textures/../evil.txt",
            b"textures/has\0nul.txt",
            b"C:/evil.txt",
            b"C:evil.txt",
            b".",
            b"./.",
            b".\\.",
        ] {
            assert!(validate_archive_path_bytes_for_extraction(path).is_err());
        }
    }

    #[test]
    fn rejects_dot_only_normalized_targets() {
        for path in [b".".as_slice(), b"./.", b".\\."] {
            assert!(safe_target_path_normalized(Path::new("out"), path).is_err());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_non_utf8_target_paths_on_macos() {
        let err = safe_target_path_normalized(Path::new("out"), b"bad-\xff.dds").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));

        let err =
            flat_target_path_normalized(Path::new("out"), b"textures/bad-\xff.dds").unwrap_err();
        assert!(matches!(err, ArchiveError::UnsafePath(_)));
    }
}
