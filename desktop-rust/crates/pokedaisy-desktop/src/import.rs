use crate::{inspect, library::LibraryEntry};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub rejected: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ArchivePolicy {
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_entries: usize,
}

impl Default for ArchivePolicy {
    fn default() -> Self {
        Self {
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_bytes: MAX_ARCHIVE_TOTAL_BYTES,
            max_entries: MAX_ARCHIVE_ENTRIES,
        }
    }
}

/// Extract valid ROM members into content-addressed files. Archive paths are
/// never used as output paths; only the validated extension is retained.
pub fn import_archive(
    entries: &mut Vec<LibraryEntry>,
    archive_path: &Path,
    library_dir: &Path,
    policy: ArchivePolicy,
) -> ImportSummary {
    let mut summary = ImportSummary::default();
    let Ok(metadata) = fs::metadata(archive_path) else {
        summary.rejected = 1;
        return summary;
    };
    if metadata.len() > MAX_ARCHIVE_BYTES {
        summary.rejected = 1;
        return summary;
    }
    let Ok(file) = fs::File::open(archive_path) else {
        summary.rejected = 1;
        return summary;
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        summary.rejected = 1;
        return summary;
    };
    if archive.len() > policy.max_entries {
        summary.rejected = 1;
        return summary;
    }
    let mut preflight_total = 0u64;
    for index in 0..archive.len() {
        let Ok(member) = archive.by_index(index) else {
            summary.rejected = 1;
            return summary;
        };
        if member.is_dir() {
            continue;
        }
        let Some(member_path) = member.enclosed_name() else {
            summary.rejected = 1;
            return summary;
        };
        let is_rom = member_path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "gb" | "gbc" | "gba" | "nds"
                )
            });
        if is_rom {
            if member.size() > policy.max_entry_bytes {
                summary.rejected = 1;
                return summary;
            }
            preflight_total = preflight_total.saturating_add(member.size());
            if preflight_total > policy.max_total_bytes {
                summary.rejected = 1;
                return summary;
            }
        }
    }
    if fs::create_dir_all(library_dir).is_err() {
        summary.rejected = 1;
        return summary;
    }
    let library_root = fs::canonicalize(library_dir).unwrap_or_else(|_| library_dir.to_path_buf());
    let mut total_declared = 0u64;
    let mut total_copied = 0u64;
    for index in 0..archive.len() {
        let Ok(mut member) = archive.by_index(index) else {
            summary.rejected += 1;
            continue;
        };
        if member.is_dir() {
            continue;
        }
        let Some(member_path) = member.enclosed_name() else {
            summary.rejected += 1;
            continue;
        };
        if member.size() > policy.max_entry_bytes {
            summary.rejected += 1;
            continue;
        }
        let Some(ext) = member_path.extension().and_then(|value| value.to_str()) else {
            summary.rejected += 1;
            continue;
        };
        if !matches!(
            ext.to_ascii_lowercase().as_str(),
            "gb" | "gbc" | "gba" | "nds"
        ) {
            continue;
        }
        total_declared = total_declared.saturating_add(member.size());
        if total_declared > policy.max_total_bytes {
            summary.rejected += 1;
            continue;
        }
        let extension = ext.to_ascii_lowercase();
        let temp_path = library_root.join(format!(
            ".extracting-{}-{index}.{extension}",
            std::process::id()
        ));
        let Ok(mut output) = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        else {
            summary.rejected += 1;
            continue;
        };
        let mut digest = Sha256::new();
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        let mut failed = false;
        loop {
            match member.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    copied = copied.saturating_add(count as u64);
                    total_copied = total_copied.saturating_add(count as u64);
                    if copied > policy.max_entry_bytes
                        || copied > member.size()
                        || total_copied > policy.max_total_bytes
                    {
                        failed = true;
                        break;
                    }
                    digest.update(&buffer[..count]);
                    if output.write_all(&buffer[..count]).is_err() {
                        failed = true;
                        break;
                    }
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        if failed || copied != member.size() || output.sync_all().is_err() {
            drop(output);
            let _ = fs::remove_file(&temp_path);
            summary.rejected += 1;
            continue;
        }
        drop(output);
        if inspect::inspect_file(&temp_path).is_none() {
            let _ = fs::remove_file(&temp_path);
            summary.rejected += 1;
            continue;
        }
        let hash = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let destination = library_root.join(format!("{hash}.{extension}"));
        if destination.is_file() {
            let _ = fs::remove_file(&temp_path);
            if crate::identity::sha256_file(&destination).is_ok_and(|existing| existing == hash) {
                summary.duplicates += 1;
            } else {
                summary.rejected += 1;
            }
            continue;
        }
        if fs::rename(&temp_path, &destination).is_err() {
            let _ = fs::remove_file(&temp_path);
            summary.rejected += 1;
            continue;
        }
        let canonical = destination.canonicalize().unwrap_or(destination);
        if entries.iter().any(|entry| entry.path == canonical) {
            summary.duplicates += 1;
        } else {
            entries.push(LibraryEntry::from_path(canonical));
            summary.imported += 1;
        }
    }
    summary
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
    use super::{ArchivePolicy, import_archive, import_paths};
    use crate::library::LibraryEntry;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use zip::write::SimpleFileOptions;

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

    #[test]
    fn archive_extracts_only_valid_roms_with_generated_names() {
        let root = std::env::temp_dir().join(format!("vyntra-zip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("games.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("folder/Pokemon.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        writer
            .start_file("notes.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"ignore").unwrap();
        writer.finish().unwrap();

        let mut entries = Vec::new();
        let summary = import_archive(
            &mut entries,
            &archive_path,
            &root.join("library"),
            ArchivePolicy::default(),
        );

        assert_eq!(summary.imported, 1);
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .path
                .starts_with(root.join("library").canonicalize().unwrap())
        );
        assert_eq!(fs::read(&entries[0].path).unwrap(), valid_gba());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_rejects_declared_entry_over_quota_without_extracting_it() {
        let root = std::env::temp_dir().join(format!("vyntra-zip-quota-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("large.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("large.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&vec![0u8; 128]).unwrap();
        writer.finish().unwrap();
        let policy = ArchivePolicy {
            max_entry_bytes: 64,
            ..ArchivePolicy::default()
        };

        let summary = import_archive(
            &mut Vec::new(),
            &archive_path,
            &root.join("library"),
            policy,
        );

        assert_eq!(summary.rejected, 1);
        assert_eq!(
            fs::read_dir(root.join("library"))
                .map(|items| items.count())
                .unwrap_or(0),
            0
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_quota_failure_does_not_leave_an_earlier_rom_imported() {
        let root = std::env::temp_dir().join(format!("vyntra-zip-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("mixed.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("first.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        writer
            .start_file("too-large.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&vec![0u8; 128]).unwrap();
        writer.finish().unwrap();
        let mut entries = Vec::new();
        let policy = ArchivePolicy {
            max_entry_bytes: 200,
            max_total_bytes: 256,
            ..ArchivePolicy::default()
        };

        let summary = import_archive(&mut entries, &archive_path, &root.join("library"), policy);

        assert_eq!(summary.imported, 0);
        assert_eq!(entries.len(), 0);
        assert_eq!(
            fs::read_dir(root.join("library"))
                .map(|items| items.count())
                .unwrap_or(0),
            0
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_rejects_traversal_members_before_writing_any_rom() {
        let root =
            std::env::temp_dir().join(format!("vyntra-zip-traversal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("hostile.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../outside.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        writer.finish().unwrap();
        let mut entries = Vec::new();

        let summary = import_archive(
            &mut entries,
            &archive_path,
            &root.join("library"),
            ArchivePolicy::default(),
        );

        assert_eq!(summary.imported, 0);
        assert_eq!(summary.rejected, 1);
        assert!(!root.join("outside.gba").exists());
        assert_eq!(
            fs::read_dir(root.join("library"))
                .map(|items| items.count())
                .unwrap_or(0),
            0
        );
        let _ = fs::remove_dir_all(root);
    }
}
