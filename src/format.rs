use std::fs::File;
use std::path::Path;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::{ArchiveError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ArchiveFormat {
    Tes3,
    Tes4,
    Fo4,
}

pub fn guess_format(path: &Path) -> Result<ArchiveFormat> {
    let mut file = File::open(path)?;
    let format = ba2::guess_format(&mut file).ok_or(ArchiveError::UnknownFormat)?;
    match format {
        ba2::FileFormat::TES3 => Ok(ArchiveFormat::Tes3),
        ba2::FileFormat::TES4 => Ok(ArchiveFormat::Tes4),
        ba2::FileFormat::FO4 => Ok(ArchiveFormat::Fo4),
    }
}
