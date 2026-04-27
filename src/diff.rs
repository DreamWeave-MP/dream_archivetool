use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::{archive_path_bytes_to_display, archive_path_bytes_to_hex};
use crate::{ArchiveError, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Options controlling archive comparison.
pub struct DiffOptions {
    /// Hash extracted payload bytes instead of comparing only listed metadata.
    pub hash_payloads: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Archive comparison report.
pub struct DiffReport {
    pub old: String,
    pub new: String,
    pub hash_payloads: bool,
    pub added: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
    pub changed: Vec<DiffChange>,
    pub unchanged: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Entry present on one side of an archive diff.
pub struct DiffEntry {
    pub path: String,
    pub path_bytes_hex: String,
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Entry present in both archives but with differing metadata or payload hash.
pub struct DiffChange {
    pub path: String,
    pub path_bytes_hex: String,
    pub old: DiffEntryState,
    pub new: DiffEntryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Side-specific state for a changed entry.
pub struct DiffEntryState {
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct DiffEntryData {
    path: Vec<u8>,
    size: Option<u64>,
    compressed_size: Option<u64>,
    payload_hash: Option<String>,
}

/// Compare two archives by normalized path bytes and metadata, optionally hashing payloads.
pub fn diff_archives(old: &Path, new: &Path, options: &DiffOptions) -> Result<DiffReport> {
    let old_archive = crate::loaded::LoadedArchive::open(old)?;
    let new_archive = crate::loaded::LoadedArchive::open(new)?;
    reject_unnameable_entries(&old_archive, old)?;
    reject_unnameable_entries(&new_archive, new)?;
    let old_entries = diff_entries(&old_archive, options.hash_payloads)?;
    let new_entries = diff_entries(&new_archive, options.hash_payloads)?;
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0;

    for (path, old_entry) in &old_entries {
        if let Some(new_entry) = new_entries.get(path) {
            if same_entry_state(old_entry, new_entry) {
                unchanged += 1;
            } else {
                changed.push(DiffChange {
                    path: archive_path_bytes_to_display(path),
                    path_bytes_hex: archive_path_bytes_to_hex(path),
                    old: entry_state(old_entry),
                    new: entry_state(new_entry),
                });
            }
        } else {
            removed.push(diff_entry(old_entry));
        }
    }
    for (path, new_entry) in &new_entries {
        if !old_entries.contains_key(path) {
            added.push(diff_entry(new_entry));
        }
    }

    Ok(DiffReport {
        old: old.display().to_string(),
        new: new.display().to_string(),
        hash_payloads: options.hash_payloads,
        added,
        removed,
        changed,
        unchanged,
    })
}

fn reject_unnameable_entries(archive: &crate::loaded::LoadedArchive, path: &Path) -> Result<()> {
    if archive.has_unnameable_entries() {
        return Err(ArchiveError::Archive(format!(
            "archive '{}' contains entries without recoverable paths; refusing to diff it lossy",
            path.display()
        )));
    }
    Ok(())
}

fn diff_entries(
    archive: &crate::loaded::LoadedArchive,
    hash_payloads: bool,
) -> Result<BTreeMap<Vec<u8>, DiffEntryData>> {
    let mut entries = BTreeMap::new();
    for entry in archive.list_loaded_entries()? {
        if entries.contains_key(&entry.path) {
            return Err(ArchiveError::Archive(format!(
                "archive contains duplicate normalized path: {}",
                archive_path_bytes_to_display(&entry.path)
            )));
        }
        let payload_hash = if hash_payloads {
            Some(payload_hash(archive, &entry.path)?)
        } else {
            None
        };
        entries.insert(
            entry.path.clone(),
            DiffEntryData {
                path: entry.path,
                size: entry.size,
                compressed_size: entry.compressed_size,
                payload_hash,
            },
        );
    }
    Ok(entries)
}

fn payload_hash(archive: &crate::loaded::LoadedArchive, path: &[u8]) -> Result<String> {
    let mut hasher = Fnv1a64Writer::default();
    archive.extract_normalized_entry_path_to_writer(path, &mut hasher)?;
    Ok(format!("{:016x}", hasher.finish()))
}

#[derive(Debug)]
struct Fnv1a64Writer {
    hash: u64,
}

impl Default for Fnv1a64Writer {
    fn default() -> Self {
        Self {
            hash: FNV1A64_OFFSET,
        }
    }
}

impl Fnv1a64Writer {
    fn finish(self) -> u64 {
        self.hash
    }
}

impl Write for Fnv1a64Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.hash = fnv1a64_update(self.hash, buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn same_entry_state(left: &DiffEntryData, right: &DiffEntryData) -> bool {
    left.size == right.size
        && left.compressed_size == right.compressed_size
        && left.payload_hash == right.payload_hash
}

fn diff_entry(entry: &DiffEntryData) -> DiffEntry {
    DiffEntry {
        path: archive_path_bytes_to_display(&entry.path),
        path_bytes_hex: archive_path_bytes_to_hex(&entry.path),
        size: entry.size,
        compressed_size: entry.compressed_size,
        payload_hash: entry.payload_hash.clone(),
    }
}

fn entry_state(entry: &DiffEntryData) -> DiffEntryState {
    DiffEntryState {
        size: entry.size,
        compressed_size: entry.compressed_size,
        payload_hash: entry.payload_hash.clone(),
    }
}

#[cfg(test)]
fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_update(FNV1A64_OFFSET, bytes)
}

const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv1a64_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    #[test]
    fn streaming_fnv_matches_whole_buffer_hash() {
        let mut writer = Fnv1a64Writer::default();
        writer.write_all(b"abc").unwrap();
        writer.write_all(b"def").unwrap();
        assert_eq!(writer.finish(), fnv1a64(b"abcdef"));
    }
}
