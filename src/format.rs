use std::fs::File;
use std::path::Path;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::{ArchiveError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
/// Supported Bethesda archive families.
#[serde(rename_all = "kebab-case")]
pub enum ArchiveFormat {
    /// Morrowind-era BSA archives.
    Tes3,
    /// Oblivion/Fallout 3/Skyrim-era BSA archives.
    Tes4,
    /// Fallout 4/Starfield BA2 archives.
    Fo4,
}

/// Detect an archive format from its file header.
pub fn guess_format(path: &Path) -> Result<ArchiveFormat> {
    let mut file = File::open(path)?;
    let format = ba2::guess_format(&mut file).ok_or(ArchiveError::UnknownFormat)?;
    match format {
        ba2::FileFormat::TES3 => Ok(ArchiveFormat::Tes3),
        ba2::FileFormat::TES4 => Ok(ArchiveFormat::Tes4),
        ba2::FileFormat::FO4 => Ok(ArchiveFormat::Fo4),
    }
}
