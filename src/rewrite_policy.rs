// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared archive rewrite capability checks.
//!
//! `info`, dry-run planning, and actual update operations must answer the same question: can this
//! archive be rewritten without known path loss or unsupported format semantics? Keep that contract
//! here rather than teaching every caller its own slightly different lie.

use crate::{ArchiveError, Result, loaded::LoadedArchiveRef};

const UNNAMEABLE_ENTRIES_BLOCKER: &str =
    "archive contains entries without recoverable paths; refusing to rewrite it lossy";
const TES4_HASH_ONLY_BLOCKER: &str =
    "TES4 hash-only archives do not have recoverable path names; refusing to rewrite them lossy";
const TES4_UNSUPPORTED_FLAGS_BLOCKER: &str =
    "TES4 archive uses header flag bits this tool cannot preserve; refusing to rewrite it lossy";
pub(crate) const GNMF_BLOCKER: &str = "creating or updating GNMF BA2 archives requires console texture swizzle semantics and is not supported by dream_archive";
const TES4_REWRITABLE_ARCHIVE_FLAGS: u32 = dream_archive::bsa::tes4::ArchiveFlags::DIRECTORY_STRINGS
    .bits()
    | dream_archive::bsa::tes4::ArchiveFlags::FILE_STRINGS.bits()
    | dream_archive::bsa::tes4::ArchiveFlags::COMPRESSED.bits()
    | dream_archive::bsa::tes4::ArchiveFlags::EMBEDDED_FILE_NAMES.bits();

/// Return the reason an archive cannot be safely rewritten, if this tool knows one.
pub(crate) fn rewrite_blocker(archive: LoadedArchiveRef<'_>) -> Option<&'static str> {
    if archive.has_unnameable_entries() {
        return Some(UNNAMEABLE_ENTRIES_BLOCKER);
    }
    match archive.as_dream_archive() {
        dream_archive::Archive::Tes4Bsa(archive)
            if !tes4_has_recoverable_path_storage(archive.info().archive_flags) =>
        {
            Some(TES4_HASH_ONLY_BLOCKER)
        }
        dream_archive::Archive::Tes4Bsa(archive)
            if tes4_unsupported_archive_flag_bits(archive.info().archive_flags) != 0 =>
        {
            Some(TES4_UNSUPPORTED_FLAGS_BLOCKER)
        }
        dream_archive::Archive::BA2(archive)
            if archive.info().format == dream_archive::ba2::PayloadFormat::GNMF =>
        {
            Some(GNMF_BLOCKER)
        }
        _ => None,
    }
}

pub(crate) fn ensure_ba2_payload_format_writable(
    format: dream_archive::ba2::PayloadFormat,
) -> Result<()> {
    if format == dream_archive::ba2::PayloadFormat::GNMF {
        return Err(ArchiveError::Archive(GNMF_BLOCKER.to_string()));
    }
    Ok(())
}

/// Reject archives whose rewrite would be lossy or depend on unsupported format semantics.
pub(crate) fn ensure_rewritable(archive: LoadedArchiveRef<'_>) -> Result<()> {
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

pub(crate) fn tes4_unsupported_archive_flag_bits(
    flags: dream_archive::bsa::tes4::ArchiveFlags,
) -> u32 {
    flags.bits() & !TES4_REWRITABLE_ARCHIVE_FLAGS
}
