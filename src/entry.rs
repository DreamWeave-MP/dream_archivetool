use std::path::Path;

use ba2::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{ArchiveFormat, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
}

pub fn list_entries(path: &Path) -> Result<Vec<ArchiveEntry>> {
    match crate::format::guess_format(path)? {
        ArchiveFormat::Tes3 => list_tes3(path),
        ArchiveFormat::Tes4 => list_tes4(path),
        ArchiveFormat::Fo4 => list_fo4(path),
    }
}

fn list_tes3(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let archive = ba2::tes3::Archive::read(path)
        .map_err(|err| crate::ArchiveError::Archive(err.to_string()))?;
    Ok(archive
        .iter()
        .map(|(key, file)| ArchiveEntry {
            path: normalized_archive_path(key.name()),
            size: Some(file.len() as u64),
            compressed_size: None,
        })
        .collect())
}

fn list_tes4(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let (archive, _options) = ba2::tes4::Archive::read(path)
        .map_err(|err| crate::ArchiveError::Archive(err.to_string()))?;
    let mut entries = Vec::new();
    for (directory_key, directory) in &archive {
        for (file_key, file) in directory {
            let path =
                normalized_archive_path(&format!("{}/{}", directory_key.name(), file_key.name()));
            let compressed_size = file.is_compressed().then_some(file.len() as u64);
            entries.push(ArchiveEntry {
                path,
                size: file
                    .decompressed_len()
                    .map_or(Some(file.len() as u64), |len| Some(len as u64)),
                compressed_size,
            });
        }
    }
    Ok(entries)
}

fn list_fo4(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let (archive, _options) = ba2::fo4::Archive::read(path)
        .map_err(|err| crate::ArchiveError::Archive(err.to_string()))?;
    Ok(archive
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
                path: normalized_archive_path(key.name()),
                size: Some(decompressed_size),
                compressed_size: (compressed_size > 0).then_some(compressed_size),
            }
        })
        .collect())
}

fn normalized_archive_path(path: &(impl ToString + ?Sized)) -> String {
    path.to_string().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn lists_tes3_entries() {
        let dir = std::env::temp_dir().join(format!(
            "rome-archivetool-list-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let archive: ba2::tes3::Archive = [(
            ba2::tes3::ArchiveKey::from(b"textures/example.dds".as_slice()),
            ba2::tes3::File::from(b"payload".as_slice()),
        )]
        .into_iter()
        .collect();
        let mut output = fs::File::create(&archive_path).unwrap();
        archive.write(&mut output).unwrap();

        let entries = list_entries(&archive_path).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "textures/example.dds");
        assert_eq!(entries[0].size, Some(7));

        fs::remove_dir_all(dir).unwrap();
    }
}
