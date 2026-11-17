//! IPC/UI protocol scaffold for `docs/VYNTRA_PLAN_TECNICO.md` (INFRA-01/T02).
//!
//! The current shell uses ad-hoc JSON actions (`{"action": ...}`) and publishes
//! state snapshots. This module defines versioned envelope types to migrate
//! toward explicit commands, acknowledgements and structured errors without
//! breaking the existing frontend immediately.

use crate::domain::CONTRACT_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

/// Application-level error with a stable code for UI handling and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    /// Stable machine code, e.g. `invalid_request`, `unsupported`, `io_error`.
    pub code: String,
    /// Human-readable reason (should be localizable at presentation boundaries).
    pub reason: String,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AppError {
    pub fn new(code: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            reason: reason.into(),
            retryable: false,
            details: None,
        }
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }
}

/// Versioned command envelope (UI/shell -> application service).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiCommandEnvelope {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    pub command_id: String,
    pub action: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub epoch: Option<u64>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
    #[serde(default)]
    pub payload: Value,
}

/// Acknowledgement for a command (accepted/rejected/completed as applicable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiAck {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    pub command_id: String,
    #[serde(default)]
    pub request_id: Option<String>,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
}

/// Bounded replay cache for revisioned commands. It lets a client recover the
/// original acknowledgement after a lost response without repeating effects.
pub struct CommandAckCache {
    capacity: usize,
    entries: VecDeque<(String, UiAck)>,
}

impl CommandAckCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
        }
    }

    pub fn lookup(&self, command: &UiCommandEnvelope) -> Option<&UiAck> {
        let key = command_dedupe_key(command)?;
        self.entries
            .iter()
            .find_map(|(cached, ack)| (cached == &key).then_some(ack))
    }

    pub fn remember(&mut self, command: &UiCommandEnvelope, ack: UiAck) {
        let Some(key) = command_dedupe_key(command) else {
            return;
        };
        self.entries.retain(|(cached, _)| cached != &key);
        self.entries.push_back((key, ack));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }
}

fn command_dedupe_key(command: &UiCommandEnvelope) -> Option<String> {
    command.expected_revision?;
    serde_json::to_string(&serde_json::json!({
        "expected_revision": command.expected_revision,
        "session_id": command.session_id,
        "epoch": command.epoch,
        "request_id": command.request_id,
        "action": command.action,
        "payload": command.payload,
    }))
    .ok()
}

/// Event envelope (application/worker -> UI subscribers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
    pub event_type: String,
    pub event_id: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub epoch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default)]
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monotonic_ms: Option<u64>,
}

pub fn default_protocol_version() -> u32 {
    CONTRACT_VERSION
}

impl UiAck {
    pub fn success(command_id: impl Into<String>, result: Option<Value>) -> Self {
        Self {
            protocol_version: CONTRACT_VERSION,
            command_id: command_id.into(),
            request_id: None,
            ok: true,
            result,
            error: None,
            revision: None,
        }
    }

    pub fn failure(command_id: impl Into<String>, error: AppError) -> Self {
        Self {
            protocol_version: CONTRACT_VERSION,
            command_id: command_id.into(),
            request_id: None,
            ok: false,
            result: None,
            error: Some(error),
            revision: None,
        }
    }
}

/// Parse a legacy action payload (`{"action":"..."}`) into a best-effort command
/// envelope. This supports incremental migration of `shell.rs` handlers.
pub fn parse_legacy_action(body: &str) -> Result<UiCommandEnvelope, AppError> {
    const MAX_LEGACY_BYTES: usize = 32 * 1024;
    if body.len() > MAX_LEGACY_BYTES {
        return Err(AppError::new("payload_too_large", "Mensaje demasiado grande").retryable());
    }
    let payload: Value = serde_json::from_str(body)
        .map_err(|e| AppError::new("invalid_json", format!("JSON inválido: {e}")))?;
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(UiCommandEnvelope {
        protocol_version: CONTRACT_VERSION,
        command_id: format!("legacy-{action}"),
        action,
        request_id: payload
            .get("request_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        session_id: payload
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        epoch: payload.get("epoch").and_then(|v| v.as_u64()),
        expected_revision: payload.get("expected_revision").and_then(|v| v.as_u64()),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_action_parses_and_preserves_payload() {
        let cmd = parse_legacy_action(r#"{"action":"scan","x":1}"#).unwrap();
        assert_eq!(cmd.action, "scan");
        assert_eq!(cmd.command_id, "legacy-scan");
        assert_eq!(cmd.payload["x"], 1);
    }

    #[test]
    fn oversized_payload_rejected() {
        let big = format!(r#"{{"action":"x","d":"{}"}}"#, "a".repeat(40_000));
        let err = parse_legacy_action(&big).unwrap_err();
        assert_eq!(err.code, "payload_too_large");
    }

    #[test]
    fn ack_helpers() {
        let ok = UiAck::success("c1", None);
        assert!(ok.ok);
        let bad = UiAck::failure("c2", AppError::new("x", "y"));
        assert!(!bad.ok);
    }

    #[test]
    fn command_ack_cache_replays_only_the_same_revisioned_command() {
        let command = parse_legacy_action(
            r#"{"action":"import_roots","expected_revision":7,"roots":["C:/roms"]}"#,
        )
        .unwrap();
        let mut cache = CommandAckCache::new(2);
        let ack = UiAck::success(
            "legacy-import_roots",
            Some(serde_json::json!({"started":true})),
        );
        cache.remember(&command, ack.clone());

        assert_eq!(cache.lookup(&command), Some(&ack));
        let next_revision = parse_legacy_action(
            r#"{"action":"import_roots","expected_revision":8,"roots":["C:/roms"]}"#,
        )
        .unwrap();
        assert!(cache.lookup(&next_revision).is_none());
        let legacy = parse_legacy_action(r#"{"action":"import_roots"}"#).unwrap();
        cache.remember(&legacy, ack);
        assert!(cache.lookup(&legacy).is_none());

        let second = parse_legacy_action(r#"{"action":"scan","expected_revision":8}"#).unwrap();
        let third = parse_legacy_action(r#"{"action":"scan","expected_revision":9}"#).unwrap();
        cache.remember(&second, UiAck::success("second", None));
        cache.remember(&third, UiAck::success("third", None));
        assert!(cache.lookup(&command).is_none());
        assert_eq!(
            cache.lookup(&third).map(|ack| ack.command_id.as_str()),
            Some("third")
        );
    }
}
