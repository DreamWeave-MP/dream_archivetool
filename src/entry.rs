use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
}

pub fn list_entries(_path: &Path) -> Result<Vec<ArchiveEntry>> {
    Ok(Vec::new())
}
