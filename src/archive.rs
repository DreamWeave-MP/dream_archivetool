use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    AddOptions, ArchiveEntry, ArchiveFormat, CreateOptions, ExtractAllOptions, ExtractOptions,
    ExtractSummary, Result,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Basic metadata about an archive.
pub struct ArchiveInfo {
    /// Path that was opened, formatted for display.
    pub path: String,
    /// Detected archive family.
    pub format: ArchiveFormat,
    /// Number of file entries in the archive.
    pub file_count: usize,
}

/// Opened archive handle for batch inspection and extraction without reopening the file.
pub struct OpenArchive {
    path: String,
    archive: crate::loaded::LoadedArchive,
}

impl OpenArchive {
    /// Open an archive once and keep its index available for repeated operations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let archive = crate::loaded::LoadedArchive::open(path)?;
        Ok(Self {
            path: path.display().to_string(),
            archive,
        })
    }

    /// Return archive format plus file-count metadata.
    #[must_use]
    pub fn info(&self) -> ArchiveInfo {
        ArchiveInfo {
            path: self.path.clone(),
            format: self.format(),
            file_count: self.file_count(),
        }
    }

    /// Return the detected archive family.
    #[must_use]
    pub fn format(&self) -> ArchiveFormat {
        self.archive.format()
    }

    /// Return the number of file entries in the archive.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.archive.file_count()
    }

    /// List all entries in the archive.
    pub fn list(&self) -> Result<Vec<ArchiveEntry>> {
        self.archive.list_entries()
    }

    /// Read a single archive entry into memory.
    pub fn read_entry(&self, entry: &str) -> Result<Vec<u8>> {
        self.archive.read_entry_bytes(entry)
    }

    /// Read a single archive entry selected by raw archive path bytes into memory.
    pub fn read_entry_by_path(&self, entry: &[u8]) -> Result<Vec<u8>> {
        self.archive.read_entry_bytes_by_path(entry)
    }

    /// Extract a single archive entry into a writer without materializing the whole payload.
    pub fn extract_entry_to_writer(&self, entry: &str, out: &mut dyn Write) -> Result<u64> {
        self.archive.extract_entry_to_writer(entry, out)
    }

    /// Extract a single archive entry selected by raw archive path bytes into a writer.
    pub fn extract_entry_path_to_writer(&self, entry: &[u8], out: &mut dyn Write) -> Result<u64> {
        self.archive.extract_entry_path_to_writer(entry, out)
    }
}

/// Stateless facade for archive inspection, extraction, creation, and update operations.
///
/// This type exists to provide a compact public API shared by the CLI and Lua bindings. Each method
/// opens the archive it operates on; callers that need repeated list/read/extract operations should
/// use [`OpenArchive`] to avoid reopening and rebuilding archive indexes.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArchiveTool;

impl ArchiveTool {
    /// Detect the archive format from the file header.
    pub fn open(path: impl AsRef<Path>) -> Result<OpenArchive> {
        OpenArchive::open(path)
    }

    pub fn guess_format(path: impl AsRef<Path>) -> Result<ArchiveFormat> {
        crate::format::guess_format(path.as_ref())
    }

    /// Open an archive and return format plus file-count metadata.
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

    /// List all entries in an archive.
    pub fn list(path: impl AsRef<Path>) -> Result<Vec<ArchiveEntry>> {
        crate::entry::list_entries(path.as_ref())
    }

    /// Read a single archive entry into memory.
    ///
    /// Entry path matching is case-insensitive and treats `\` as `/`.
    pub fn read_entry(path: impl AsRef<Path>, entry: &str) -> Result<Vec<u8>> {
        crate::extract::read_entry_bytes(path.as_ref(), entry)
    }

    /// Read a single archive entry selected by raw archive path bytes into memory.
    pub fn read_entry_by_path(path: impl AsRef<Path>, entry: &[u8]) -> Result<Vec<u8>> {
        crate::extract::read_entry_bytes_by_path(path.as_ref(), entry)
    }

    /// Extract a single archive entry into a writer without materializing the whole payload.
    ///
    /// Entry path matching is case-insensitive and treats `\` as `/`.
    pub fn extract_entry_to_writer(
        path: impl AsRef<Path>,
        entry: &str,
        out: &mut dyn Write,
    ) -> Result<u64> {
        crate::extract::extract_entry_to_writer(path.as_ref(), entry, out)
    }

    /// Extract a single archive entry selected by raw archive path bytes into a writer.
    pub fn extract_entry_path_to_writer(
        path: impl AsRef<Path>,
        entry: &[u8],
        out: &mut dyn Write,
    ) -> Result<u64> {
        crate::extract::extract_entry_path_to_writer(path.as_ref(), entry, out)
    }

    /// Extract a single archive entry to disk according to `options`.
    pub fn extract(
        path: impl AsRef<Path>,
        entry: &str,
        options: &ExtractOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_entry(path.as_ref(), entry, options)
    }

    /// Extract a single archive entry selected by raw archive path bytes to disk.
    pub fn extract_by_path(
        path: impl AsRef<Path>,
        entry: &[u8],
        options: &ExtractOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_entry_by_path(path.as_ref(), entry, options)
    }

    /// Extract every archive entry to disk according to `options`.
    pub fn extract_all(
        path: impl AsRef<Path>,
        options: &ExtractAllOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_all(path.as_ref(), options)
    }

    /// Create a new archive from a file or directory.
    ///
    /// Returns the number of entries written. Existing output is replaced only after a successful
    /// write to a temporary file in the output directory.
    pub fn create(
        output: impl AsRef<Path>,
        input: impl AsRef<Path>,
        options: &CreateOptions,
    ) -> Result<usize> {
        crate::create::create_archive(output.as_ref(), input.as_ref(), options)
    }

    /// Add or replace entries by writing a new archive to `options.output`.
    ///
    /// The source archive is not modified in place. New inputs replace existing archive entries with
    /// the same archive path.
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
            "dream-archivetool-info-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder.add_bytes("meshes/example.nif", b"payload").unwrap();
        builder.write_path(&archive_path).unwrap();

        let info = ArchiveTool::info(&archive_path).unwrap();

        assert_eq!(info.format, ArchiveFormat::Tes3);
        assert_eq!(info.file_count, 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn open_archive_reuses_loaded_index() {
        let dir = std::env::temp_dir().join(format!(
            "dream-archivetool-open-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let mut builder = dream_archive::Tes3BsaBuilder::new();
        builder.add_bytes("icons/gold.dds", b"gold").unwrap();
        builder.write_path(&archive_path).unwrap();

        let archive = ArchiveTool::open(&archive_path).unwrap();

        assert_eq!(archive.info().file_count, 1);
        assert_eq!(archive.list().unwrap()[0].path, "icons/gold.dds");
        assert_eq!(archive.read_entry("icons/gold.dds").unwrap(), b"gold");

        fs::remove_dir_all(dir).unwrap();
    }
}
