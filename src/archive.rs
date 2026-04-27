use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    AddOptions, AddPlan, ArchiveEntry, ArchiveFormat, CreateOptions, CreatePlan, DiffOptions,
    DiffReport, ExtractAllOptions, ExtractAllPlan, ExtractOptions, ExtractSummary, Result,
    VerifyOptions, VerifyReport,
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
    /// Number of entries with recoverable path names.
    pub named_entry_count: usize,
    /// Whether any entries do not have recoverable path names.
    pub has_unnameable_entries: bool,
    /// Whether this tool can rewrite the archive without known lossy behavior.
    pub rewritable: bool,
    /// Explanation when `rewritable` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_blocker: Option<String>,
    /// TES4-family metadata when `format` is [`ArchiveFormat::Tes4`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tes4: Option<Tes4Info>,
    /// BA2 metadata when `format` is [`ArchiveFormat::Ba2`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ba2: Option<Ba2Info>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// TES4-family archive metadata that affects rewrite behavior.
pub struct Tes4Info {
    pub version: String,
    pub archive_types: String,
    pub archive_flags: Vec<String>,
    pub name_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// BA2 archive metadata that affects rewrite behavior.
pub struct Ba2Info {
    pub version: String,
    pub payload_format: String,
    pub compression_format: String,
    pub strings: bool,
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
        archive_info(&self.path, &self.archive)
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

    /// Read a single archive entry selected by normalized archive path bytes into memory.
    pub fn read_entry_by_path_bytes(&self, entry: &[u8]) -> Result<Vec<u8>> {
        self.archive.read_entry_bytes_by_path(entry)
    }

    /// Read a single archive entry selected by normalized archive path bytes into memory.
    ///
    /// Prefer [`Self::read_entry_by_path_bytes`] in new code; this name predates the explicit
    /// distinction between display paths, filesystem paths, and archive path bytes.
    pub fn read_entry_by_path(&self, entry: &[u8]) -> Result<Vec<u8>> {
        self.read_entry_by_path_bytes(entry)
    }

    /// Extract a single archive entry into a writer without materializing the whole payload.
    pub fn extract_entry_to_writer(&self, entry: &str, out: &mut dyn Write) -> Result<u64> {
        self.archive.extract_entry_to_writer(entry, out)
    }

    /// Extract a single archive entry selected by normalized archive path bytes into a writer.
    pub fn extract_entry_by_path_bytes_to_writer(
        &self,
        entry: &[u8],
        out: &mut dyn Write,
    ) -> Result<u64> {
        self.archive.extract_entry_path_to_writer(entry, out)
    }

    /// Extract a single archive entry selected by normalized archive path bytes into a writer.
    ///
    /// Prefer [`Self::extract_entry_by_path_bytes_to_writer`] in new code; this name predates the
    /// explicit byte-path API naming convention.
    pub fn extract_entry_path_to_writer(&self, entry: &[u8], out: &mut dyn Write) -> Result<u64> {
        self.extract_entry_by_path_bytes_to_writer(entry, out)
    }
}

fn archive_info(path: &str, archive: &crate::loaded::LoadedArchive) -> ArchiveInfo {
    let rewrite_blocker = crate::rewrite_policy::rewrite_blocker(archive).map(str::to_string);
    ArchiveInfo {
        path: path.to_string(),
        format: archive.format(),
        file_count: archive.file_count(),
        named_entry_count: archive.named_entry_count(),
        has_unnameable_entries: archive.has_unnameable_entries(),
        rewritable: rewrite_blocker.is_none(),
        rewrite_blocker,
        tes4: tes4_info(archive),
        ba2: ba2_info(archive),
    }
}

fn tes4_info(archive: &crate::loaded::LoadedArchive) -> Option<Tes4Info> {
    let dream_archive::Archive::Tes4Bsa(archive) = archive.as_dream_archive() else {
        return None;
    };
    let info = archive.info();
    let flags = info.archive_flags;
    let directory_strings =
        flags.contains(dream_archive::bsa::tes4::ArchiveFlags::DIRECTORY_STRINGS);
    let file_strings = flags.contains(dream_archive::bsa::tes4::ArchiveFlags::FILE_STRINGS);
    let embedded_file_names =
        flags.contains(dream_archive::bsa::tes4::ArchiveFlags::EMBEDDED_FILE_NAMES);
    Some(Tes4Info {
        version: format!("{:?}", info.version),
        archive_types: format!("{:?}", info.archive_types),
        archive_flags: tes4_archive_flags(flags),
        name_mode: tes4_name_mode(directory_strings, file_strings, embedded_file_names).to_string(),
    })
}

fn tes4_archive_flags(flags: dream_archive::bsa::tes4::ArchiveFlags) -> Vec<String> {
    let mut names = Vec::new();
    if flags.contains(dream_archive::bsa::tes4::ArchiveFlags::DIRECTORY_STRINGS) {
        names.push("directory-strings".to_string());
    }
    if flags.contains(dream_archive::bsa::tes4::ArchiveFlags::FILE_STRINGS) {
        names.push("file-strings".to_string());
    }
    if flags.contains(dream_archive::bsa::tes4::ArchiveFlags::COMPRESSED) {
        names.push("compressed".to_string());
    }
    if flags.contains(dream_archive::bsa::tes4::ArchiveFlags::EMBEDDED_FILE_NAMES) {
        names.push("embedded-file-names".to_string());
    }
    names
}

fn tes4_name_mode(
    directory_strings: bool,
    file_strings: bool,
    embedded_file_names: bool,
) -> &'static str {
    match (directory_strings && file_strings, embedded_file_names) {
        (true, true) => "strings-and-embedded",
        (true, false) => "strings",
        (false, true) => "embedded",
        (false, false) => "hash-only",
    }
}

fn ba2_info(archive: &crate::loaded::LoadedArchive) -> Option<Ba2Info> {
    let dream_archive::Archive::BA2(archive) = archive.as_dream_archive() else {
        return None;
    };
    let info = archive.info();
    Some(Ba2Info {
        version: format!("{:?}", info.version),
        payload_format: format!("{:?}", info.format).to_ascii_lowercase(),
        compression_format: format!("{:?}", info.compression_format).to_ascii_lowercase(),
        strings: info.strings,
    })
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
        Ok(archive_info(&path.display().to_string(), &archive))
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

    /// Read a single archive entry selected by normalized archive path bytes into memory.
    pub fn read_entry_by_path_bytes(path: impl AsRef<Path>, entry: &[u8]) -> Result<Vec<u8>> {
        crate::extract::read_entry_bytes_by_path(path.as_ref(), entry)
    }

    /// Read a single archive entry selected by normalized archive path bytes into memory.
    ///
    /// Prefer [`Self::read_entry_by_path_bytes`] in new code; this name predates the explicit
    /// distinction between display paths, filesystem paths, and archive path bytes.
    pub fn read_entry_by_path(path: impl AsRef<Path>, entry: &[u8]) -> Result<Vec<u8>> {
        Self::read_entry_by_path_bytes(path, entry)
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

    /// Extract a single archive entry selected by normalized archive path bytes into a writer.
    pub fn extract_entry_by_path_bytes_to_writer(
        path: impl AsRef<Path>,
        entry: &[u8],
        out: &mut dyn Write,
    ) -> Result<u64> {
        crate::extract::extract_entry_path_to_writer(path.as_ref(), entry, out)
    }

    /// Extract a single archive entry selected by normalized archive path bytes into a writer.
    ///
    /// Prefer [`Self::extract_entry_by_path_bytes_to_writer`] in new code; this name predates the
    /// explicit byte-path API naming convention.
    pub fn extract_entry_path_to_writer(
        path: impl AsRef<Path>,
        entry: &[u8],
        out: &mut dyn Write,
    ) -> Result<u64> {
        Self::extract_entry_by_path_bytes_to_writer(path, entry, out)
    }

    /// Extract a single archive entry to disk according to `options`.
    pub fn extract(
        path: impl AsRef<Path>,
        entry: &str,
        options: &ExtractOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_entry(path.as_ref(), entry, options)
    }

    /// Extract a single archive entry selected by normalized archive path bytes to disk.
    pub fn extract_by_path_bytes(
        path: impl AsRef<Path>,
        entry: &[u8],
        options: &ExtractOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_entry_by_path(path.as_ref(), entry, options)
    }

    /// Extract a single archive entry selected by normalized archive path bytes to disk.
    ///
    /// Prefer [`Self::extract_by_path_bytes`] in new code; this name predates the explicit
    /// byte-path API naming convention.
    pub fn extract_by_path(
        path: impl AsRef<Path>,
        entry: &[u8],
        options: &ExtractOptions,
    ) -> Result<ExtractSummary> {
        Self::extract_by_path_bytes(path, entry, options)
    }

    /// Extract every archive entry to disk according to `options`.
    pub fn extract_all(
        path: impl AsRef<Path>,
        options: &ExtractAllOptions,
    ) -> Result<ExtractSummary> {
        crate::extract::extract_all(path.as_ref(), options)
    }

    /// Plan full archive extraction without writing files.
    pub fn plan_extract_all(
        path: impl AsRef<Path>,
        options: &ExtractAllOptions,
    ) -> Result<ExtractAllPlan> {
        crate::extract::plan_extract_all(path.as_ref(), options)
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

    /// Plan archive creation without writing output.
    pub fn plan_create(
        output: impl AsRef<Path>,
        input: impl AsRef<Path>,
        options: &CreateOptions,
    ) -> Result<CreatePlan> {
        crate::create::plan_create_archive(output.as_ref(), input.as_ref(), options)
    }

    /// Add or replace entries by writing a new archive to `options.output`.
    ///
    /// The source archive is not modified in place. New inputs replace existing archive entries with
    /// the same archive path.
    pub fn add(path: impl AsRef<Path>, options: &AddOptions) -> Result<usize> {
        crate::create::add_to_archive(path.as_ref(), options)
    }

    /// Plan archive add/update without writing output.
    pub fn plan_add(path: impl AsRef<Path>, options: &AddOptions) -> Result<AddPlan> {
        crate::create::plan_add_to_archive(path.as_ref(), options)
    }

    /// Verify archive index health and, optionally, payload readability.
    pub fn verify(path: impl AsRef<Path>, options: &VerifyOptions) -> Result<VerifyReport> {
        crate::verify::verify_archive(path.as_ref(), options)
    }

    /// Compare two archives by normalized path bytes and optional payload hashes.
    pub fn diff(
        old: impl AsRef<Path>,
        new: impl AsRef<Path>,
        options: &DiffOptions,
    ) -> Result<DiffReport> {
        crate::diff::diff_archives(old.as_ref(), new.as_ref(), options)
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
