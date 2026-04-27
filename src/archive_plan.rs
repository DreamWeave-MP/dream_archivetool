//! Serializable archive mutation plans shared by create and add dry-runs.

use serde::{Deserialize, Serialize};

use crate::ArchiveFormat;

/// Plan for creating an archive without writing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatePlan {
    pub operation: ArchivePlanOperation,
    pub format: ArchiveFormat,
    pub output: String,
    pub files: usize,
    pub entries: Vec<ArchivePlanEntry>,
}

/// Plan for adding to an archive without writing it.
///
/// `entries` is a stable report order grouped by action, not a promise of final physical archive
/// ordering. Existing entries may be preserved in backend archive order when writing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddPlan {
    pub operation: ArchivePlanOperation,
    pub archive: String,
    pub output: String,
    pub format: ArchiveFormat,
    pub files: usize,
    pub added: usize,
    pub replaced: usize,
    pub preserved: usize,
    pub entries: Vec<ArchivePlanEntry>,
}

/// Archive mutation operation represented by a dry-run plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchivePlanOperation {
    Create,
    Add,
}

/// A single archive mutation planned action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivePlanEntry {
    pub action: ArchivePlanAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub path: String,
    pub path_bytes_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Planned archive mutation action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchivePlanAction {
    Add,
    Replace,
    Preserve,
}
