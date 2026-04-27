use std::fs;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::{ArchiveError, Result};

pub(super) fn with_temp_output(
    output: &Path,
    fsync: bool,
    write: impl FnOnce(&mut fs::File) -> Result<()>,
) -> Result<()> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let temp = write_temp_output(parent, fsync, write)?;
    persist_temp_output(temp, output, fsync)
}

pub(super) fn write_temp_output(
    parent: &Path,
    fsync: bool,
    write: impl FnOnce(&mut fs::File) -> Result<()>,
) -> Result<NamedTempFile> {
    let mut temp = NamedTempFile::new_in(parent)?;
    write(temp.as_file_mut())?;
    if fsync {
        temp.as_file_mut().sync_all()?;
    }
    Ok(temp)
}

pub(super) fn persist_temp_output(temp: NamedTempFile, output: &Path, fsync: bool) -> Result<()> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    temp.persist(output)
        .map_err(|err| ArchiveError::Io(err.error))?;
    if fsync {
        sync_parent_dir(parent)?;
    }
    Ok(())
}

fn sync_parent_dir(parent: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}
