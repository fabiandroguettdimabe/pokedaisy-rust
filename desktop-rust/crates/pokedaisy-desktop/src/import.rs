use crate::{inspect, library::LibraryEntry};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use zip::ZipArchive;

const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_ARCHIVE_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_LOOSE_ROM_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub rejected: usize,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
pub struct ArchiveProgress {
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub member: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ArchivePolicy {
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_cache_bytes: u64,
    pub max_entries: usize,
}

impl Default for ArchivePolicy {
    fn default() -> Self {
        Self {
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_bytes: MAX_ARCHIVE_TOTAL_BYTES,
            max_cache_bytes: 4 * 1024 * 1024 * 1024,
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
    import_archive_selected(
        entries,
        archive_path,
        library_dir,
        policy,
        None,
        None,
        |_| {},
    )
}

pub fn import_archive_members(
    entries: &mut Vec<LibraryEntry>,
    archive_path: &Path,
    library_dir: &Path,
    policy: ArchivePolicy,
    selected: &[String],
) -> ImportSummary {
    import_archive_selected(
        entries,
        archive_path,
        library_dir,
        policy,
        Some(selected),
        None,
        |_| {},
    )
}

pub fn import_archive_members_controlled(
    entries: &mut Vec<LibraryEntry>,
    archive_path: &Path,
    library_dir: &Path,
    policy: ArchivePolicy,
    selected: &[String],
    cancelled: &AtomicBool,
    on_progress: impl Fn(ArchiveProgress),
) -> ImportSummary {
    import_archive_selected(
        entries,
        archive_path,
        library_dir,
        policy,
        Some(selected),
        Some(cancelled),
        on_progress,
    )
}

pub fn archive_rom_members(
    archive_path: &Path,
    policy: ArchivePolicy,
) -> Result<Vec<String>, String> {
    let metadata = fs::metadata(archive_path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err("El archivo ZIP supera el límite comprimido".into());
    }
    let file = fs::File::open(archive_path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    if archive.len() > policy.max_entries {
        return Err("El ZIP contiene demasiadas entradas".into());
    }
    let mut total = 0u64;
    let mut members = Vec::new();
    for index in 0..archive.len() {
        let member = archive.by_index(index).map_err(|error| error.to_string())?;
        if member.is_dir() {
            continue;
        }
        let path = member
            .enclosed_name()
            .ok_or("El ZIP contiene una ruta no permitida")?;
        let is_rom = path
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
                return Err("Una ROM del ZIP supera el límite por archivo".into());
            }
            total = total.saturating_add(member.size());
            if total > policy.max_total_bytes {
                return Err("Las ROMs del ZIP superan la cuota total".into());
            }
            members.push(member.name().to_owned());
        }
    }
    Ok(members)
}

fn import_archive_selected(
    entries: &mut Vec<LibraryEntry>,
    archive_path: &Path,
    library_dir: &Path,
    policy: ArchivePolicy,
    selected: Option<&[String]>,
    cancelled: Option<&AtomicBool>,
    on_progress: impl Fn(ArchiveProgress),
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
        if is_rom && selected.is_some_and(|names| !names.iter().any(|name| name == member.name())) {
            continue;
        }
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
    let mut cached_bytes = 0u64;
    if library_dir.exists() {
        let Ok(cached_files) = fs::read_dir(library_dir) else {
            summary.rejected = 1;
            return summary;
        };
        for entry in cached_files.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                cached_bytes = cached_bytes.saturating_add(
                    entry
                        .metadata()
                        .map_or(policy.max_cache_bytes, |metadata| metadata.len()),
                );
            }
        }
    }
    if cached_bytes.saturating_add(preflight_total) > policy.max_cache_bytes {
        summary.rejected = 1;
        return summary;
    }
    if fs::create_dir_all(library_dir).is_err() {
        summary.rejected = 1;
        return summary;
    }
    let library_root = fs::canonicalize(library_dir).unwrap_or_else(|_| library_dir.to_path_buf());
    let mut total_declared = 0u64;
    let mut total_copied = 0u64;
    let entries_before = entries.len();
    let mut created_paths = Vec::new();
    for index in 0..archive.len() {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            summary.cancelled = true;
            break;
        }
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
        if selected.is_some_and(|names| !names.iter().any(|name| name == member.name())) {
            continue;
        }
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
                    on_progress(ArchiveProgress {
                        completed_bytes: total_copied,
                        total_bytes: preflight_total,
                        member: member.name().to_owned(),
                    });
                    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                        summary.cancelled = true;
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
            if !summary.cancelled {
                summary.rejected += 1;
            }
            if summary.cancelled {
                break;
            }
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
        created_paths.push(destination.clone());
        let canonical = destination.canonicalize().unwrap_or(destination);
        if entries.iter().any(|entry| entry.path == canonical) {
            summary.duplicates += 1;
        } else {
            entries.push(LibraryEntry::from_path(canonical));
            summary.imported += 1;
        }
    }
    if summary.rejected > 0 || summary.cancelled {
        entries.truncate(entries_before);
        for path in created_paths {
            let _ = fs::remove_file(path);
        }
        summary.imported = 0;
    }
    summary
}

pub fn import_paths(
    entries: &mut Vec<LibraryEntry>,
    paths: impl IntoIterator<Item = PathBuf>,
) -> ImportSummary {
    import_paths_with_limit(entries, paths, MAX_LOOSE_ROM_BYTES)
}

fn import_paths_with_limit(
    entries: &mut Vec<LibraryEntry>,
    paths: impl IntoIterator<Item = PathBuf>,
    max_bytes: u64,
) -> ImportSummary {
    let cancelled = AtomicBool::new(false);
    import_paths_controlled(entries, paths, max_bytes, &cancelled, |_| {})
}

pub fn import_paths_controlled(
    entries: &mut Vec<LibraryEntry>,
    paths: impl IntoIterator<Item = PathBuf>,
    max_bytes: u64,
    cancelled: &AtomicBool,
    on_progress: impl Fn(ArchiveProgress),
) -> ImportSummary {
    let mut summary = ImportSummary::default();
    let mut candidates = Vec::new();
    let mut total_bytes = 0u64;
    for path in paths {
        if cancelled.load(Ordering::Acquire) {
            summary.cancelled = true;
            return summary;
        }
        let Ok(metadata) = fs::metadata(&path) else {
            summary.rejected += 1;
            continue;
        };
        if !metadata.is_file()
            || metadata.len() > max_bytes
            || inspect::inspect_file(&path).is_none()
        {
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
        }) || candidates.iter().any(|(candidate, _)| candidate == &path);
        if duplicate {
            summary.duplicates += 1;
        } else {
            total_bytes = total_bytes.saturating_add(metadata.len());
            candidates.push((path, metadata.len()));
        }
    }
    let mut completed_bytes = 0u64;
    for (path, expected_size) in candidates {
        if cancelled.load(Ordering::Acquire) {
            summary.cancelled = true;
            break;
        }
        let Ok(mut file) = fs::File::open(&path) else {
            summary.rejected += 1;
            continue;
        };
        let mut digest = Sha256::new();
        let mut file_bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        let mut failed = false;
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    file_bytes = file_bytes.saturating_add(count as u64);
                    if file_bytes > max_bytes || file_bytes > expected_size {
                        failed = true;
                        break;
                    }
                    digest.update(&buffer[..count]);
                    completed_bytes = completed_bytes.saturating_add(count as u64);
                    on_progress(ArchiveProgress {
                        completed_bytes,
                        total_bytes,
                        member: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    });
                    if cancelled.load(Ordering::Acquire) {
                        summary.cancelled = true;
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
        if summary.cancelled {
            break;
        }
        if failed || file_bytes != expected_size {
            summary.rejected += 1;
            continue;
        }
        let hash = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        entries.push(LibraryEntry::from_path_with_hash(path, hash));
        summary.imported += 1;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::{
        ArchivePolicy, archive_rom_members, import_archive, import_archive_members,
        import_archive_members_controlled, import_paths, import_paths_controlled,
        import_paths_with_limit,
    };
    use crate::library::LibraryEntry;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
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
    fn loose_rom_over_quota_is_rejected_before_hashing() {
        let root = std::env::temp_dir().join(format!("vyntra-loose-quota-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("oversized.gba");
        fs::write(&path, valid_gba()).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(1024)
            .unwrap();
        let summary = import_paths_with_limit(&mut Vec::new(), [path], 512);
        assert_eq!(summary.rejected, 1);
        assert_eq!(summary.imported, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn loose_file_cancellation_during_hashing_does_not_add_partial_catalog_entry() {
        let root = std::env::temp_dir().join(format!("vyntra-loose-cancel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("game.gba");
        fs::write(&path, valid_gba()).unwrap();
        let cancelled = AtomicBool::new(false);
        let mut entries = Vec::new();

        let summary = import_paths_controlled(&mut entries, [path], 1024, &cancelled, |_| {
            cancelled.store(true, std::sync::atomic::Ordering::Release)
        });

        assert!(summary.cancelled);
        assert_eq!(entries.len(), 0);
        assert_eq!(summary.imported, 0);
        let _ = fs::remove_dir_all(root);
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
    fn archive_candidate_selection_imports_only_the_chosen_member() {
        let root = std::env::temp_dir().join(format!("vyntra-zip-choice-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("games.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("A/Pokemon.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        let mut other = valid_gba();
        other[0xa0] = b'B';
        writer
            .start_file("B/Pokemon.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&other).unwrap();
        writer.finish().unwrap();
        let members = archive_rom_members(&archive_path, ArchivePolicy::default()).unwrap();
        assert_eq!(members, vec!["A/Pokemon.gba", "B/Pokemon.gba"]);
        let selected = vec![members[1].clone()];

        let summary = import_archive_members(
            &mut Vec::new(),
            &archive_path,
            &root.join("library"),
            ArchivePolicy::default(),
            &selected,
        );

        assert_eq!(summary.imported, 1);
        assert_eq!(fs::read_dir(root.join("library")).unwrap().count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_cancellation_during_streaming_removes_partial_output() {
        let root = std::env::temp_dir().join(format!("vyntra-zip-cancel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("game.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("game.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        writer.finish().unwrap();
        let members = vec!["game.gba".to_owned()];
        let cancel = AtomicBool::new(false);

        let summary = import_archive_members_controlled(
            &mut Vec::new(),
            &archive_path,
            &root.join("library"),
            ArchivePolicy::default(),
            &members,
            &cancel,
            |_| cancel.store(true, std::sync::atomic::Ordering::Release),
        );

        assert!(summary.cancelled);
        assert_eq!(summary.imported, 0);
        assert_eq!(
            fs::read_dir(root.join("library"))
                .map(|items| items.count())
                .unwrap_or(0),
            0
        );
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
    fn archive_cache_quota_rejects_before_extracting() {
        let root =
            std::env::temp_dir().join(format!("vyntra-zip-cache-quota-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("one.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("game.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        writer.finish().unwrap();
        let cache = root.join("library");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("existing.gba"), [0u8; 10]).unwrap();
        let policy = ArchivePolicy {
            max_cache_bytes: 200,
            ..ArchivePolicy::default()
        };

        let summary = import_archive(&mut Vec::new(), &archive_path, &cache, policy);

        assert_eq!(summary.rejected, 1);
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
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

    #[test]
    fn corrupt_compressed_member_rolls_back_prior_roms_from_archive() {
        let root = std::env::temp_dir().join(format!("vyntra-zip-corrupt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("damaged.zip");
        let file = fs::File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("first.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&valid_gba()).unwrap();
        let mut second = valid_gba();
        second[0xa0] = b'B';
        writer
            .start_file("second.gba", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&second).unwrap();
        writer.finish().unwrap();

        let mut bytes = fs::read(&archive_path).unwrap();
        let name = b"second.gba";
        let name_at = bytes
            .windows(name.len())
            .position(|window| window == name)
            .unwrap();
        let header = name_at - 30;
        let name_len =
            u16::from_le_bytes(bytes[header + 26..header + 28].try_into().unwrap()) as usize;
        let extra_len =
            u16::from_le_bytes(bytes[header + 28..header + 30].try_into().unwrap()) as usize;
        let data_at = header + 30 + name_len + extra_len;
        bytes[data_at + 8] ^= 0x7f;
        fs::write(&archive_path, bytes).unwrap();
        let mut entries = Vec::new();

        let summary = import_archive(
            &mut entries,
            &archive_path,
            &root.join("library"),
            ArchivePolicy::default(),
        );

        assert_eq!(summary.rejected, 1);
        assert_eq!(summary.imported, 0);
        assert!(entries.is_empty());
        assert_eq!(
            fs::read_dir(root.join("library"))
                .map(|items| items.count())
                .unwrap_or(0),
            0
        );
        let _ = fs::remove_dir_all(root);
    }
}
