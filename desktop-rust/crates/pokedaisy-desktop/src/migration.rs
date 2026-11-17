//! Non-destructive, journaled migration for legacy save artifacts.
use crate::identity;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
        let sha256 = identity::sha256_file(&source).unwrap_or_default();
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
    for item in items {
        let key = item.destination.to_string_lossy().to_lowercase();
        if let Some((game_id, kind, hash)) = destinations.get(&key) {
            if *game_id != item.game_id || *kind != item.kind || *hash != item.sha256 {
                ambiguous.insert(key);
            }
        } else {
            destinations.insert(key, (&item.game_id, &item.kind, &item.sha256));
        }
    }
    let mut report = MigrationReport {
        schema: 1,
        items: Vec::with_capacity(items.len()),
    };
    for item in items {
        let key = item.destination.to_string_lossy().to_lowercase();
        if ambiguous.contains(&key) {
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
        } else if item.bytes > 512 * 1024 * 1024 {
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
            let same =
                identity::sha256_file(&item.destination).ok().as_deref() == Some(&item.sha256);
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
        if item.destination.is_file() {
            let dest_hash = identity::sha256_file(&item.destination).map_err(|e| e.to_string())?;
            if dest_hash == item.sha256 {
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
        let actual = match identity::sha256_file(&item.source) {
            Ok(hash) => hash,
            Err(_) => {
                journal.items[index].status = MigrationStatus::Conflict;
                journal.items[index].reason = Some("El origen ya no está disponible".into());
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
    let parent = item.destination.parent().ok_or("Destino sin carpeta")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let staging = item.destination.with_extension("migration-tmp");
    let mut src = fs::File::open(&item.source).map_err(|e| e.to_string())?;
    let mut dst = fs::File::create(&staging).map_err(|e| e.to_string())?;
    std::io::copy(&mut src, &mut dst).map_err(|e| e.to_string())?;
    dst.sync_all().map_err(|e| e.to_string())?;
    drop(dst);
    if identity::sha256_file(&staging).map_err(|e| e.to_string())? != item.sha256 {
        let _ = fs::remove_file(&staging);
        return Err("La copia migrada no coincide con el hash del origen".into());
    }
    fs::rename(&staging, &item.destination).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MigrationItem, MigrationReport, MigrationStatus, apply, install_payload, load_journal,
        persist, persist_with, persist_with_install, preview,
    };
    use std::fs;
    use std::io::Write;

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
            Some("El origen cambió después de la vista previa")
        );
        assert_eq!(fs::read(&source).unwrap(), b"save changed after preview");
        assert!(!destination.exists());
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
        assert!(!path.with_extension("tmp").exists());
        let _ = fs::remove_dir_all(root);
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
        assert!(path.with_extension("tmp").exists());

        persist(&path, &next).unwrap();
        assert_eq!(load_journal(&path).unwrap(), Some(next));
        assert!(!path.with_extension("tmp").exists());
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
        assert!(!path.with_extension("tmp").exists());
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
        assert!(path.with_extension("tmp").exists());
        assert_eq!(load_journal(&path).unwrap(), Some(previous.clone()));

        persist(&path, &next).unwrap();
        assert_eq!(load_journal(&path).unwrap(), Some(next));
        assert!(!path.with_extension("tmp").exists());
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
    fn migration_retry_replaces_partial_staging_after_interrupted_copy() {
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
        assert!(!staging.exists());
        let recovered = load_journal(&journal_path).unwrap().unwrap();
        assert_eq!(recovered.items[0].status, MigrationStatus::Committed);
        let _ = fs::remove_dir_all(root);
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
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(report) => Ok(Some(report)),
            Err(primary_error) => match fs::read(&backup) {
                Ok(bytes) => serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|backup_error| {
                        format!("Journal y respaldo inválidos ({primary_error}; {backup_error})")
                    }),
                Err(backup_error) => Err(format!(
                    "Journal inválido ({primary_error}); no se pudo leer respaldo ({backup_error})"
                )),
            },
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match fs::read(&backup) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        },
        Err(error) => Err(error.to_string()),
    }
}

fn persist(path: &Path, report: &MigrationReport) -> Result<(), String> {
    persist_with(path, report, |file, bytes| {
        file.write_all(bytes)?;
        file.sync_all()
    })
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    let staged = (|| -> Result<(), String> {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        stage(&mut file, &bytes).map_err(|e| e.to_string())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    let had_previous = path.is_file();
    let backup = path.with_extension("bak");
    if had_previous {
        if let Err(error) = fs::copy(path, &backup) {
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
