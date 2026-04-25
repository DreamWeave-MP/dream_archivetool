use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{ArchiveEntry, ArchiveFormat, Result};

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
        let format = Self::guess_format(path)?;
        let file_count = Self::list(path)?.len();
        Ok(ArchiveInfo {
            path: path.display().to_string(),
            format,
            file_count,
        })
    }

    pub fn list(path: impl AsRef<Path>) -> Result<Vec<ArchiveEntry>> {
        crate::entry::list_entries(path.as_ref())
    }
}
