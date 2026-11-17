use crate::{inspect, library::LibraryEntry};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub rejected: usize,
}

pub fn import_paths(
    entries: &mut Vec<LibraryEntry>,
    paths: impl IntoIterator<Item = PathBuf>,
) -> ImportSummary {
    let mut summary = ImportSummary::default();
    for path in paths {
        if !path.is_file() || inspect::inspect_file(&path).is_none() {
            summary.rejected += 1;
            continue;
        }
        let Ok(path) = path.canonicalize() else {
            summary.rejected += 1;
            continue;
        };
        let duplicate = entries.iter().any(|entry| {
            entry
                .path
                .canonicalize()
                .unwrap_or_else(|_| entry.path.clone())
                == path
        });
        if duplicate {
            summary.duplicates += 1;
        } else {
            entries.push(LibraryEntry::from_path(path));
            summary.imported += 1;
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::import_paths;
    use crate::library::LibraryEntry;
    use std::fs;
    use std::path::PathBuf;

    fn valid_gba() -> Vec<u8> {
        let mut bytes = vec![0u8; 0xc0];
        bytes[0xb2] = 0x96;
        bytes[0xac..0xb0].copy_from_slice(b"ABCD");
        bytes[0xbc] = 0x19;
        bytes
    }

    #[test]
    fn multi_file_import_reports_valid_invalid_and_duplicate_paths() {
        let root = std::env::temp_dir().join(format!("vyntra-import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let valid = root.join("valid.gba");
        let invalid = root.join("invalid.gba");
        fs::write(&valid, valid_gba()).unwrap();
        fs::write(&invalid, b"not a rom").unwrap();
        let mut entries = Vec::new();

        let summary = import_paths(&mut entries, [valid.clone(), invalid, valid.clone()]);

        assert_eq!(summary.imported, 1);
        assert_eq!(summary.duplicates, 1);
        assert_eq!(summary.rejected, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].path,
            PathBuf::from(&valid.canonicalize().unwrap())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_skips_nonexistent_rom_paths() {
        let mut entries: Vec<LibraryEntry> = Vec::new();
        let summary = import_paths(&mut entries, [PathBuf::from("missing.gba")]);
        assert_eq!(summary.imported, 0);
        assert_eq!(summary.rejected, 1);
    }
}
