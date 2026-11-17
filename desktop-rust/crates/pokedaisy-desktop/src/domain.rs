//! Domain types aligned with `docs/VYNTRA_PLAN_TECNICO.md` (proposed contracts).
//!
//! This module is an incremental F1/INFRA-01 scaffold: it introduces stable,
//! versionable types without changing current emulator behavior.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Protocol/schema version for future desktop contracts (UiCommand/UiAck, events).
pub const CONTRACT_VERSION: u32 = 1;

/// Confidence in a ROM/game identity match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IdentityConfidence {
    /// Exact hash or authoritative signature match.
    Strict,
    /// Partial/header/heuristic match; must not enable strict live adapters by itself.
    Partial,
    /// Not enough evidence to identify the game.
    #[default]
    Unknown,
}

/// Platform family used by the desktop frontend/catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Gb,
    Gbc,
    Gba,
    Nds,
}

impl Platform {
    pub fn from_system_label(label: &str) -> Option<Self> {
        match label.to_ascii_uppercase().as_str() {
            "GB" => Some(Self::Gb),
            "GBC" => Some(Self::Gbc),
            "GBA" => Some(Self::Gba),
            "DS" | "NDS" => Some(Self::Nds),
            _ => None,
        }
    }
}

/// Proposed game identity (hash-first). Fields may be unknown for legacy scans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameIdentity {
    pub platform: Platform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_crc32: Option<String>,
    #[serde(default)]
    pub confidence: IdentityConfidence,
}

/// Where a ROM comes from (file, archive entry, SAF-like locator in future).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomSource {
    pub source_id: String,
    pub locator: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_entry: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Immutable identity for a running session (player/core/save ownership).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub session_id: String,
    pub game_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rom_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    /// Incremented on loads/rewinds/restores that invalidate in-flight snapshots.
    #[serde(default)]
    pub epoch: u64,
}

/// Capability reporting: detected presence is not the same as verified support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Declared,
    Detected,
    Verified,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityReport {
    pub feature: String,
    pub state: CapabilityState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
}

/// Kind of persisted save artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind {
    BatteryRam,
    SaveState,
    CompanionCopy,
    ExportBundle,
}

/// Proposed manifest entry for a persisted save/state (not yet wired to runtime).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveManifest {
    pub save_id: String,
    pub kind: SaveKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rom_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix: Option<u64>,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_from_system_label() {
        assert_eq!(Platform::from_system_label("gb"), Some(Platform::Gb));
        assert_eq!(Platform::from_system_label("DS"), Some(Platform::Nds));
        assert_eq!(Platform::from_system_label("x"), None);
    }

    #[test]
    fn capability_states_serialize_stably() {
        let r = CapabilityReport {
            feature: "savestate".into(),
            state: CapabilityState::Detected,
            scope: Some("core".into()),
            reason: None,
            evidence_id: None,
        };
        let j = serde_json::to_value(r).unwrap();
        assert_eq!(j["state"], "detected");
    }
}
