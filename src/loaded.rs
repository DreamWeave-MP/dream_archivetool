use std::path::Path;

use dream_archive::{Archive, BStr, ByteSlice};

use crate::{ArchiveEntry, ArchiveError, ArchiveFormat, Result};

pub enum LoadedArchive {
    Tes3(dream_archive::bsa::tes3::Archive),
    Tes4(dream_archive::bsa::tes4::Archive),
    Fo4(dream_archive::ba2::Archive),
}

impl LoadedArchive {
    pub fn open(path: &Path) -> Result<Self> {
        match Archive::open_path(path).map_err(|err| ArchiveError::Archive(err.to_string()))? {
            Archive::Tes3Bsa(archive) => Ok(Self::Tes3(archive)),
            Archive::Tes4Bsa(archive) => Ok(Self::Tes4(archive)),
            Archive::BA2(archive) => Ok(Self::Fo4(archive)),
        }
    }

    pub fn file_count(&self) -> usize {
        match self {
            Self::Tes3(archive) => archive.len(),
            Self::Tes4(archive) => archive.len(),
            Self::Fo4(archive) => archive.len(),
        }
    }

    pub fn format(&self) -> ArchiveFormat {
        match self {
            Self::Tes3(_) => ArchiveFormat::Tes3,
            Self::Tes4(_) => ArchiveFormat::Tes4,
            Self::Fo4(_) => ArchiveFormat::Fo4,
        }
    }

    pub fn list_entries(&self) -> Vec<ArchiveEntry> {
        match self {
            Self::Tes3(archive) => archive
                .entries()
                .iter()
                .map(|entry| ArchiveEntry {
                    path: path_to_string(entry.path()),
                    size: Some(entry.file().size.into()),
                    compressed_size: None,
                })
                .collect(),
            Self::Tes4(archive) => archive
                .entries()
                .iter()
                .filter_map(|entry| {
                    let path = entry.path()?;
                    let record = entry.file();
                    Some(ArchiveEntry {
                        path: path_to_string(path),
                        size: Some(record.stored_size.into()),
                        compressed_size: record
                            .is_compressed(archive.info().archive_flags)
                            .then_some(record.stored_size.into()),
                    })
                })
                .collect(),
            Self::Fo4(archive) => archive
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
                    ArchiveEntry {
                        path: path_to_string(entry.name()),
                        size: Some(size),
                        compressed_size: (compressed_size > 0).then_some(compressed_size),
                    }
                })
                .collect(),
        }
    }

    pub fn read_entry_bytes(&self, entry: &str) -> Result<Vec<u8>> {
        let entry = normalize_archive_path(entry);
        let bytes = match self {
            Self::Tes3(archive) => archive
                .read_file(entry.as_bytes())
                .map_err(|err| ArchiveError::Archive(err.to_string()))?,
            Self::Tes4(archive) => archive
                .read_file(entry.as_bytes())
                .map_err(|err| ArchiveError::Archive(err.to_string()))?,
            Self::Fo4(archive) => archive
                .read_file(entry.as_bytes())
                .map_err(|err| ArchiveError::Archive(err.to_string()))?,
        };
        bytes.ok_or(ArchiveError::EntryNotFound(entry))
    }

    pub fn for_each_entry_bytes(
        &self,
        mut visit: impl FnMut(&str, Vec<u8>) -> Result<()>,
    ) -> Result<()> {
        for entry in self.list_entries() {
            let bytes = self.read_entry_bytes(&entry.path)?;
            visit(&entry.path, bytes)?;
        }
        Ok(())
    }
}

fn path_to_string(path: &BStr) -> String {
    normalize_archive_path(&path.to_str_lossy())
}

fn normalize_archive_path(path: &str) -> String {
    path.replace('\\', "/")
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
        archive
            .for_each_entry_bytes(|path, bytes| {
                entries.push((path.to_string(), bytes));
                Ok(())
            })
            .unwrap();

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
