//! Shared archive rewrite capability checks.
//!
//! `info`, dry-run planning, and actual update operations must answer the same question: can this
//! archive be rewritten without known path loss or unsupported format semantics? Keep that contract
//! here rather than teaching every caller its own slightly different lie.

use crate::{ArchiveError, Result, loaded::LoadedArchive};

const UNNAMEABLE_ENTRIES_BLOCKER: &str =
    "archive contains entries without recoverable paths; refusing to rewrite it lossy";
const TES4_HASH_ONLY_BLOCKER: &str =
    "TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy";
const GNMF_BLOCKER: &str = "creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive";

/// Return the reason an archive cannot be safely rewritten, if this tool knows one.
pub(crate) fn rewrite_blocker(archive: &LoadedArchive) -> Option<&'static str> {
    if archive.has_unnameable_entries() {
        return Some(UNNAMEABLE_ENTRIES_BLOCKER);
    }
    match archive {
        LoadedArchive::Tes4(archive)
            if !tes4_has_recoverable_path_storage(archive.info().archive_flags) =>
        {
            Some(TES4_HASH_ONLY_BLOCKER)
        }
        LoadedArchive::Fo4(archive)
            if archive.info().format == dream_archive::ba2::PayloadFormat::GNMF =>
        {
            Some(GNMF_BLOCKER)
        }
        _ => None,
    }
}

/// Reject archives whose rewrite would be lossy or depend on unsupported format semantics.
pub(crate) fn ensure_rewritable(archive: &LoadedArchive) -> Result<()> {
    if let Some(blocker) = rewrite_blocker(archive) {
        return Err(ArchiveError::Archive(blocker.to_string()));
    }
    Ok(())
}

pub(crate) fn tes4_has_recoverable_path_storage(
    flags: dream_archive::bsa::tes4::ArchiveFlags,
) -> bool {
    let has_directory_strings =
        flags.contains(dream_archive::bsa::tes4::ArchiveFlags::DIRECTORY_STRINGS);
    let has_file_strings = flags.contains(dream_archive::bsa::tes4::ArchiveFlags::FILE_STRINGS);
    let has_embedded_names =
        flags.contains(dream_archive::bsa::tes4::ArchiveFlags::EMBEDDED_FILE_NAMES);
    (has_directory_strings && has_file_strings) || has_embedded_names
}
