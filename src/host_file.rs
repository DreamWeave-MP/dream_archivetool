// SPDX-License-Identifier: GPL-3.0-or-later

//! Files the tool writes on the host: archives and extracted entries.

use std::path::Path;

/// The directory a host file is in. A bare file name's `parent()` is the empty path, which no
/// file system call accepts; it means the current directory.
pub(crate) fn parent_directory(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
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
