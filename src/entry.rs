use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
}

pub fn list_entries(path: &Path) -> Result<Vec<ArchiveEntry>> {
    Ok(crate::loaded::LoadedArchive::open(path)?.list_entries())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn lists_tes3_entries() {
        let dir = std::env::temp_dir().join(format!(
            "rome-archivetool-list-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.bsa");
        let archive: ba2::tes3::Archive = [(
            ba2::tes3::ArchiveKey::from(b"textures/example.dds".as_slice()),
            ba2::tes3::File::from(b"payload".as_slice()),
        )]
        .into_iter()
        .collect();
        let mut output = fs::File::create(&archive_path).unwrap();
        archive.write(&mut output).unwrap();

        let entries = list_entries(&archive_path).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "textures/example.dds");
        assert_eq!(entries[0].size, Some(7));

        fs::remove_dir_all(dir).unwrap();
    }
}
