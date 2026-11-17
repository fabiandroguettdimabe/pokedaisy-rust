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

const SUPPORTED_ACTIONS: &[&str] = &[
    "ready",
    "scan",
    "reset_settings_section",
    "favorite",
    "game_note",
    "library_view",
    "library_filter",
    "library_selection",
    "relink",
    "type_matchup",
    "import",
    "import_roots",
    "cancel_import",
    "choose",
    "set",
    "play",
    "stop_player",
    "pause_player",
    "save_player_state",
    "load_player_state",
    "export_vault_bundle",
    "import_vault_bundle",
    "export_save",
    "migration_preview",
    "migration_apply",
    "probe_core",
    "open_folder",
];

#[derive(Clone, Copy)]
enum PayloadFieldType {
    String,
    Boolean,
    Number,
    Array,
    Any,
}

fn validate_versioned_payload(action: &str, payload: &Value) -> Result<(), AppError> {
    let Some(object) = payload.as_object() else {
        return Err(AppError::new(
            "invalid_payload",
            "El payload del comando debe ser un objeto JSON",
        ));
    };
    let (schema, allowed_fields): (&[(&str, PayloadFieldType)], &[&str]) = match action {
        "reset_settings_section" => (&[("section", PayloadFieldType::String)], &["section"]),
        "favorite" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("value", PayloadFieldType::Boolean),
            ],
            &["storage_id", "value"],
        ),
        "game_note" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("value", PayloadFieldType::String),
            ],
            &["storage_id", "value"],
        ),
        "library_view" | "library_filter" | "library_selection" => {
            (&[("value", PayloadFieldType::String)], &["value"])
        }
        "relink" | "play" => (&[("storage_id", PayloadFieldType::String)], &["storage_id"]),
        "type_matchup" => (
            &[
                ("attack", PayloadFieldType::Number),
                ("defenses", PayloadFieldType::Array),
            ],
            &["attack", "defenses"],
        ),
        "choose" | "open_folder" => (&[("field", PayloadFieldType::String)], &["field"]),
        "set" => (
            &[
                ("field", PayloadFieldType::String),
                ("value", PayloadFieldType::Any),
            ],
            &["field", "value", "index"],
        ),
        "pause_player" => (&[("value", PayloadFieldType::Boolean)], &["value"]),
        "export_vault_bundle" | "export_save" => (&[("file", PayloadFieldType::String)], &["file"]),
        "probe_core" => (&[("index", PayloadFieldType::Number)], &["index"]),
        _ => (&[], &[]),
    };

    if object
        .keys()
        .any(|field| !allowed_fields.contains(&field.as_str()))
    {
        return Err(AppError::new(
            "invalid_payload",
            "El payload contiene campos no permitidos para esta acción",
        ));
    }

    for (field, field_type) in schema {
        let value = object
            .get(*field)
            .ok_or_else(|| AppError::new("invalid_payload", format!("Falta el campo `{field}`")))?;
        let matches_type = match field_type {
            PayloadFieldType::String => value.is_string(),
            PayloadFieldType::Boolean => value.is_boolean(),
            PayloadFieldType::Number => value.as_u64().is_some(),
            PayloadFieldType::Array => value.is_array(),
            PayloadFieldType::Any => !value.is_null(),
        };
        if !matches_type {
            return Err(AppError::new(
                "invalid_payload",
                format!("El campo `{field}` tiene un tipo inválido"),
            ));
        }
    }

    let invalid_storage_id = || {
        object["storage_id"].as_str().is_none_or(|id| {
            id.is_empty()
                || id.len() > 64
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        })
    };
    let invalid = match action {
        "reset_settings_section" => !matches!(
            object["section"].as_str(),
            Some("general" | "folders" | "cores" | "av" | "input")
        ),
        "library_view" => !matches!(object["value"].as_str(), Some("grid" | "list")),
        "library_filter" => !matches!(
            object["value"].as_str(),
            Some("Todos" | "Favoritos" | "GB" | "GBC" | "GBA" | "DS")
        ),
        "library_selection" => object["value"].as_str().is_some_and(|value| {
            value.len() > 64
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        }),
        "game_note" => {
            invalid_storage_id()
                || object["value"].as_str().is_some_and(|note| {
                    note.len() > 2000
                        || note
                            .chars()
                            .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
                })
        }
        "favorite" | "relink" | "play" => invalid_storage_id(),
        "choose" => object["field"].as_str().is_some_and(|field| {
            !["library", "saves", "states", "system"].contains(&field)
                && !field
                    .strip_prefix("core")
                    .and_then(|index| index.parse::<usize>().ok())
                    .is_some_and(|index| index < 3)
        }),
        "open_folder" => !matches!(
            object["field"].as_str(),
            Some("config" | "library" | "saves" | "states" | "system")
        ),
        "export_vault_bundle" | "export_save" => object["file"]
            .as_str()
            .is_some_and(|path| path.is_empty() || path.len() > 4096),
        "type_matchup" => {
            object["attack"]
                .as_u64()
                .is_some_and(|value| value > u8::MAX as u64)
                || object["defenses"].as_array().is_some_and(|values| {
                    values
                        .iter()
                        .any(|value| value.as_u64().is_none_or(|id| id > u8::MAX as u64))
                })
        }
        "probe_core" => object["index"].as_u64().is_none_or(|index| index >= 3),
        "set" => {
            let field = object["field"].as_str().unwrap_or_default();
            let value = &object["value"];
            if field != "key" && object.contains_key("index") {
                return Err(AppError::new(
                    "invalid_payload",
                    "El campo `index` solo se admite al asignar una tecla",
                ));
            }
            match field {
                "volume" => value.as_u64().is_none_or(|number| number > 100),
                "smooth" | "fullscreen" | "recursive" | "reduced_motion" => !value.is_boolean(),
                "fast_forward" => value
                    .as_u64()
                    .is_none_or(|number| !(2..=4).contains(&number)),
                "autosave" => value
                    .as_u64()
                    .is_none_or(|number| !(10..=120).contains(&number)),
                "key" => {
                    value
                        .as_str()
                        .is_none_or(|key| key.is_empty() || key.len() > 32)
                        || object["index"].as_u64().is_none_or(|index| index >= 14)
                }
                "keys_reset" | "cores_reset" => !value.is_boolean(),
                _ => true,
            }
        }
        _ => false,
    };
    if invalid {
        return Err(AppError::new(
            "invalid_payload",
            "El payload contiene un valor fuera del contrato de la acción",
        ));
    }
    Ok(())
}

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

/// Parse a versioned command envelope, while accepting the prior flat action
/// shape during the frontend migration.
pub fn parse_ui_command(body: &str) -> Result<UiCommandEnvelope, AppError> {
    const MAX_LEGACY_BYTES: usize = 32 * 1024;
    if body.len() > MAX_LEGACY_BYTES {
        return Err(AppError::new("payload_too_large", "Mensaje demasiado grande").retryable());
    }
    let payload: Value = serde_json::from_str(body)
        .map_err(|e| AppError::new("invalid_json", format!("JSON inválido: {e}")))?;
    if payload.get("protocol_version").is_some() || payload.get("command_id").is_some() {
        if payload.get("command_id").is_some() && payload.get("protocol_version").is_none() {
            return Err(AppError::new(
                "missing_protocol_version",
                "Falta la versión del protocolo",
            ));
        }
        let command: UiCommandEnvelope = serde_json::from_value(payload.clone())
            .map_err(|e| AppError::new("invalid_envelope", format!("Envelope inválido: {e}")))?;
        if command.protocol_version != CONTRACT_VERSION {
            return Err(AppError::new(
                "unsupported_protocol_version",
                "Versión del protocolo no compatible",
            ));
        }
        if command.command_id.trim().is_empty() {
            return Err(AppError::new(
                "invalid_command_id",
                "Falta un command_id válido",
            ));
        }
        for (name, value) in [
            ("command_id", Some(command.command_id.as_str())),
            ("request_id", command.request_id.as_deref()),
            ("session_id", command.session_id.as_deref()),
        ] {
            if value.is_some_and(|value| {
                value.is_empty() || value.len() > 128 || value.chars().any(char::is_control)
            }) {
                return Err(AppError::new(
                    "invalid_envelope",
                    format!("El campo `{name}` no es válido"),
                ));
            }
        }
        let action = command.action.trim();
        if action.is_empty() || action != command.action {
            return Err(AppError::new("invalid_action", "Falta una acción válida"));
        }
        if !SUPPORTED_ACTIONS.contains(&action) {
            return Err(AppError::new("unknown_action", "Acción no disponible"));
        }
        validate_versioned_payload(action, &command.payload)?;
        return Ok(command);
    }
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .filter(|action| !action.trim().is_empty())
        .ok_or_else(|| AppError::new("invalid_action", "Falta una acción válida"))?;
    if !SUPPORTED_ACTIONS.contains(&action) {
        return Err(AppError::new("unknown_action", "Acción no disponible"));
    }
    let action = action.to_owned();
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

/// Compatibility entry point for callers and tests that still use the legacy
/// protocol name.
pub fn parse_legacy_action(body: &str) -> Result<UiCommandEnvelope, AppError> {
    parse_ui_command(body)
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
    fn versioned_envelope_parses_command_and_nested_payload() {
        let cmd = parse_ui_command(
            r#"{"protocol_version":1,"command_id":"ui-7","action":"favorite","request_id":"r-2","expected_revision":4,"payload":{"storage_id":"abc","value":true}}"#,
        )
        .unwrap();
        assert_eq!(cmd.command_id, "ui-7");
        assert_eq!(cmd.request_id.as_deref(), Some("r-2"));
        assert_eq!(cmd.expected_revision, Some(4));
        assert_eq!(cmd.payload["storage_id"], "abc");
        assert_eq!(cmd.payload["value"], true);
    }

    #[test]
    fn versioned_envelopes_reject_unsupported_versions_and_missing_ids() {
        for (body, code) in [
            (
                r#"{"protocol_version":2,"command_id":"x","action":"scan"}"#,
                "unsupported_protocol_version",
            ),
            (
                r#"{"command_id":"x","action":"scan","payload":{}}"#,
                "missing_protocol_version",
            ),
            (
                r#"{"protocol_version":1,"command_id":" ","action":"scan"}"#,
                "invalid_command_id",
            ),
            (
                r#"{"protocol_version":1,"command_id":"x","action":" scan ","payload":{}}"#,
                "invalid_action",
            ),
        ] {
            assert_eq!(parse_ui_command(body).unwrap_err().code, code);
        }

        let long_command_id = serde_json::json!({
            "protocol_version": 1,
            "command_id": "x".repeat(129),
            "action": "scan",
            "payload": {},
        })
        .to_string();
        assert_eq!(
            parse_ui_command(&long_command_id).unwrap_err().code,
            "invalid_envelope"
        );
        let control_request_id = serde_json::json!({
            "protocol_version": 1,
            "command_id": "ok",
            "request_id": "bad\nrequest",
            "action": "scan",
            "payload": {},
        })
        .to_string();
        assert_eq!(
            parse_ui_command(&control_request_id).unwrap_err().code,
            "invalid_envelope"
        );
    }

    #[test]
    fn versioned_payloads_validate_object_shape_and_action_fields() {
        let valid = r#"{"protocol_version":1,"command_id":"ok","action":"favorite","payload":{"storage_id":"g1","value":true}}"#;
        assert!(parse_ui_command(valid).is_ok());

        for body in [
            r#"{"protocol_version":1,"command_id":"x","action":"favorite","payload":[]}"#,
            r#"{"protocol_version":1,"command_id":"x","action":"favorite","payload":{"storage_id":"g1"}}"#,
            r#"{"protocol_version":1,"command_id":"x","action":"favorite","payload":{"storage_id":"g1","value":"yes"}}"#,
        ] {
            assert_eq!(parse_ui_command(body).unwrap_err().code, "invalid_payload");
        }

        assert!(
            parse_ui_command(
                r#"{"protocol_version":1,"command_id":"ready","action":"ready","payload":{}}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn versioned_payloads_reject_values_outside_action_contracts() {
        for (action, payload) in [
            ("library_view", serde_json::json!({"value":"tiles"})),
            ("library_filter", serde_json::json!({"value":"GBC+"})),
            (
                "type_matchup",
                serde_json::json!({"attack":256,"defenses":[2]}),
            ),
            ("set", serde_json::json!({"field":"volume","value":101})),
            ("set", serde_json::json!({"field":"autosave","value":5})),
            (
                "set",
                serde_json::json!({"field":"unknown_setting","value":true}),
            ),
            ("choose", serde_json::json!({"field":"core3"})),
            ("open_folder", serde_json::json!({"field":"arbitrary"})),
            ("play", serde_json::json!({"storage_id":"../game"})),
            (
                "favorite",
                serde_json::json!({"storage_id":"g1","value":true,"path":"C:/outside"}),
            ),
            ("probe_core", serde_json::json!({"index":3})),
            (
                "game_note",
                serde_json::json!({"storage_id":"g1","value":"bad\u{0001}note"}),
            ),
        ] {
            let body = serde_json::json!({
                "protocol_version": 1,
                "command_id": "invalid-value",
                "action": action,
                "payload": payload,
            })
            .to_string();
            assert_eq!(parse_ui_command(&body).unwrap_err().code, "invalid_payload");
        }
        for payload in [
            serde_json::json!({"field":"volume","value":25}),
            serde_json::json!({"field":"fast_forward","value":4}),
            serde_json::json!({"field":"autosave","value":120}),
            serde_json::json!({"field":"key","index":0,"value":"A"}),
            serde_json::json!({"field":"keys_reset","value":true}),
        ] {
            let body = serde_json::json!({
                "protocol_version": 1,
                "command_id": "valid-setting",
                "action": "set",
                "payload": payload,
            })
            .to_string();
            assert!(parse_ui_command(&body).is_ok());
        }
        for (action, payload) in [
            ("choose", serde_json::json!({"field":"core2"})),
            ("open_folder", serde_json::json!({"field":"system"})),
            ("play", serde_json::json!({"storage_id":"a1-b2"})),
            ("probe_core", serde_json::json!({"index":2})),
        ] {
            let body = serde_json::json!({
                "protocol_version": 1,
                "command_id": "valid-field",
                "action": action,
                "payload": payload,
            })
            .to_string();
            assert!(parse_ui_command(&body).is_ok());
        }
    }

    #[test]
    fn oversized_payload_rejected() {
        let big = format!(r#"{{"action":"x","d":"{}"}}"#, "a".repeat(40_000));
        let err = parse_legacy_action(&big).unwrap_err();
        assert_eq!(err.code, "payload_too_large");
    }

    #[test]
    fn malformed_and_truncated_json_return_a_structured_error() {
        for body in [r#"{"action":"scan""#, "{"] {
            let error = parse_legacy_action(body).unwrap_err();
            assert_eq!(error.code, "invalid_json");
            assert!(!error.reason.is_empty());
        }
    }

    #[test]
    fn missing_and_unknown_actions_are_rejected_with_stable_codes() {
        for (body, code) in [
            (r#"{}"#, "invalid_action"),
            (r#"{"action":"  "}"#, "invalid_action"),
            (r#"{"action":"run_shell"}"#, "unknown_action"),
        ] {
            assert_eq!(parse_legacy_action(body).unwrap_err().code, code);
        }
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
