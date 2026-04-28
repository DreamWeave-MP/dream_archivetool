//! Public helpers for archive virtual-path byte contracts.
//!
//! Archive entry identity is byte-oriented. Display strings are for humans; these helpers are for
//! frontends that need to feed `path_bytes_hex` normalized lookup keys back into this crate without
//! copying CLI-only parsing code. Yes, that was a real footgun. Now it has a name.

use crate::{ArchiveError, Result};

/// Normalize archive virtual-path bytes only.
///
/// This is not a safety check. Lookup and extraction apply additional validation before reading or
/// writing files.
#[must_use]
pub fn normalize_archive_path_bytes(path: impl AsRef<[u8]>) -> Vec<u8> {
    crate::paths::normalize_archive_path_bytes(path)
}

/// Encode archive path bytes as lowercase hexadecimal text.
///
/// This function does not normalize its input. Call [`normalize_archive_path_bytes`] first when
/// encoding lookup keys compatible with `path_bytes_hex` report fields.
#[must_use]
pub fn encode_archive_path_hex(path: &[u8]) -> String {
    crate::paths::archive_path_bytes_to_hex(path)
}

/// Decode hexadecimal archive path bytes from `ArchiveEntry::path_bytes_hex`.
pub fn decode_archive_path_hex(hex: &str) -> Result<Vec<u8>> {
    let bytes = hex.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(ArchiveError::Archive(
            "archive path hex must contain an even number of hexadecimal digits".to_string(),
        ));
    }
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn hex_nibble(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ArchiveError::Archive(
            "archive path hex contains a non-hexadecimal digit".to_string(),
        )),
    }
}
