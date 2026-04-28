// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A single file entry listed from an archive.
#[non_exhaustive]
pub struct ArchiveEntry {
    /// Normalized archive path using `/` separators, converted lossily for display.
    ///
    /// This is presentation text, not the authoritative identity for entries whose names are not
    /// valid UTF-8. Use [`Self::path_bytes_hex`] for scriptable normalized lookup.
    pub path: String,
    /// Hex-encoded normalized archive path bytes for scriptable lookup, not raw identity.
    #[serde(default)]
    pub path_bytes_hex: String,
    /// Decompressed size when known.
    pub size: Option<u64>,
    /// Compressed size when the entry is stored compressed and the format exposes it.
    pub compressed_size: Option<u64>,
}

/// List all entries in an archive.
///
/// # Errors
///
/// Returns an error if the archive cannot be opened or its entries cannot be read.
pub fn list_entries(path: &Path) -> Result<Vec<ArchiveEntry>> {
    crate::loaded::LoadedArchive::open(path)?.list_entries()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn lists_tes3_entries() {
        let dir = std::env::temp_dir().join(format!(
            "dream_archivetool-list-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder
            .add_bytes("textures/example.dds", b"payload")
            .unwrap();
        builder.write_path(&archive_path).unwrap();

        let entries = list_entries(&archive_path).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "textures/example.dds");
        assert_eq!(
            entries[0].path_bytes_hex,
            "74657874757265732f6578616d706c652e646473"
        );
        assert_eq!(entries[0].size, Some(7));

        fs::remove_dir_all(dir).unwrap();
    }
}
