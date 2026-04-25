use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverwriteMode {
    #[default]
    Fail,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    pub output: Option<PathBuf>,
    pub overwrite: OverwriteMode,
    pub preserve_paths: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ExtractAllOptions {
    pub output: Option<PathBuf>,
    pub overwrite: OverwriteMode,
}
