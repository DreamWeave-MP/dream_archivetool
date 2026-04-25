use std::path::Path;

use ba2::prelude::*;

use crate::{
    ArchiveEntry, ArchiveError, ArchiveFormat, CreateOptions, Fo4ArchiveKind, Fo4Version, Result,
    Tes4Version,
};

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
                        let path = normalize_archive_path(&format!(
                            "{}/{}",
                            directory_key.name(),
                            file_key.name()
                        ));
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

    pub fn create_options(&self) -> CreateOptions {
        match self {
            Self::Tes3(_) => CreateOptions {
                format: ArchiveFormat::Tes3,
                ..Default::default()
            },
            Self::Tes4(_, options) => CreateOptions {
                format: ArchiveFormat::Tes4,
                tes4_version: match options.version() {
                    ba2::tes4::Version::v103 => Tes4Version::Oblivion,
                    ba2::tes4::Version::v104 => Tes4Version::Skyrim,
                    ba2::tes4::Version::v105 => Tes4Version::SkyrimSe,
                },
                ..Default::default()
            },
            Self::Fo4(_, options) => CreateOptions {
                format: ArchiveFormat::Fo4,
                fo4_kind: match options.format() {
                    ba2::fo4::Format::GNRL => Fo4ArchiveKind::Gnrl,
                    ba2::fo4::Format::DX10 => Fo4ArchiveKind::Dx10,
                    ba2::fo4::Format::GNMF => Fo4ArchiveKind::Gnmf,
                },
                fo4_version: match options.version() {
                    ba2::fo4::Version::v1 => Fo4Version::Fallout4,
                    ba2::fo4::Version::v2 | ba2::fo4::Version::v3 => Fo4Version::Starfield,
                    ba2::fo4::Version::v7 | ba2::fo4::Version::v8 => Fo4Version::Fallout4NextGen,
                },
                ..Default::default()
            },
        }
    }

    pub fn read_entry_bytes(&self, entry: &str) -> Result<Vec<u8>> {
        match self {
            Self::Tes3(archive) => {
                for (key, file) in archive {
                    if archive_path_eq(key.name(), entry) {
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
                        if archive_path_eq(&candidate, entry) {
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
                    if archive_path_eq(key.name(), entry) {
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
}

fn archive_path_eq(left: &(impl ToString + ?Sized), right: &str) -> bool {
    normalize_archive_path(&left.to_string()).eq_ignore_ascii_case(&normalize_archive_path(right))
}

fn normalize_archive_path(path: &(impl ToString + ?Sized)) -> String {
    path.to_string().replace('\\', "/")
}
