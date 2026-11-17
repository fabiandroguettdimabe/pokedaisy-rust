//! Deterministic identity helpers shared by the desktop shell and player.
//!
//! Content hashing is used to isolate saves/states from file-name collisions.
//! These helpers are intentionally small and side-effect free so they can be
//! reused by the upcoming catalog/manifest contracts (T03/T05).

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

/// SHA-256 of a file's full content, lowercase hex.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// SHA-256 of a byte buffer, lowercase hex.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Short stable storage key derived from the ROM content (fallback: file name).
pub fn storage_key(path: &Path) -> String {
    match sha256_file(path) {
        Ok(hash) => hash[..16].to_string(),
        Err(_) => sanitized_name_key(path),
    }
}

/// Best-effort deterministic key when the file cannot be read.
fn sanitized_name_key(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let mut key = String::from("name-");
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            key.push(c.to_ascii_lowercase());
        } else {
            key.push('_');
        }
    }
    if key.len() > 40 {
        key.truncate(40);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vyntra-identity-{}-{}", std::process::id(), name));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    #[test]
    fn sha256_is_stable_and_content_sensitive() {
        let a = temp_file("a.gb", b"rom-a");
        let b = temp_file("b.gb", b"rom-b");
        let ha = sha256_file(&a).unwrap();
        assert_eq!(ha.len(), 64);
        assert_eq!(ha, sha256_file(&a).unwrap());
        assert_ne!(ha, sha256_file(&b).unwrap());
    }

    #[test]
    fn storage_key_uses_content_not_name() {
        let a = temp_file("x.gb", b"same");
        let b = temp_file("y.gb", b"same");
        assert_eq!(storage_key(&a), storage_key(&b));
        let c = temp_file("x2.gb", b"different");
        assert_ne!(storage_key(&a), storage_key(&c));
    }

    #[test]
    fn missing_file_falls_back_to_sanitized_name() {
        let missing = std::env::temp_dir().join("Vyntra Missing ROM!.gb");
        let key = storage_key(&missing);
        assert!(key.starts_with("name-"));
        assert!(
            key.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }
}
