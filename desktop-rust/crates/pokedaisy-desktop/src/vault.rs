//! Compatibility checks for persisted state artifacts.
use crate::{
    domain::{SaveKind, SaveManifest},
    identity,
};

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
