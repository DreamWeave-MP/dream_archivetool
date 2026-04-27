use std::io::Write;
use std::path::Path;

use dream_archive::Archive;
use dream_path::ByteSlice;

use crate::paths::{
    archive_path_bytes_to_display, archive_path_bytes_to_hex, normalize_archive_path,
    normalize_archive_path_bytes,
};
use crate::{ArchiveEntry, ArchiveError, ArchiveFormat, Result};

#[derive(Debug, Clone)]
pub(crate) struct LoadedEntry {
    pub path: Vec<u8>,
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
}

impl LoadedEntry {
    fn public_entry(&self) -> ArchiveEntry {
        ArchiveEntry {
            path: archive_path_bytes_to_display(&self.path),
            path_bytes_hex: archive_path_bytes_to_hex(&self.path),
            size: self.size,
            compressed_size: self.compressed_size,
        }
    }
}

pub struct LoadedArchive {
    archive: Archive,
}

impl LoadedArchive {
    pub fn open(path: &Path) -> Result<Self> {
        let archive = Archive::open_path(path).map_err(|err| {
            ArchiveError::Archive(format!(
                "failed to open archive '{}': {err}",
                path.display()
            ))
        })?;
        Ok(Self { archive })
    }

    pub(crate) fn as_dream_archive(&self) -> &Archive {
        &self.archive
    }

    pub fn file_count(&self) -> usize {
        self.archive.len()
    }

    pub fn format(&self) -> ArchiveFormat {
        match self.archive.format() {
            dream_archive::FileFormat::BSA(dream_archive::BsaFormat::TES3) => ArchiveFormat::Tes3,
            dream_archive::FileFormat::BSA(dream_archive::BsaFormat::TES4) => ArchiveFormat::Tes4,
            dream_archive::FileFormat::BA2 => ArchiveFormat::Ba2,
        }
    }

    pub fn list_entries(&self) -> Result<Vec<ArchiveEntry>> {
        Ok(self
            .list_loaded_entries()?
            .into_iter()
            .map(|entry| entry.public_entry())
            .collect())
    }

    pub(crate) fn list_loaded_entries(&self) -> Result<Vec<LoadedEntry>> {
        Ok(match &self.archive {
            Archive::Tes3Bsa(archive) => archive
                .entries()
                .iter()
                .map(|entry| {
                    let path = normalize_archive_path_bytes(entry.path().as_bytes());
                    Ok(LoadedEntry {
                        path,
                        size: Some(entry.file().size.into()),
                        compressed_size: None,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            Archive::Tes4Bsa(archive) => archive
                .entries()
                .iter()
                .filter_map(|entry| {
                    let path = entry.path()?;
                    let path = normalize_archive_path_bytes(path.as_bytes());
                    let record = entry.file();
                    Some(Ok(LoadedEntry {
                        path,
                        size: None,
                        compressed_size: record
                            .is_compressed(archive.info().archive_flags)
                            .then_some(record.stored_size.into()),
                    }))
                })
                .collect::<Result<Vec<_>>>()?,
            Archive::BA2(archive) => archive
                .entries()
                .iter()
                .filter(|entry| !entry.name().is_empty())
                .map(|entry| {
                    let size = entry
                        .file()
                        .chunks()
                        .iter()
                        .map(|chunk| u64::from(chunk.size()))
                        .sum();
                    let compressed_size = entry
                        .file()
                        .chunks()
                        .iter()
                        .filter(|chunk| chunk.is_compressed())
                        .map(|chunk| u64::from(chunk.packed_size()))
                        .sum::<u64>();
                    let path = normalize_archive_path_bytes(entry.name().as_bytes());
                    Ok(LoadedEntry {
                        path,
                        size: Some(size),
                        compressed_size: (compressed_size > 0).then_some(compressed_size),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }

    pub fn named_entry_count(&self) -> usize {
        match &self.archive {
            Archive::Tes3Bsa(archive) => archive.len(),
            Archive::Tes4Bsa(archive) => archive
                .entries()
                .iter()
                .filter(|entry| entry.path().is_some())
                .count(),
            Archive::BA2(archive) => archive
                .entries()
                .iter()
                .filter(|entry| !entry.name().is_empty())
                .count(),
        }
    }

    pub fn has_unnameable_entries(&self) -> bool {
        self.named_entry_count() != self.file_count()
    }

    pub fn read_entry_bytes(&self, entry: &str) -> Result<Vec<u8>> {
        let entry = normalize_archive_path(entry);
        self.read_entry_bytes_by_path(entry.as_bytes())
            .map_err(|err| match err {
                ArchiveError::EntryNotFound(_) => ArchiveError::EntryNotFound(entry),
                err => err,
            })
    }

    pub fn read_entry_bytes_by_path(&self, entry: &[u8]) -> Result<Vec<u8>> {
        let entry = normalize_archive_path_bytes(entry);
        self.read_entry_bytes_by_normalized_path(&entry)
    }

    pub(crate) fn read_entry_bytes_by_normalized_path(&self, entry: &[u8]) -> Result<Vec<u8>> {
        let bytes = self
            .archive
            .read_file(entry)
            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
        bytes.ok_or_else(|| ArchiveError::EntryNotFound(archive_path_bytes_to_display(entry)))
    }

    pub fn extract_entry_to_writer(&self, entry: &str, out: &mut dyn Write) -> Result<u64> {
        let entry = normalize_archive_path(entry);
        self.extract_entry_path_to_writer(entry.as_bytes(), out)
            .map_err(|err| match err {
                ArchiveError::EntryNotFound(_) => ArchiveError::EntryNotFound(entry),
                err => err,
            })
    }

    pub fn extract_entry_path_to_writer(&self, entry: &[u8], out: &mut dyn Write) -> Result<u64> {
        let entry = normalize_archive_path_bytes(entry);
        self.extract_normalized_entry_path_to_writer(&entry, out)
    }

    pub(crate) fn extract_normalized_entry_path_to_writer(
        &self,
        entry: &[u8],
        out: &mut dyn Write,
    ) -> Result<u64> {
        let written = self
            .archive
            .extract_file_required(entry, out)
            .map_err(|err| match err {
                dream_archive::Error::FileNotFound(_) => {
                    ArchiveError::EntryNotFound(archive_path_bytes_to_display(entry))
                }
                err => ArchiveError::Archive(err.to_string()),
            })?;
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn streams_entries_without_collecting_first() {
        let dir = std::env::temp_dir().join(format!(
            "dream-archivetool-stream-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder.add_bytes("a.txt", b"a").unwrap();
        builder.add_bytes("b.txt", b"b").unwrap();
        builder.write_path(&archive_path).unwrap();

        let archive = LoadedArchive::open(&archive_path).unwrap();
        let mut entries = Vec::new();
        for entry in archive.list_entries().unwrap() {
            let bytes = archive.read_entry_bytes(&entry.path).unwrap();
            entries.push((entry.path, bytes));
        }

        entries.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            entries,
            vec![
                ("a.txt".into(), b"a".to_vec()),
                ("b.txt".into(), b"b".to_vec())
            ]
        );

        fs::remove_dir_all(dir).unwrap();
    }
}
