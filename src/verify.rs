use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::{
    archive_path_bytes_to_display, archive_path_bytes_to_hex, safe_target_path_normalized,
};
use crate::{ArchiveFormat, ArchiveTool, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Options controlling archive verification.
pub struct VerifyOptions {
    /// Stream every named payload to a sink to check that extraction succeeds.
    pub read_payloads: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Archive verification report suitable for CLI and GUI callers.
pub struct VerifyReport {
    pub path: String,
    pub format: ArchiveFormat,
    pub file_count: usize,
    pub named_entry_count: usize,
    pub unnameable_entries: usize,
    pub rewritable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_blocker: Option<String>,
    pub duplicate_normalized_paths: Vec<VerifyPathIssue>,
    pub unsafe_paths: Vec<VerifyPathIssue>,
    pub payloads_read: Option<usize>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Path-level issue reported by verification.
pub struct VerifyPathIssue {
    pub path: String,
    pub path_bytes_hex: String,
}

/// Verify archive index health and, optionally, payload readability.
pub fn verify_archive(path: &Path, options: &VerifyOptions) -> Result<VerifyReport> {
    let info = ArchiveTool::info(path)?;
    let archive = crate::loaded::LoadedArchive::open(path)?;
    let entries = archive.list_loaded_entries()?;
    let mut seen = BTreeSet::new();
    let mut duplicate_normalized_paths = Vec::new();
    let mut unsafe_paths = Vec::new();

    for entry in &entries {
        if !seen.insert(entry.path.clone()) {
            duplicate_normalized_paths.push(path_issue(&entry.path));
        }
        if safe_target_path_normalized(Path::new("."), &entry.path).is_err() {
            unsafe_paths.push(path_issue(&entry.path));
        }
    }

    let payloads_read = if options.read_payloads {
        let mut sink = io::sink();
        for entry in &entries {
            archive.extract_normalized_entry_path_to_writer(&entry.path, &mut sink)?;
        }
        Some(entries.len())
    } else {
        None
    };

    let mut warnings = Vec::new();
    if info.has_unnameable_entries {
        warnings.push("archive contains entries without recoverable path names".to_string());
    }
    if !duplicate_normalized_paths.is_empty() {
        warnings.push("archive contains duplicate normalized paths".to_string());
    }
    if !unsafe_paths.is_empty() {
        warnings.push("archive contains paths unsafe to extract directly".to_string());
    }
    if info
        .ba2
        .as_ref()
        .is_some_and(|ba2| ba2.payload_format == "dx10")
    {
        warnings.push("BA2 DX10 rewrite may buffer preserved texture entries".to_string());
    }

    Ok(VerifyReport {
        path: info.path,
        format: info.format,
        file_count: info.file_count,
        named_entry_count: info.named_entry_count,
        unnameable_entries: info.file_count.saturating_sub(info.named_entry_count),
        rewritable: info.rewritable,
        rewrite_blocker: info.rewrite_blocker,
        duplicate_normalized_paths,
        unsafe_paths,
        payloads_read,
        warnings,
    })
}

fn path_issue(path: &[u8]) -> VerifyPathIssue {
    VerifyPathIssue {
        path: archive_path_bytes_to_display(path),
        path_bytes_hex: archive_path_bytes_to_hex(path),
    }
}
