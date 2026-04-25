use std::io::Write;
use std::path::Path;

use ba2::prelude::*;

use crate::{ArchiveEntry, ArchiveError, ArchiveFormat, Result};

pub enum LoadedArchive {
    Tes3(ba2::tes3::Archive<'static>),
    Tes4(ba2::tes4::Archive<'static>, ba2::tes4::ArchiveOptions),
    Fo4(ba2::fo4::Archive<'static>, ba2::fo4::ArchiveOptions),
}

impl LoadedArchive {
    pub fn open(path: &Path) -> Result<Self> {
        match crate::format::guess_format(path)? {
            ArchiveFormat::Tes3 => Ok(Self::Tes3(
                ba2::tes3::Archive::read(path)
                    .map_err(|err| ArchiveError::Archive(err.to_string()))?,
            )),
            ArchiveFormat::Tes4 => {
                let (archive, options) = ba2::tes4::Archive::read(path)
                    .map_err(|err| ArchiveError::Archive(err.to_string()))?;
                Ok(Self::Tes4(archive, options))
            }
            ArchiveFormat::Fo4 => {
                let (archive, options) = ba2::fo4::Archive::read(path)
                    .map_err(|err| ArchiveError::Archive(err.to_string()))?;
                Ok(Self::Fo4(archive, options))
            }
        }
    }

    pub fn file_count(&self) -> usize {
        match self {
            Self::Tes3(archive) => archive.len(),
            Self::Tes4(archive, _) => archive.values().map(ba2::tes4::Directory::len).sum(),
            Self::Fo4(archive, _) => archive.len(),
        }
    }

    pub fn format(&self) -> ArchiveFormat {
        match self {
            Self::Tes3(_) => ArchiveFormat::Tes3,
            Self::Tes4(_, _) => ArchiveFormat::Tes4,
            Self::Fo4(_, _) => ArchiveFormat::Fo4,
        }
    }

    pub fn list_entries(&self) -> Vec<ArchiveEntry> {
        match self {
            Self::Tes3(archive) => archive
                .iter()
                .map(|(key, file)| ArchiveEntry {
                    path: normalize_archive_path(key.name()),
                    size: Some(file.len() as u64),
                    compressed_size: None,
                })
                .collect(),
            Self::Tes4(archive, _) => {
                let mut entries = Vec::new();
                for (directory_key, directory) in archive {
                    for (file_key, file) in directory {
                        let path = joined_archive_path(directory_key.name(), file_key.name());
                        let compressed_size = file.is_compressed().then_some(file.len() as u64);
                        entries.push(ArchiveEntry {
                            path,
                            size: Some(file.decompressed_len().unwrap_or_else(|| file.len()) as u64),
                            compressed_size,
                        });
                    }
                }
                entries
            }
            Self::Fo4(archive, _) => archive
                .iter()
                .map(|(key, file)| {
                    let compressed_size = file
                        .iter()
                        .filter(|chunk| chunk.is_compressed())
                        .map(|chunk| chunk.len() as u64)
                        .sum();
                    let decompressed_size = file
                        .iter()
                        .map(|chunk| chunk.decompressed_len().unwrap_or_else(|| chunk.len()) as u64)
                        .sum();
                    ArchiveEntry {
                        path: normalize_archive_path(key.name()),
                        size: Some(decompressed_size),
                        compressed_size: (compressed_size > 0).then_some(compressed_size),
                    }
                })
                .collect(),
        }
    }

    pub fn read_entry_bytes(&self, entry: &str) -> Result<Vec<u8>> {
        let entry = normalize_archive_path(entry);
        match self {
            Self::Tes3(archive) => {
                for (key, file) in archive {
                    if archive_path_eq_normalized(key.name(), &entry) {
                        let mut bytes = Vec::with_capacity(file.len());
                        file.write(&mut bytes)
                            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
                        return Ok(bytes);
                    }
                }
            }
            Self::Tes4(archive, archive_options) => {
                let file_options = ba2::tes4::FileCompressionOptions::from(archive_options);
                for (directory_key, directory) in archive {
                    for (file_key, file) in directory {
                        let candidate = format!("{}/{}", directory_key.name(), file_key.name());
                        if archive_path_eq_normalized(&candidate, &entry) {
                            let mut bytes = Vec::with_capacity(
                                file.decompressed_len().unwrap_or_else(|| file.len()),
                            );
                            file.write(&mut bytes, &file_options)
                                .map_err(|err| ArchiveError::Archive(err.to_string()))?;
                            return Ok(bytes);
                        }
                    }
                }
            }
            Self::Fo4(archive, archive_options) => {
                let file_options = ba2::fo4::FileWriteOptions::from(archive_options);
                for (key, file) in archive {
                    if archive_path_eq_normalized(key.name(), &entry) {
                        let capacity = file
                            .iter()
                            .map(|chunk| chunk.decompressed_len().unwrap_or_else(|| chunk.len()))
                            .sum();
                        let mut bytes = Vec::with_capacity(capacity);
                        file.write(&mut bytes, &file_options)
                            .map_err(|err| ArchiveError::Archive(err.to_string()))?;
                        return Ok(bytes);
                    }
                }
            }
        }
        Err(ArchiveError::EntryNotFound(entry.to_string()))
    }

    pub fn for_each_entry_writer(
        &self,
        mut visit: impl FnMut(&str, LoadedEntryWriter<'_>) -> Result<()>,
    ) -> Result<()> {
        match self {
            Self::Tes3(archive) => {
                for (key, file) in archive {
                    visit(
                        &normalize_archive_path(key.name()),
                        LoadedEntryWriter::Tes3(file),
                    )?;
                }
                Ok(())
            }
            Self::Tes4(archive, archive_options) => {
                let file_options = ba2::tes4::FileCompressionOptions::from(archive_options);
                for (directory_key, directory) in archive {
                    for (file_key, file) in directory {
                        let path = joined_archive_path(directory_key.name(), file_key.name());
                        visit(&path, LoadedEntryWriter::Tes4(file, file_options))?;
                    }
                }
                Ok(())
            }
            Self::Fo4(archive, archive_options) => {
                let file_options = ba2::fo4::FileWriteOptions::from(archive_options);
                for (key, file) in archive {
                    visit(
                        &normalize_archive_path(key.name()),
                        LoadedEntryWriter::Fo4(file, file_options),
                    )?;
                }
                Ok(())
            }
        }
    }
}

pub(crate) enum LoadedEntryWriter<'a> {
    Tes3(&'a ba2::tes3::File<'static>),
    Tes4(
        &'a ba2::tes4::File<'static>,
        ba2::tes4::FileCompressionOptions,
    ),
    Fo4(&'a ba2::fo4::File<'static>, ba2::fo4::FileWriteOptions),
}

impl LoadedEntryWriter<'_> {
    pub(crate) fn write_to(&self, output: &mut dyn Write) -> Result<()> {
        match self {
            Self::Tes3(file) => file
                .write(output)
                .map_err(|err| ArchiveError::Archive(err.to_string())),
            Self::Tes4(file, options) => file
                .write(output, options)
                .map_err(|err| ArchiveError::Archive(err.to_string())),
            Self::Fo4(file, options) => file
                .write(output, options)
                .map_err(|err| ArchiveError::Archive(err.to_string())),
        }
    }
}

fn archive_path_eq_normalized(left: &(impl ToString + ?Sized), normalized_right: &str) -> bool {
    let left = left.to_string();
    if left.contains('\\') {
        normalize_archive_path(&left).eq_ignore_ascii_case(normalized_right)
    } else {
        left.eq_ignore_ascii_case(normalized_right)
    }
}

fn normalize_archive_path(path: &(impl ToString + ?Sized)) -> String {
    path.to_string().replace('\\', "/")
}

fn joined_archive_path(
    directory: &(impl ToString + ?Sized),
    file: &(impl ToString + ?Sized),
) -> String {
    let directory = normalize_archive_path(directory);
    let file = normalize_archive_path(file);
    let mut path = String::with_capacity(directory.len() + 1 + file.len());
    path.push_str(&directory);
    if !path.is_empty() {
        path.push('/');
    }
    path.push_str(&file);
    path
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn streams_entries_without_collecting_first() {
        let dir = std::env::temp_dir().join(format!(
            "rome-archivetool-stream-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let archive: ba2::tes3::Archive = [
            (
                ba2::tes3::ArchiveKey::from(b"a.txt".as_slice()),
                ba2::tes3::File::from(b"a".as_slice()),
            ),
            (
                ba2::tes3::ArchiveKey::from(b"b.txt".as_slice()),
                ba2::tes3::File::from(b"b".as_slice()),
            ),
        ]
        .into_iter()
        .collect();
        let mut output = fs::File::create(&archive_path).unwrap();
        archive.write(&mut output).unwrap();
        let archive = LoadedArchive::open(&archive_path).unwrap();
        let mut visited = Vec::new();

        archive
            .for_each_entry_writer(|path, writer| {
                let mut bytes = Vec::new();
                writer.write_to(&mut bytes).unwrap();
                visited.push((path.to_string(), bytes));
                Ok(())
            })
            .unwrap();

        assert_eq!(visited.len(), 2);
        assert!(visited.contains(&("a.txt".to_string(), b"a".to_vec())));
        assert!(visited.contains(&("b.txt".to_string(), b"b".to_vec())));
        fs::remove_dir_all(dir).unwrap();
    }
}
