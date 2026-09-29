// SPDX-License-Identifier: GPL-3.0-or-later

//! Files the tool writes on the host: archives and extracted entries.

use std::io;
use std::path::Path;

use tempfile::NamedTempFile;

/// The directory a host file is in. A bare file name's `parent()` is the empty path, which no
/// file system call accepts; it means the current directory.
pub(crate) fn parent_directory(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// A temporary file in `directory`, to be written and then renamed over an output. It is
/// created the way any new file is, with 0666 less the umask; `NamedTempFile::new_in` would make
/// it 0600, and the rename keeps that, leaving every archive and extracted file private.
pub(crate) fn output_temp_file(directory: &Path) -> io::Result<NamedTempFile> {
    #[cfg(unix)]
    let mut builder = tempfile::Builder::new();
    #[cfg(not(unix))]
    let builder = tempfile::Builder::new();
    #[cfg(unix)]
    builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(0o666));
    builder.tempfile_in(directory)
}

/// Give `temp` the permissions of the file it is about to replace, so that updating an archive
/// in place leaves who may read it as it was.
#[cfg(unix)]
pub(crate) fn keep_permissions(temp: &NamedTempFile, replaced: &Path) -> io::Result<()> {
    temp.as_file()
        .set_permissions(std::fs::metadata(replaced)?.permissions())
}

/// Windows has only a read-only attribute, and a read-only archive cannot be replaced at all.
#[cfg(not(unix))]
pub(crate) fn keep_permissions(_temp: &NamedTempFile, _replaced: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_file_name_is_in_the_current_directory() {
        assert_eq!(parent_directory(Path::new("MyMod.bsa")), Path::new("."));
        assert_eq!(
            parent_directory(Path::new("out/MyMod.bsa")),
            Path::new("out")
        );
        assert_eq!(parent_directory(Path::new("/")), Path::new("."));
    }
}
