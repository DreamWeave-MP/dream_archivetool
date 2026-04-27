//! Serializable archive mutation plans shared by create and add dry-runs.

use serde::{Deserialize, Serialize};

use crate::ArchiveFormat;

/// Plan for creating an archive without writing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CreatePlan {
    /// Operation represented by this plan.
    pub operation: ArchivePlanOperation,
    /// Archive format that would be written.
    pub format: ArchiveFormat,
    /// Output archive path formatted for display.
    pub output: String,
    /// Number of entries that would be written.
    pub files: usize,
    /// Planned entry actions in stable report order.
    pub entries: Vec<ArchivePlanEntry>,
}

/// Plan for adding to an archive without writing it.
///
/// `entries` is a stable report order grouped by action, not a promise of final physical archive
/// ordering. Existing entries may be preserved in backend archive order when writing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AddPlan {
    /// Operation represented by this plan.
    pub operation: ArchivePlanOperation,
    /// Source archive path formatted for display.
    pub archive: String,
    /// Output archive path formatted for display.
    pub output: String,
    /// Archive format that would be written.
    pub format: ArchiveFormat,
    /// Total number of entries that would be written.
    pub files: usize,
    /// Number of new entries that would be added.
    pub added: usize,
    /// Number of existing entries that would be replaced.
    pub replaced: usize,
    /// Number of existing entries that would be preserved.
    pub preserved: usize,
    /// Planned entry actions in stable report order.
    pub entries: Vec<ArchivePlanEntry>,
}

/// Archive mutation operation represented by a dry-run plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchivePlanOperation {
    Create,
    Add,
}

/// A single archive mutation planned action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ArchivePlanEntry {
    /// Planned mutation action for this entry.
    pub action: ArchivePlanAction,
    /// Host source path for added/replaced entries, formatted for display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Archive path formatted for display.
    pub path: String,
    /// Hex-encoded normalized archive-path lookup key, not raw identity.
    pub path_bytes_hex: String,
    /// Source file size when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Planned archive mutation action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchivePlanAction {
    Add,
    Replace,
    Preserve,
}
