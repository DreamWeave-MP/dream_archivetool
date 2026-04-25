use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    AddOptions, ArchiveEntry, ArchiveFormat, CreateOptions, ExtractAllOptions, ExtractOptions,
    ExtractSummary, Result,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveInfo {
    pub path: String,
    pub format: ArchiveFormat,
    pub file_count: usize,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ArchiveTool;

impl ArchiveTool {
    pub fn guess_format(path: impl AsRef<Path>) -> Result<ArchiveFormat> {
        crate::format::guess_format(path.as_ref())
    }

    pub fn info(path: impl AsRef<Path>) -> Result<ArchiveInfo> {
        let path = path.as_ref();
        let archive = crate::loaded::LoadedArchive::open(path)?;
        let format = archive.format();
        let file_count = archive.file_count();
        Ok(ArchiveInfo {
            path: path.display().to_string(),
            format,
            file_count,
        })
    }

    pub fn list(path: impl AsRef<Path>) -> Result<Vec<ArchiveEntry>> {
        crate::entry::list_entries(path.as_ref())
    }

    pub fn read_entry(path: impl AsRef<Path>, entry: &str) -> Result<Vec<u8>> {
        crate::extract::read_entry_bytes(path.as_ref(), entry)
    }

    pub fn extract(
        path: impl AsRef<Path>,
        entry: &str,
        options: &ExtractOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_entry(path.as_ref(), entry, options)
    }

    pub fn extract_all(
        path: impl AsRef<Path>,
        options: &ExtractAllOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_all(path.as_ref(), options)
    }

    pub fn create(
        output: impl AsRef<Path>,
        input: impl AsRef<Path>,
        options: &CreateOptions,
    ) -> Result<usize> {
        crate::create::create_archive(output.as_ref(), input.as_ref(), options)
    }

    pub fn add(path: impl AsRef<Path>, options: &AddOptions) -> Result<usize> {
        crate::create::add_to_archive(path.as_ref(), options)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn reports_tes3_info() {
        let dir = std::env::temp_dir().join(format!(
            "rome-archivetool-info-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let archive: ba2::tes3::Archive = [(
            ba2::tes3::ArchiveKey::from(b"meshes/example.nif".as_slice()),
            ba2::tes3::File::from(b"payload".as_slice()),
        )]
        .into_iter()
        .collect();
        let mut output = fs::File::create(&archive_path).unwrap();
        archive.write(&mut output).unwrap();

        let info = ArchiveTool::info(&archive_path).unwrap();

        assert_eq!(info.format, ArchiveFormat::Tes3);
        assert_eq!(info.file_count, 1);

        fs::remove_dir_all(dir).unwrap();
    }
}
