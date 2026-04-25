use std::path::PathBuf;

use crate::ArchiveFormat;

#[derive(Debug, Clone)]
pub struct CreateOptions {
    pub format: ArchiveFormat,
}

#[derive(Debug, Clone, Default)]
pub struct AddOptions {
    pub inputs: Vec<PathBuf>,
}
