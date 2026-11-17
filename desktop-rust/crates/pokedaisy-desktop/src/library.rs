//! Library index (catalog) persistence with a forward-compatible schema.
//!
//! The legacy format is a plain JSON array of paths. The new format is a JSON
//! array of objects carrying a stable id, content hash and best-effort metadata.
//! Loading accepts both; writing always emits the new format atomically.

use crate::identity;
use crate::{domain::IdentityConfidence, inspect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Stable identifier derived from the ROM content hash.
    pub id: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<IdentityConfidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_unix: Option<u64>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played_unix: Option<u64>,
}

impl LibraryEntry {
    pub fn from_path(path: PathBuf) -> Self {
        let content_hash = identity::sha256_file(&path).ok();
        let inspection = inspect::inspect_file(&path);
        let id = content_hash
            .as_ref()
            .map(|h| h[..16].to_string())
            .unwrap_or_else(|| identity::storage_key(&path));
        Self {
            id,
            title: inspection
                .as_ref()
                .and_then(|info| info.title.clone())
                .or_else(|| {
                    path.file_stem()
                        .map(|s| s.to_string_lossy().replace(['_', '-'], " "))
                }),
            system: inspection
                .as_ref()
                .map(|info| info.system.to_string())
                .or_else(|| system_from_extension(&path)),
            game_code: inspection.as_ref().and_then(|info| info.game_code.clone()),
            revision: inspection.as_ref().and_then(|info| info.revision),
            confidence: inspection.map(|info| info.confidence),
            path,
            content_hash,
            last_seen_unix: None,
            favorite: false,
            last_played_unix: None,
        }
    }

    pub fn refresh(&mut self) {
        self.content_hash = identity::sha256_file(&self.path).ok();
        if let Some(hash) = &self.content_hash {
            self.id = hash[..16].to_string();
        }
        let inspection = inspect::inspect_file(&self.path);
        if let Some(info) = inspection {
            self.title = info.title.or_else(|| self.title.clone());
            self.system = Some(info.system.to_string());
            self.game_code = info.game_code;
            self.revision = info.revision;
            self.confidence = Some(info.confidence);
        } else if self.title.is_none() {
            self.title = self
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().replace(['_', '-'], " "));
        }
        if self.system.is_none() {
            self.system = system_from_extension(&self.path);
        }
    }
}

pub fn set_favorite(entries: &mut [LibraryEntry], id: &str, favorite: bool) -> bool {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) else {
        return false;
    };
    entry.favorite = favorite;
    true
}

pub fn mark_played(entries: &mut [LibraryEntry], id: &str, played_at_unix: u64) -> bool {
    let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) else {
        return false;
    };
    entry.last_played_unix = Some(played_at_unix);
    true
}

pub fn relink_entry(
    entries: &mut [LibraryEntry],
    id: &str,
    candidate: &Path,
) -> Result<(), String> {
    let entry = entries
        .iter_mut()
        .find(|entry| entry.id == id)
        .ok_or("El juego ya no está en la biblioteca")?;
    let expected = entry
        .content_hash
        .as_deref()
        .ok_or("Esta entrada no tiene hash previo; no se puede reenlazar con seguridad")?;
    if !candidate.is_file() {
        return Err("El archivo seleccionado no existe".into());
    }
    let actual = identity::sha256_file(candidate)?;
    if actual != expected {
        return Err("El archivo seleccionado no coincide con el contenido original".into());
    }
    entry.path = candidate
        .canonicalize()
        .map_err(|error| error.to_string())?;
    entry.refresh();
    Ok(())
}

fn system_from_extension(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "gb" => Some("GB".into()),
        "gbc" => Some("GBC".into()),
        "gba" => Some("GBA".into()),
        "nds" => Some("DS".into()),
        _ => None,
    }
}

/// Load the library index, accepting both the legacy path array and the object array.
pub fn load(path: &Path) -> Result<Vec<LibraryEntry>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Índice de biblioteca inválido: {e}"))?;
    parse_value(value)
}

/// Parse an already-decoded JSON value into entries (legacy or new schema).
pub fn parse_value(value: Value) -> Result<Vec<LibraryEntry>, String> {
    let array = value
        .as_array()
        .ok_or("El índice de biblioteca debe ser una lista")?;
    let mut out = Vec::with_capacity(array.len());
    for item in array {
        if let Some(path) = item.as_str() {
            out.push(LibraryEntry::from_path(PathBuf::from(path)));
            continue;
        }
        let mut entry: LibraryEntry = serde_json::from_value(item.clone())
            .map_err(|e| format!("Entrada de biblioteca inválida: {e}"))?;
        // Legacy/new entries may omit id; derive it deterministically.
        if entry.id.is_empty() {
            entry.refresh();
        }
        if entry.content_hash.is_none() && entry.path.is_file() {
            entry.refresh();
        }
        out.push(entry);
    }
    Ok(out)
}

pub fn save(path: &Path, entries: &[LibraryEntry]) -> Result<(), String> {
    atomic_write_json(path, entries)
}

fn atomic_write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if path.exists() {
        fs::copy(path, path.with_extension("json.bak")).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_path_array_parses() {
        let value = serde_json::json!(["C:/roms/a.gb", "C:/roms/b.gba"]);
        let entries = parse_value(value).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].system.as_deref(), Some("GB"));
        assert_eq!(entries[1].system.as_deref(), Some("GBA"));
        assert!(!entries[0].id.is_empty());
    }

    #[test]
    fn favorite_and_recent_metadata_roundtrip_without_changing_identity() {
        let dir = std::env::temp_dir().join(format!("vyntra-library-meta-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let rom = dir.join("favorite.gba");
        fs::write(&rom, b"fixture").unwrap();
        let mut entry = LibraryEntry::from_path(rom);
        let id = entry.id.clone();
        entry.favorite = true;
        entry.last_played_unix = Some(1234);
        let index = dir.join("library.json");
        save(&index, &[entry]).unwrap();
        let loaded = load(&index).unwrap();
        assert_eq!(loaded[0].id, id);
        assert!(loaded[0].favorite);
        assert_eq!(loaded[0].last_played_unix, Some(1234));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn favorite_and_played_updates_resolve_only_by_stable_id() {
        let mut entries = vec![LibraryEntry {
            id: "game-a".into(),
            path: PathBuf::from("a.gba"),
            content_hash: None,
            title: None,
            system: None,
            game_code: None,
            revision: None,
            confidence: None,
            last_seen_unix: None,
            favorite: false,
            last_played_unix: None,
        }];
        assert!(set_favorite(&mut entries, "game-a", true));
        assert!(mark_played(&mut entries, "game-a", 42));
        assert!(!mark_played(&mut entries, "missing", 50));
        assert!(entries[0].favorite);
        assert_eq!(entries[0].last_played_unix, Some(42));
    }

    #[test]
    fn relink_requires_exact_content_hash_and_preserves_identity() {
        let dir = std::env::temp_dir().join(format!("vyntra-relink-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let old_path = dir.join("old.gba");
        let new_path = dir.join("restored.gba");
        let wrong_path = dir.join("wrong.gba");
        let mut rom = vec![0u8; 0xc0];
        rom[0xb2] = 0x96;
        rom[0xac..0xb0].copy_from_slice(b"ABCD");
        rom[0xbc] = 1;
        fs::write(&old_path, &rom).unwrap();
        fs::write(&new_path, &rom).unwrap();
        fs::write(&wrong_path, b"different rom").unwrap();
        let mut entry = LibraryEntry::from_path(old_path.clone());
        entry.favorite = true;
        let id = entry.id.clone();
        fs::remove_file(&old_path).unwrap();

        let mut entries = vec![entry];
        assert!(relink_entry(&mut entries, &id, &wrong_path).is_err());
        assert_eq!(entries[0].path, old_path);
        relink_entry(&mut entries, &id, &new_path).unwrap();
        assert_eq!(entries[0].id, id);
        assert_eq!(entries[0].path, new_path.canonicalize().unwrap());
        assert!(entries[0].favorite);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rom_header_metadata_is_used_when_structurally_valid() {
        let dir = std::env::temp_dir().join(format!("vyntra-header-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("filename.gba");
        let mut bytes = vec![0u8; 0xc0];
        bytes[0xb2] = 0x96;
        bytes[0xa0..0xac].copy_from_slice(b"TEST ROM    ");
        bytes[0xac..0xb0].copy_from_slice(b"ABCD");
        fs::write(&path, bytes).unwrap();
        let entry = LibraryEntry::from_path(path.clone());
        assert_eq!(entry.title.as_deref(), Some("TEST ROM"));
        assert_eq!(entry.system.as_deref(), Some("GBA"));
        assert_eq!(entry.game_code.as_deref(), Some("ABCD"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn object_array_roundtrips() {
        let entry = LibraryEntry {
            id: "abc123".into(),
            path: PathBuf::from("C:/roms/a.gb"),
            content_hash: Some("abc123".into()),
            title: Some("A".into()),
            system: Some("GB".into()),
            game_code: None,
            revision: None,
            confidence: None,
            last_seen_unix: Some(1),
            favorite: false,
            last_played_unix: None,
        };
        let value = serde_json::json!([entry]);
        let parsed = parse_value(value).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "abc123");
    }

    #[test]
    fn save_is_atomic_and_loadable() {
        let dir = std::env::temp_dir().join(format!("vyntra-library-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("vyntra-library.json");
        let entries = vec![LibraryEntry {
            id: "id1".into(),
            path: PathBuf::from("C:/roms/a.gb"),
            content_hash: None,
            title: Some("A".into()),
            system: Some("GB".into()),
            game_code: None,
            revision: None,
            confidence: None,
            last_seen_unix: None,
            favorite: false,
            last_played_unix: None,
        }];
        save(&path, &entries).unwrap();
        assert!(!path.with_extension("tmp").exists());
        let loaded = load(&path).unwrap();
        assert_eq!(loaded[0].id, "id1");
        let _ = fs::remove_dir_all(&dir);
    }
}
