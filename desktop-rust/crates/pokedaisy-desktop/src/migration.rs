//! Non-destructive, journaled migration for legacy save artifacts.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);
const MAX_MIGRATION_BYTES: u64 = 512 * 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationItem {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub game_id: String,
    pub kind: String,
    pub sha256: String,
    pub bytes: u64,
}

impl MigrationItem {
    pub fn new(source: PathBuf, destination: PathBuf, game_id: String, kind: String) -> Self {
        let metadata = fs::metadata(&source);
        let bytes = metadata.as_ref().map_or(0, |m| m.len());
        let sha256 = if bytes <= MAX_MIGRATION_BYTES {
            sha256_file_bounded(&source, bytes).unwrap_or_default()
        } else {
            String::new()
        };
        Self {
            source,
            destination,
            game_id,
            kind,
            sha256,
            bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStatus {
    Ready,
    Conflict,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewItem {
    pub item: MigrationItem,
    pub status: MigrationStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    pub schema: u32,
    pub items: Vec<PreviewItem>,
}

pub fn preview(items: &[MigrationItem]) -> Result<MigrationReport, String> {
    let mut destinations = std::collections::HashMap::<String, (&str, &str, &str)>::new();
    let mut ambiguous = std::collections::HashSet::<String>::new();
    let mut source_targets =
        std::collections::HashMap::<String, (String, String, String, String)>::new();
    let mut ambiguous_sources = std::collections::HashSet::<String>::new();
    for item in items {
        let key = item.destination.to_string_lossy().to_lowercase();
        if let Some((game_id, kind, hash)) = destinations.get(&key) {
            if *game_id != item.game_id || *kind != item.kind || *hash != item.sha256 {
                ambiguous.insert(key);
            }
        } else {
            destinations.insert(key, (&item.game_id, &item.kind, &item.sha256));
        }
        let source_key = item.source.to_string_lossy().to_lowercase();
        let target = (
            item.destination.to_string_lossy().to_lowercase(),
            item.game_id.clone(),
            item.kind.clone(),
            item.sha256.clone(),
        );
        if source_targets
            .get(&source_key)
            .is_some_and(|existing| existing != &target)
        {
            ambiguous_sources.insert(source_key);
        } else {
            source_targets.insert(source_key, target);
        }
    }
    let mut report = MigrationReport {
        schema: 1,
        items: Vec::with_capacity(items.len()),
    };
    for item in items {
        let key = item.destination.to_string_lossy().to_lowercase();
        let source_key = item.source.to_string_lossy().to_lowercase();
        if ambiguous_sources.contains(&source_key) {
            report.items.push(PreviewItem {
                item: item.clone(),
                status: MigrationStatus::Conflict,
                reason: Some("El mismo archivo de origen apunta a varios juegos o destinos".into()),
            });
        } else if ambiguous.contains(&key) {
            report.items.push(PreviewItem {
                item: item.clone(),
                status: MigrationStatus::Conflict,
                reason: Some("Varios guardados distintos apuntan al mismo destino".into()),
            });
        } else if item.kind == "state_unverified" {
            report.items.push(PreviewItem {
                item: item.clone(),
                status: MigrationStatus::Conflict,
                reason: Some(
                    "El estado legado no identifica ROM ni versión de core; se conserva sin migrar"
                        .into(),
                ),
            });
        } else if item.bytes > MAX_MIGRATION_BYTES {
            report.items.push(PreviewItem {
                item: item.clone(),
                status: MigrationStatus::Conflict,
                reason: Some("El archivo supera el límite de migración de 512 MiB".into()),
            });
        } else if item.sha256.is_empty() || !item.source.is_file() {
            report.items.push(PreviewItem {
                item: item.clone(),
                status: MigrationStatus::Conflict,
                reason: Some("Origen ausente o ilegible".into()),
            });
        } else if item.destination.exists() {
            let same = sha256_file_bounded(&item.destination, item.bytes)
                .ok()
                .as_deref()
                == Some(&item.sha256);
            report.items.push(PreviewItem {
                item: item.clone(),
                status: if same {
                    MigrationStatus::Committed
                } else {
                    MigrationStatus::Conflict
                },
                reason: (!same).then(|| "El destino ya contiene otros datos".into()),
            });
        } else {
            report.items.push(PreviewItem {
                item: item.clone(),
                status: MigrationStatus::Ready,
                reason: None,
            });
        }
    }
    Ok(report)
}

pub fn apply(journal_path: &Path, report: &MigrationReport) -> Result<(), String> {
    if report.schema != 1 {
        return Err("Versión de journal de migración no compatible".into());
    }
    let mut journal = load_journal(journal_path)?.unwrap_or_else(|| report.clone());
    if journal.schema != 1 {
        return Err("Versión de journal de migración guardado no compatible".into());
    }
    let same_preview = journal.items.len() == report.items.len()
        && journal
            .items
            .iter()
            .zip(&report.items)
            .all(|(a, b)| a.item == b.item);
    if !same_preview {
        if journal
            .items
            .iter()
            .any(|item| item.status == MigrationStatus::Ready)
        {
            return Err("Hay una migración anterior pendiente de recuperación".into());
        }
        journal = report.clone();
    }
    persist(journal_path, &journal)?;
    for index in 0..journal.items.len() {
        let item = journal.items[index].item.clone();
        if journal.items[index].status == MigrationStatus::Conflict {
            continue;
        }
        if item.bytes > MAX_MIGRATION_BYTES {
            journal.items[index].status = MigrationStatus::Conflict;
            journal.items[index].reason =
                Some("El archivo supera el límite de migración de 512 MiB".into());
            persist(journal_path, &journal)?;
            continue;
        }
        if item.destination.is_file() {
            let dest_hash = sha256_file_bounded(&item.destination, item.bytes).ok();
            if dest_hash.as_deref() == Some(&item.sha256) {
                journal.items[index].status = MigrationStatus::Committed;
                journal.items[index].reason = None;
                persist(journal_path, &journal)?;
                continue;
            }
            journal.items[index].status = MigrationStatus::Conflict;
            journal.items[index].reason = Some("El destino ya contiene otros datos".into());
            persist(journal_path, &journal)?;
            continue;
        }
        let actual = match sha256_file_bounded(&item.source, item.bytes) {
            Ok(hash) => hash,
            Err(_) => {
                journal.items[index].status = MigrationStatus::Conflict;
                journal.items[index].reason = Some(
                    "El origen cambió o dejó de estar disponible después de la vista previa".into(),
                );
                persist(journal_path, &journal)?;
                continue;
            }
        };
        if actual != item.sha256 {
            journal.items[index].status = MigrationStatus::Conflict;
            journal.items[index].reason =
                Some("El origen cambió después de la vista previa".into());
            persist(journal_path, &journal)?;
            continue;
        }
        install_payload(&item)?;
        journal.items[index].status = MigrationStatus::Committed;
        journal.items[index].reason = None;
        persist(journal_path, &journal)?;
    }
    Ok(())
}

fn install_payload(item: &MigrationItem) -> Result<(), String> {
    install_payload_with(item, |_| Ok(()))
}

fn install_payload_with(
    item: &MigrationItem,
    after_verification: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    if item.bytes > MAX_MIGRATION_BYTES {
        return Err("El archivo supera el límite de migración de 512 MiB".into());
    }
    let parent = item.destination.parent().ok_or("Destino sin carpeta")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut src = fs::File::open(&item.source).map_err(|e| e.to_string())?;
    let (staging, mut dst) = create_staging_file(&item.destination)?;
    let result = (|| {
        let copied = std::io::copy(&mut src.take(item.bytes.saturating_add(1)), &mut dst)
            .map_err(|e| e.to_string())?;
        if copied != item.bytes {
            return Err("El origen cambió de tamaño durante la migración".into());
        }
        dst.sync_all().map_err(|e| e.to_string())?;
        drop(dst);
        if sha256_file_bounded(&staging, item.bytes).map_err(|e| e.to_string())? != item.sha256 {
            return Err("La copia migrada no coincide con el hash del origen".into());
        }
        after_verification(&staging)?;
        fs::rename(&staging, &item.destination).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

fn sha256_file_bounded(path: &Path, expected_bytes: u64) -> Result<String, String> {
    if expected_bytes > MAX_MIGRATION_BYTES {
        return Err("El archivo supera el límite de migración de 512 MiB".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bounded = file.take(expected_bytes.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = bounded.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > expected_bytes {
            return Err("El archivo cambió de tamaño después de la vista previa".into());
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_bytes {
        return Err("El archivo cambió de tamaño después de la vista previa".into());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn create_staging_file(destination: &Path) -> Result<(PathBuf, fs::File), String> {
    let parent = destination.parent().ok_or("Destino sin carpeta")?;
    let name = destination
        .file_name()
        .ok_or("Destino sin nombre")?
        .to_string_lossy();
    for _ in 0..128 {
        let sequence = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
        let staging = parent.join(format!(
            ".{name}.vyntra-migration-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
        {
            Ok(file) => return Ok((staging, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("No se pudo reservar un archivo temporal exclusivo para migración".into())
}

#[cfg(test)]
mod tests {
    use super::{
        MigrationItem, MigrationReport, MigrationStatus, apply, install_payload,
        install_payload_with, load_journal, persist, persist_with, persist_with_backup_and_install,
        persist_with_install, preview, read_file_bounded, serialize_journal_bounded,
        sha256_file_bounded,
    };
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn migration_staging_files(destination: &Path) -> Vec<std::path::PathBuf> {
        let parent = destination.parent().unwrap();
        let prefix = format!(
            ".{}.vyntra-migration-",
            destination.file_name().unwrap().to_string_lossy()
        );
        fs::read_dir(parent)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
            })
            .collect()
    }

    #[cfg(windows)]
    fn current_windows_user_sid() -> String {
        let identity = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .unwrap();
        assert!(identity.status.success());
        String::from_utf8(identity.stdout)
            .unwrap()
            .lines()
            .next()
            .and_then(|line| line.split(',').nth(1))
            .map(|value| value.trim().trim_matches('"'))
            .filter(|value| value.starts_with("S-1-"))
            .expect("whoami returns the current Windows SID")
            .to_owned()
    }

    #[cfg(windows)]
    fn deny_windows_write(path: &Path, sid: &str) -> std::process::Output {
        std::process::Command::new("icacls")
            .arg(path)
            .arg("/deny")
            .arg(format!("*{sid}:(WD)"))
            .output()
            .unwrap()
    }

    #[cfg(windows)]
    fn restore_windows_write(path: &Path, sid: &str) -> std::process::Output {
        std::process::Command::new("icacls")
            .arg(path)
            .arg("/remove:d")
            .arg(format!("*{sid}"))
            .output()
            .unwrap()
    }

    #[test]
    fn migration_refuses_a_source_changed_after_preview() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-source-changed-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal = root.join("migration.json");
        fs::write(&source, b"save at preview").unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();
        assert_eq!(report.items[0].status, MigrationStatus::Ready);

        fs::write(&source, b"save changed after preview").unwrap();
        apply(&journal, &report).unwrap();
        apply(&journal, &report).unwrap();

        let recovered: MigrationReport =
            serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
        assert_eq!(recovered.items[0].status, MigrationStatus::Conflict);
        assert_eq!(
            recovered.items[0].reason.as_deref(),
            Some("El origen cambió o dejó de estar disponible después de la vista previa")
        );
        assert_eq!(fs::read(&source).unwrap(), b"save changed after preview");
        assert!(!destination.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_refuses_an_unsupported_journal_schema_without_rewriting_it() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-journal-schema-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let journal_path = root.join("migration.json");
        let unsupported = MigrationReport {
            schema: 99,
            items: Vec::new(),
        };
        persist(&journal_path, &unsupported).unwrap();
        let original = fs::read(&journal_path).unwrap();
        let report = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };

        assert!(apply(&journal_path, &report).is_err());
        assert_eq!(fs::read(&journal_path).unwrap(), original);
        fs::remove_file(&journal_path).unwrap();
        let backup_path = journal_path.with_extension("bak");
        fs::write(&backup_path, &original).unwrap();
        assert!(apply(&journal_path, &report).is_err());
        assert!(!journal_path.exists());
        assert_eq!(fs::read(&backup_path).unwrap(), original);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_rejects_source_growth_after_preview_without_copying_it() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-source-growth-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal = root.join("migration.json");
        fs::write(&source, b"small preview").unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(&source)
            .unwrap()
            .write_all(b" appended after preview")
            .unwrap();

        apply(&journal, &report).unwrap();

        let recovered = load_journal(&journal).unwrap().unwrap();
        assert_eq!(recovered.items[0].status, MigrationStatus::Conflict);
        assert!(!destination.exists());
        assert_eq!(
            fs::read(&source).unwrap(),
            b"small preview appended after preview"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_preserves_an_unowned_staging_neighbor() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-unowned-stage-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal = root.join("migration.json");
        let unowned_staging = destination.with_extension("migration-tmp");
        let orphan_bytes = b"unrelated file sharing old temporary name";
        fs::write(&source, b"verified save payload").unwrap();
        fs::create_dir_all(unowned_staging.parent().unwrap()).unwrap();
        fs::write(&unowned_staging, orphan_bytes).unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();

        apply(&journal, &report).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"verified save payload");
        assert_eq!(fs::read(&source).unwrap(), b"verified save payload");
        assert_eq!(fs::read(&unowned_staging).unwrap(), orphan_bytes);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_migration_hash_rejects_bytes_beyond_the_preview_size() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-bounded-hash-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let bytes = b"previewed bytes";
        fs::write(&source, bytes).unwrap();

        assert_eq!(
            sha256_file_bounded(&source, bytes.len() as u64).unwrap(),
            crate::identity::sha256_bytes(bytes)
        );
        assert!(sha256_file_bounded(&source, 4).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preview_marks_one_legacy_source_for_distinct_games_as_conflict() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-shared-source-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("Same.gb.sav");
        fs::write(&source, b"legacy battery").unwrap();
        let items = [
            MigrationItem::new(
                source.clone(),
                root.join("by-id/game-a/battery.sav"),
                "game-a".into(),
                "battery".into(),
            ),
            MigrationItem::new(
                source,
                root.join("by-id/game-b/battery.sav"),
                "game-b".into(),
                "battery".into(),
            ),
        ];

        let report = preview(&items).unwrap();

        assert!(
            report
                .items
                .iter()
                .all(|item| item.status == MigrationStatus::Conflict)
        );
        assert!(report.items.iter().all(|item| {
            item.reason
                .as_deref()
                .is_some_and(|reason| reason.contains("origen"))
        }));
        assert!(!root.join("by-id/game-a/battery.sav").exists());
        assert!(!root.join("by-id/game-b/battery.sav").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn storage_full_during_journal_staging_preserves_previous_journal() {
        let root =
            std::env::temp_dir().join(format!("vyntra-migration-full-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        let previous = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        persist(&path, &previous).unwrap();

        let error = persist_with(&path, &next, |file, bytes| {
            file.write_all(&bytes[..bytes.len().min(4)])?;
            Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
        });

        assert!(error.is_err());
        assert_eq!(
            fs::read(&path).unwrap(),
            serde_json::to_vec_pretty(&previous).unwrap()
        );
        assert!(migration_staging_files(&path).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn migration_acl_denial_preserves_previous_journal_and_source() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-acl-denial-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal = root.join("migration.json");
        let source_bytes = b"legacy battery remains untouched";
        fs::write(&source, source_bytes).unwrap();
        let previous = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };
        persist(&journal, &previous).unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();

        let sid = current_windows_user_sid();
        let denied_acl = deny_windows_write(&root, &sid);
        if !denied_acl.status.success() {
            eprintln!(
                "Skipping denied-folder assertion: icacls could not set the temporary ACL: {}",
                String::from_utf8_lossy(&denied_acl.stderr)
            );
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let apply_result = apply(&journal, &report);
        let restore_acl = restore_windows_write(&root, &sid);
        let journal_bytes = fs::read(&journal).unwrap();
        let source_after = fs::read(&source).unwrap();
        let destination_exists = destination.exists();
        let _ = fs::remove_dir_all(&root);

        assert!(
            restore_acl.status.success(),
            "temporary ACL must be restored"
        );
        assert!(apply_result.is_err());
        assert_eq!(journal_bytes, serde_json::to_vec_pretty(&previous).unwrap());
        assert_eq!(source_after, source_bytes);
        assert!(!destination_exists);
    }

    #[cfg(windows)]
    #[test]
    fn migration_payload_acl_denial_keeps_source_and_ready_journal() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-payload-acl-denial-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("legacy.sav");
        let destination_parent = root.join("destination/game");
        let destination = destination_parent.join("battery.sav");
        let journal = root.join("migration.json");
        let source_bytes = b"legacy battery remains untouched";
        fs::create_dir_all(&destination_parent).unwrap();
        fs::write(&source, source_bytes).unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();
        let sid = current_windows_user_sid();
        let denied_acl = deny_windows_write(&destination_parent, &sid);
        if !denied_acl.status.success() {
            eprintln!(
                "Skipping denied-payload assertion: icacls could not set the temporary ACL: {}",
                String::from_utf8_lossy(&denied_acl.stderr)
            );
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let apply_result = apply(&journal, &report);
        let restore_acl = restore_windows_write(&destination_parent, &sid);
        let source_after = fs::read(&source).unwrap();
        let journal_after = load_journal(&journal).unwrap().unwrap();
        let destination_exists = destination.exists();
        let staging_files = migration_staging_files(&destination);
        let _ = fs::remove_dir_all(&root);

        assert!(
            restore_acl.status.success(),
            "temporary ACL must be restored"
        );
        assert!(apply_result.is_err());
        assert_eq!(source_after, source_bytes);
        assert_eq!(journal_after.items[0].status, MigrationStatus::Ready);
        assert!(!destination_exists);
        assert!(staging_files.is_empty());
    }

    #[test]
    fn journal_persistence_preserves_an_unowned_tmp_neighbor() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-unowned-journal-tmp-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        let unowned_tmp = path.with_extension("tmp");
        let contents = b"unrelated application data";
        fs::write(&unowned_tmp, contents).unwrap();
        let report = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };

        persist(&path, &report).unwrap();

        assert_eq!(fs::read(&unowned_tmp).unwrap(), contents);
        assert_eq!(load_journal(&path).unwrap(), Some(report));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn journal_reader_stops_at_its_configured_byte_limit() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-journal-read-limit-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        fs::write(&path, b"0123456789abcdef").unwrap();

        assert_eq!(read_file_bounded(&path, 16).unwrap(), b"0123456789abcdef");
        let error = read_file_bounded(&path, 8).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn journal_serializer_rejects_reports_over_its_byte_limit() {
        let report = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };

        assert!(serialize_journal_bounded(&report, 4096).is_ok());
        assert!(serialize_journal_bounded(&report, 8).is_err());
    }

    #[test]
    fn journal_recovers_previous_version_after_process_exit_during_staging() {
        const JOURNAL_ENV: &str = "VYNTRA_MIGRATION_STAGE_CRASH_PATH";
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-stage-crash-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        let previous = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        persist(&path, &previous).unwrap();

        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration::tests::terminate_during_journal_staging",
            ])
            .env(JOURNAL_ENV, &path)
            .status()
            .unwrap();
        assert_eq!(child.code(), Some(78));
        assert_eq!(load_journal(&path).unwrap(), Some(previous));
        let orphaned_staging = migration_staging_files(&path);
        assert_eq!(orphaned_staging.len(), 1);
        let orphaned_bytes = fs::read(&orphaned_staging[0]).unwrap();

        persist(&path, &next).unwrap();
        assert_eq!(load_journal(&path).unwrap(), Some(next));
        assert_eq!(fs::read(&orphaned_staging[0]).unwrap(), orphaned_bytes);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn terminate_during_journal_staging() {
        const JOURNAL_ENV: &str = "VYNTRA_MIGRATION_STAGE_CRASH_PATH";
        let Some(path) = std::env::var_os(JOURNAL_ENV) else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        let _ = persist_with(&path, &next, |file, bytes| {
            file.write_all(&bytes[..bytes.len().min(4)])?;
            file.sync_all()?;
            std::process::exit(78)
        });
        unreachable!("the subprocess exits during journal staging");
    }

    #[test]
    fn failed_journal_install_restores_previous_journal() {
        let root =
            std::env::temp_dir().join(format!("vyntra-migration-rename-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        let previous = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        persist(&path, &previous).unwrap();

        let error = persist_with_install(
            &path,
            &next,
            |file, bytes| file.write_all(bytes),
            |_, _| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        );

        assert!(error.is_err());
        assert_eq!(
            fs::read(&path).unwrap(),
            serde_json::to_vec_pretty(&previous).unwrap()
        );
        assert!(migration_staging_files(&path).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_journal_backup_copy_preserves_previous_journal() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-backup-failure-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        let previous = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        persist(&path, &previous).unwrap();

        let error = persist_with_backup_and_install(
            &path,
            &next,
            |file, bytes| file.write_all(bytes),
            |_, _| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            |from, to| fs::rename(from, to),
        );

        assert!(error.is_err());
        assert_eq!(
            fs::read(&path).unwrap(),
            serde_json::to_vec_pretty(&previous).unwrap()
        );
        assert!(migration_staging_files(&path).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn journal_load_recovers_backup_after_interrupted_replacement() {
        const JOURNAL_ENV: &str = "VYNTRA_MIGRATION_JOURNAL_CRASH_PATH";
        let root =
            std::env::temp_dir().join(format!("vyntra-migration-cut-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("migration.json");
        let previous = MigrationReport {
            schema: 1,
            items: Vec::new(),
        };
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        persist(&path, &previous).unwrap();

        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration::tests::terminate_during_journal_install",
            ])
            .env(JOURNAL_ENV, &path)
            .status()
            .unwrap();
        assert_eq!(child.code(), Some(77));
        assert!(!path.exists());
        let orphaned_staging = migration_staging_files(&path);
        assert_eq!(orphaned_staging.len(), 1);
        assert_eq!(
            serde_json::from_slice::<MigrationReport>(&fs::read(&orphaned_staging[0]).unwrap())
                .unwrap(),
            next
        );
        assert_eq!(load_journal(&path).unwrap(), Some(previous.clone()));

        persist(&path, &next).unwrap();
        assert_eq!(load_journal(&path).unwrap(), Some(next));
        assert!(orphaned_staging[0].exists());
        fs::write(&path, b"{").unwrap();
        assert_eq!(load_journal(&path).unwrap(), Some(previous));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn terminate_during_journal_install() {
        const JOURNAL_ENV: &str = "VYNTRA_MIGRATION_JOURNAL_CRASH_PATH";
        let Some(path) = std::env::var_os(JOURNAL_ENV) else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let next = MigrationReport {
            schema: 2,
            items: Vec::new(),
        };
        let _ = persist_with_install(
            &path,
            &next,
            |file, bytes| {
                file.write_all(bytes)?;
                file.sync_all()
            },
            |_, _| std::process::exit(77),
        );
        unreachable!("the subprocess exits after removing the previous journal");
    }

    #[test]
    fn migration_retry_preserves_partial_staging_after_interrupted_copy() {
        let root =
            std::env::temp_dir().join(format!("vyntra-migration-copy-cut-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal_path = root.join("migration.json");
        let bytes = b"complete legacy save bytes";
        fs::write(&source, bytes).unwrap();
        let item = MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        );
        let report = preview(&[item]).unwrap();
        assert_eq!(report.items[0].status, MigrationStatus::Ready);
        persist(&journal_path, &report).unwrap();

        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let staging = destination.with_extension("migration-tmp");
        const INTERRUPT_PATH_ENV: &str = "VYNTRA_MIGRATION_STAGING_INTERRUPT_PATH";
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration::tests::terminate_during_migration_copy_staging",
            ])
            .env(INTERRUPT_PATH_ENV, &staging)
            .status()
            .unwrap();
        assert_eq!(child.code(), Some(75));
        assert_eq!(
            fs::read(&staging).unwrap(),
            b"partial bytes before process interruption"
        );

        apply(&journal_path, &report).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(
            fs::read(&staging).unwrap(),
            b"partial bytes before process interruption"
        );
        let recovered = load_journal(&journal_path).unwrap().unwrap();
        assert_eq!(recovered.items[0].status, MigrationStatus::Committed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_retry_preserves_verified_staging_after_process_termination() {
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-verified-cut-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal_path = root.join("migration.json");
        let bytes = b"verified legacy save bytes";
        fs::write(&source, bytes).unwrap();
        let report = preview(&[MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        )])
        .unwrap();
        persist(&journal_path, &report).unwrap();

        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration::tests::terminate_after_migration_staging_verification",
            ])
            .env("VYNTRA_MIGRATION_VERIFIED_SOURCE", &source)
            .env("VYNTRA_MIGRATION_VERIFIED_DEST", &destination)
            .status()
            .unwrap();
        assert_eq!(child.code(), Some(74));
        let staging_files = migration_staging_files(&destination);
        assert_eq!(staging_files.len(), 1);
        let staging = &staging_files[0];
        assert_eq!(fs::read(&staging).unwrap(), bytes);
        assert!(!destination.exists());

        apply(&journal_path, &report).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert_eq!(fs::read(&staging).unwrap(), bytes);
        assert_eq!(
            load_journal(&journal_path).unwrap().unwrap().items[0].status,
            MigrationStatus::Committed
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn terminate_after_migration_staging_verification() {
        const SOURCE_ENV: &str = "VYNTRA_MIGRATION_VERIFIED_SOURCE";
        const DEST_ENV: &str = "VYNTRA_MIGRATION_VERIFIED_DEST";
        let (Some(source), Some(destination)) =
            (std::env::var_os(SOURCE_ENV), std::env::var_os(DEST_ENV))
        else {
            return;
        };
        let source = std::path::PathBuf::from(source);
        let destination = std::path::PathBuf::from(destination);
        let item = MigrationItem::new(source, destination.clone(), "game".into(), "battery".into());
        install_payload_with(&item, |verified| {
            let expected_prefix = format!(
                ".{}.vyntra-migration-",
                destination.file_name().unwrap().to_string_lossy()
            );
            assert!(
                verified
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&expected_prefix)
            );
            std::process::exit(74);
        })
        .unwrap();
    }

    #[test]
    fn migration_recovers_after_child_exit_between_install_and_journal_commit() {
        const SOURCE_ENV: &str = "VYNTRA_MIGRATION_INSTALL_SOURCE";
        const DEST_ENV: &str = "VYNTRA_MIGRATION_INSTALL_DEST";
        let root = std::env::temp_dir().join(format!(
            "vyntra-migration-install-crash-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.sav");
        let destination = root.join("by-id/game/battery.sav");
        let journal_path = root.join("migration.json");
        fs::write(&source, b"verified legacy save").unwrap();
        let item = MigrationItem::new(
            source.clone(),
            destination.clone(),
            "game".into(),
            "battery".into(),
        );
        let report = preview(&[item]).unwrap();
        persist(&journal_path, &report).unwrap();

        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "migration::tests::terminate_after_migration_destination_install",
            ])
            .env(SOURCE_ENV, &source)
            .env(DEST_ENV, &destination)
            .status()
            .unwrap();

        assert_eq!(child.code(), Some(76));
        assert_eq!(fs::read(&destination).unwrap(), b"verified legacy save");
        let interrupted: MigrationReport =
            serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
        assert_eq!(interrupted.items[0].status, MigrationStatus::Ready);

        apply(&journal_path, &report).unwrap();
        apply(&journal_path, &report).unwrap();
        let recovered: MigrationReport =
            serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
        assert_eq!(recovered.items[0].status, MigrationStatus::Committed);
        assert_eq!(fs::read(&destination).unwrap(), b"verified legacy save");
        assert_eq!(fs::read(&source).unwrap(), b"verified legacy save");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn terminate_after_migration_destination_install() {
        const SOURCE_ENV: &str = "VYNTRA_MIGRATION_INSTALL_SOURCE";
        const DEST_ENV: &str = "VYNTRA_MIGRATION_INSTALL_DEST";
        let (Some(source), Some(destination)) =
            (std::env::var_os(SOURCE_ENV), std::env::var_os(DEST_ENV))
        else {
            return;
        };
        let item = MigrationItem::new(
            source.into(),
            destination.into(),
            "game".into(),
            "battery".into(),
        );
        install_payload(&item).unwrap();
        std::process::exit(76);
    }

    #[test]
    fn terminate_during_migration_copy_staging() {
        const INTERRUPT_PATH_ENV: &str = "VYNTRA_MIGRATION_STAGING_INTERRUPT_PATH";
        let Some(path) = std::env::var_os(INTERRUPT_PATH_ENV) else {
            return;
        };
        let mut staging = fs::File::create(path).unwrap();
        staging
            .write_all(b"partial bytes before process interruption")
            .unwrap();
        std::process::exit(75);
    }
}

fn load_journal(path: &Path) -> Result<Option<MigrationReport>, String> {
    let backup = path.with_extension("bak");
    match read_file_bounded(path, MAX_JOURNAL_BYTES) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(report) => Ok(Some(report)),
            Err(primary_error) => recover_backup(&backup, Some(&primary_error.to_string())),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => recover_backup(&backup, None),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            recover_backup(&backup, Some(&error.to_string()))
        }
        Err(error) => Err(error.to_string()),
    }
}

fn recover_backup(
    backup: &Path,
    primary_error: Option<&str>,
) -> Result<Option<MigrationReport>, String> {
    match read_file_bounded(backup, MAX_JOURNAL_BYTES) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|backup_error| match primary_error {
                Some(primary_error) => {
                    format!("Journal y respaldo inválidos ({primary_error}; {backup_error})")
                }
                None => backup_error.to_string(),
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && primary_error.is_none() => {
            Ok(None)
        }
        Err(error) => match primary_error {
            Some(primary_error) => Err(format!(
                "Journal inválido ({primary_error}); no se pudo leer respaldo ({error})"
            )),
            None => Err(error.to_string()),
        },
    }
}

fn read_file_bounded(path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "El journal supera el límite configurado",
        ));
    }
    Ok(bytes)
}

fn persist(path: &Path, report: &MigrationReport) -> Result<(), String> {
    persist_with(path, report, |file, bytes| {
        file.write_all(bytes)?;
        file.sync_all()
    })
}

fn serialize_journal_bounded(report: &MigrationReport, max_bytes: u64) -> Result<Vec<u8>, String> {
    struct LimitedBuffer {
        bytes: Vec<u8>,
        max_bytes: usize,
    }
    impl Write for LimitedBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().saturating_add(bytes.len()) > self.max_bytes {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "El journal supera el límite configurado",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut output = LimitedBuffer {
        bytes: Vec::new(),
        max_bytes: usize::try_from(max_bytes).unwrap_or(usize::MAX),
    };
    serde_json::to_writer_pretty(&mut output, report).map_err(|error| error.to_string())?;
    Ok(output.bytes)
}

fn persist_with(
    path: &Path,
    report: &MigrationReport,
    stage: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<(), String> {
    persist_with_install(path, report, stage, |from, to| fs::rename(from, to))
}

fn persist_with_install(
    path: &Path,
    report: &MigrationReport,
    stage: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
    install: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    persist_with_backup_and_install(
        path,
        report,
        stage,
        |source, backup| fs::copy(source, backup).map(|_| ()),
        install,
    )
}

fn persist_with_backup_and_install(
    path: &Path,
    report: &MigrationReport,
    stage: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
    backup_file: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    install: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serialize_journal_bounded(report, MAX_JOURNAL_BYTES)?;
    let (tmp, mut file) = create_staging_file(path)?;
    if let Err(error) = stage(&mut file, &bytes) {
        drop(file);
        let _ = fs::remove_file(&tmp);
        return Err(error.to_string());
    }
    drop(file);
    let had_previous = path.is_file();
    let backup = path.with_extension("bak");
    if had_previous {
        if let Err(error) = backup_file(path, &backup) {
            let _ = fs::remove_file(&tmp);
            return Err(error.to_string());
        }
    }
    if path.exists() {
        if let Err(error) = fs::remove_file(path) {
            let _ = fs::remove_file(&tmp);
            return Err(error.to_string());
        }
    }
    if let Err(error) = install(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        if had_previous && !path.exists() {
            if let Err(restore_error) = fs::copy(&backup, path) {
                return Err(format!(
                    "No se pudo publicar el journal ({error}); la copia previa permanece en {} ({restore_error})",
                    backup.display()
                ));
            }
        }
        return Err(error.to_string());
    }
    Ok(())
}
