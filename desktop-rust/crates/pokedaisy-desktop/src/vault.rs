//! Compatibility checks for persisted state artifacts.
use crate::{
    domain::{SaveKind, SaveManifest},
    identity,
};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;

pub const MAX_BUNDLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_BUNDLE_PAYLOAD_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveBundleManifest {
    pub schema_version: u32,
    pub storage_id: String,
    pub kind: SaveKind,
    pub rom_sha256: String,
    pub core_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_format: Option<String>,
    pub bytes: u64,
    pub sha256: String,
}

fn validate_bundle_manifest(manifest: &SaveBundleManifest, payload: &[u8]) -> Result<(), String> {
    let is_hex = |value: &str, length| {
        value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    };
    if manifest.schema_version != 1
        || !is_hex(&manifest.storage_id, 16)
        || !is_hex(&manifest.rom_sha256, 64)
        || !is_hex(&manifest.core_hash, 64)
        || !is_hex(&manifest.sha256, 64)
        || manifest.bytes != payload.len() as u64
        || manifest.bytes > MAX_BUNDLE_PAYLOAD_BYTES
    {
        return Err("El manifiesto del paquete Vault no es válido".into());
    }
    if !manifest
        .storage_id
        .eq_ignore_ascii_case(&manifest.rom_sha256[..16])
    {
        return Err("La identidad estable no corresponde al hash de la ROM".into());
    }
    match manifest.kind {
        SaveKind::SaveState
            if manifest.state_format.as_deref() != Some("libretro-serialize-v1") =>
        {
            return Err("El formato de estado del paquete no es compatible".into());
        }
        SaveKind::BatteryRam if manifest.state_format.is_some() => {
            return Err("El paquete SRAM no debe declarar formato de estado".into());
        }
        SaveKind::SaveState | SaveKind::BatteryRam => {}
        _ => return Err("El paquete no contiene un guardado restaurable".into()),
    }
    if identity::sha256_bytes(payload) != manifest.sha256 {
        return Err("El hash del contenido del paquete no coincide".into());
    }
    Ok(())
}

pub fn encode_bundle(manifest: &SaveBundleManifest, payload: &[u8]) -> Result<Vec<u8>, String> {
    validate_bundle_manifest(manifest, payload)?;
    let manifest_bytes = serde_json::to_vec(manifest).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    archive
        .start_file("manifest.json", options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(&manifest_bytes)
        .map_err(|error| error.to_string())?;
    archive
        .start_file("payload.bin", options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(payload)
        .map_err(|error| error.to_string())?;
    let bytes = archive
        .finish()
        .map_err(|error| error.to_string())?
        .into_inner();
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err("El paquete Vault supera el límite de tamaño".into());
    }
    Ok(bytes)
}

pub fn decode_bundle(bytes: &[u8]) -> Result<(SaveBundleManifest, Vec<u8>), String> {
    if bytes.len() as u64 > MAX_BUNDLE_BYTES {
        return Err("El paquete Vault supera el límite de tamaño".into());
    }
    let archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    decode_bundle_archive(archive)
}

fn decode_bundle_archive<R: Read + Seek>(
    mut archive: zip::ZipArchive<R>,
) -> Result<(SaveBundleManifest, Vec<u8>), String> {
    if archive.len() != 2 {
        return Err("El paquete Vault debe contener solo manifiesto y contenido".into());
    }
    let mut names = (0..archive.len())
        .map(|index| archive.by_index(index).map(|entry| entry.name().to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    names.sort();
    if names != ["manifest.json", "payload.bin"] {
        return Err("El paquete Vault contiene rutas inesperadas".into());
    }
    let mut manifest_bytes = Vec::new();
    archive
        .by_name("manifest.json")
        .map_err(|error| error.to_string())?
        .take(64 * 1024)
        .read_to_end(&mut manifest_bytes)
        .map_err(|error| error.to_string())?;
    let manifest: SaveBundleManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let payload_file = archive
        .by_name("payload.bin")
        .map_err(|error| error.to_string())?;
    if payload_file.size() > MAX_BUNDLE_PAYLOAD_BYTES || payload_file.size() != manifest.bytes {
        return Err("El tamaño del contenido del paquete no es válido".into());
    }
    let mut payload = Vec::with_capacity(payload_file.size() as usize);
    payload_file
        .take(MAX_BUNDLE_PAYLOAD_BYTES + 1)
        .read_to_end(&mut payload)
        .map_err(|error| error.to_string())?;
    validate_bundle_manifest(&manifest, &payload)?;
    Ok((manifest, payload))
}

pub fn read_bundle_file(path: &Path) -> Result<(SaveBundleManifest, Vec<u8>), String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    if size > MAX_BUNDLE_BYTES {
        return Err("El paquete Vault supera el límite de tamaño".into());
    }
    let archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    decode_bundle_archive(archive)
}

pub fn validate_state(
    manifest: &SaveManifest,
    bytes: &[u8],
    expected_rom_hash: &str,
    expected_core_hash: &str,
) -> Result<(), String> {
    if manifest.status != "committed" || manifest.kind != SaveKind::SaveState {
        return Err("El manifiesto no describe un estado confirmado".into());
    }
    if manifest.rom_sha256.as_deref() != Some(expected_rom_hash) {
        return Err("El estado pertenece a otra ROM o su identidad no está verificada".into());
    }
    if manifest.core_hash.as_deref() != Some(expected_core_hash) {
        return Err("El estado pertenece a otra versión de núcleo".into());
    }
    if manifest.state_format.as_deref() != Some("libretro-serialize-v1") {
        return Err("El formato del estado no es compatible".into());
    }
    if manifest.bytes != Some(bytes.len() as u64) {
        return Err("El tamaño del estado no coincide con su manifiesto".into());
    }
    if manifest.sha256.as_deref() != Some(identity::sha256_bytes(bytes).as_str()) {
        return Err("El hash del estado no coincide con su manifiesto".into());
    }
    Ok(())
}

pub fn validate_battery(
    manifest: &SaveManifest,
    bytes: &[u8],
    expected_rom_hash: &str,
    expected_core_hash: &str,
) -> Result<(), String> {
    if manifest.status != "committed" || manifest.kind != SaveKind::BatteryRam {
        return Err("El manifiesto no describe una SRAM confirmada".into());
    }
    if manifest.rom_sha256.as_deref() != Some(expected_rom_hash) {
        return Err("La SRAM pertenece a otra ROM o su identidad no está verificada".into());
    }
    if manifest.core_hash.as_deref() != Some(expected_core_hash) {
        return Err("La SRAM pertenece a otra versión de núcleo".into());
    }
    if manifest.bytes != Some(bytes.len() as u64) {
        return Err("El tamaño de la SRAM no coincide con su manifiesto".into());
    }
    if manifest.sha256.as_deref() != Some(identity::sha256_bytes(bytes).as_str()) {
        return Err("El hash de la SRAM no coincide con su manifiesto".into());
    }
    Ok(())
}

#[cfg(test)]
mod bundle_tests {
    use super::{SaveBundleManifest, decode_bundle, encode_bundle, read_bundle_file};
    use crate::{domain::SaveKind, identity};

    fn fixture_manifest(payload: &[u8]) -> SaveBundleManifest {
        SaveBundleManifest {
            schema_version: 1,
            storage_id: "aaaaaaaaaaaaaaaa".into(),
            kind: SaveKind::SaveState,
            rom_sha256: "a".repeat(64),
            core_hash: "b".repeat(64),
            state_format: Some("libretro-serialize-v1".into()),
            bytes: payload.len() as u64,
            sha256: identity::sha256_bytes(payload),
        }
    }

    #[test]
    fn save_bundle_roundtrips_with_hash_and_identity_manifest() {
        let payload = b"portable state";
        let manifest = fixture_manifest(payload);
        let bytes = encode_bundle(&manifest, payload).unwrap();
        let (decoded, content) = decode_bundle(&bytes).unwrap();
        assert_eq!(decoded, manifest);
        assert_eq!(content, payload);
        let path = std::env::temp_dir().join(format!("vyntra-bundle-{}.zip", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let (from_file, file_content) = read_bundle_file(&path).unwrap();
        assert_eq!(from_file, manifest);
        assert_eq!(file_content, payload);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_bundle_rejects_payload_hash_mismatch() {
        let payload = b"portable state";
        let mut manifest = fixture_manifest(payload);
        manifest.sha256 = "0".repeat(64);
        assert!(encode_bundle(&manifest, payload).is_err());
    }

    #[test]
    fn save_bundle_rejects_payload_corruption_detected_by_zip_crc() {
        use std::io::Write;

        let payload = b"verified save payload";
        let manifest = fixture_manifest(payload);
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        writer.start_file("payload.bin", options).unwrap();
        writer.write_all(payload).unwrap();
        let mut bytes = writer.finish().unwrap().into_inner();

        let payload_start = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice()))
            .unwrap()
            .by_name("payload.bin")
            .unwrap()
            .data_start() as usize;
        bytes[payload_start + payload.len() / 2] ^= 0xff;

        assert!(decode_bundle(&bytes).is_err());
    }
}
