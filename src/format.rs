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
    let mut file = File::open(path).map_err(|err| {
        ArchiveError::Archive(format!(
            "failed to open archive '{}': {err}",
            path.display()
        ))
    })?;
    let format = dream_archive::guess_format(&mut file)
        .map_err(|err| {
            ArchiveError::Archive(format!(
                "failed to read archive header '{}': {err}",
                path.display()
            ))
        })?
        .ok_or(ArchiveError::UnknownFormat)?;
    match format {
        dream_archive::FileFormat::BSA(dream_archive::BsaFormat::TES3) => Ok(ArchiveFormat::Tes3),
        dream_archive::FileFormat::BSA(dream_archive::BsaFormat::TES4) => Ok(ArchiveFormat::Tes4),
        dream_archive::FileFormat::BA2 => Ok(ArchiveFormat::Fo4),
    }
}
