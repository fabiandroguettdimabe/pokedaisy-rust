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
    "game_smoothing",
    "game_volume",
    "system_smoothing",
    "system_volume",
    "game_fast_forward",
    "system_fast_forward",
    "game_autosave",
    "system_autosave",
    "library_view",
    "library_filter",
    "library_selection",
    "relink",
    "type_matchup",
    "read_gen3_save",
    "refresh_gen3_save",
    "open_companion_window",
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
    NullableBoolean,
    NullableNumber,
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
        "game_smoothing" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableBoolean),
            ],
            &["storage_id", "value"],
        ),
        "system_smoothing" => (
            &[
                ("system", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableBoolean),
            ],
            &["system", "value"],
        ),
        "game_volume" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableNumber),
            ],
            &["storage_id", "value"],
        ),
        "system_volume" => (
            &[
                ("system", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableNumber),
            ],
            &["system", "value"],
        ),
        "game_fast_forward" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableNumber),
            ],
            &["storage_id", "value"],
        ),
        "system_fast_forward" => (
            &[
                ("system", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableNumber),
            ],
            &["system", "value"],
        ),
        "game_autosave" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableNumber),
            ],
            &["storage_id", "value"],
        ),
        "system_autosave" => (
            &[
                ("system", PayloadFieldType::String),
                ("value", PayloadFieldType::NullableNumber),
            ],
            &["system", "value"],
        ),
        "library_view" | "library_filter" | "library_selection" => {
            (&[("value", PayloadFieldType::String)], &["value"])
        }
        "relink" => (&[("storage_id", PayloadFieldType::String)], &["storage_id"]),
        "play" => (
            &[
                ("storage_id", PayloadFieldType::String),
                ("source_index", PayloadFieldType::Number),
            ],
            &["storage_id", "source_index"],
        ),
        "type_matchup" => (
            &[
                ("attack", PayloadFieldType::Number),
                ("defenses", PayloadFieldType::Array),
            ],
            &["attack", "defenses"],
        ),
        "read_gen3_save" | "refresh_gen3_save" => {
            (&[("format", PayloadFieldType::String)], &["format"])
        }
        "open_companion_window" => (&[], &[]),
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
            PayloadFieldType::NullableBoolean => value.is_boolean() || value.is_null(),
            PayloadFieldType::NullableNumber => value.is_null() || value.as_u64().is_some(),
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
            value.is_empty()
                || value.len() > 64
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
        "game_smoothing" => {
            invalid_storage_id() || !(object["value"].is_boolean() || object["value"].is_null())
        }
        "system_smoothing" => {
            !matches!(object["system"].as_str(), Some("GB" | "GBC" | "GBA" | "DS"))
        }
        "game_volume" => {
            invalid_storage_id() || object["value"].as_u64().is_some_and(|value| value > 100)
        }
        "system_volume" => {
            !matches!(object["system"].as_str(), Some("GB" | "GBC" | "GBA" | "DS"))
                || object["value"].as_u64().is_some_and(|value| value > 100)
        }
        "game_fast_forward" => {
            invalid_storage_id()
                || object["value"]
                    .as_u64()
                    .is_some_and(|value| !(2..=4).contains(&value))
        }
        "system_fast_forward" => {
            !matches!(object["system"].as_str(), Some("GB" | "GBC" | "GBA" | "DS"))
                || object["value"]
                    .as_u64()
                    .is_some_and(|value| !(2..=4).contains(&value))
        }
        "game_autosave" => {
            invalid_storage_id()
                || object["value"]
                    .as_u64()
                    .is_some_and(|value| !matches!(value, 10 | 20 | 30 | 60 | 120))
        }
        "system_autosave" => {
            !matches!(object["system"].as_str(), Some("GB" | "GBC" | "GBA" | "DS"))
                || object["value"]
                    .as_u64()
                    .is_some_and(|value| !matches!(value, 10 | 20 | 30 | 60 | 120))
        }
        "favorite" | "relink" => invalid_storage_id(),
        "play" => {
            invalid_storage_id()
                || object["source_index"]
                    .as_u64()
                    .is_none_or(|index| index > u32::MAX as u64)
        }
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
                    values.is_empty()
                        || values.len() > 2
                        || values
                            .iter()
                            .any(|value| value.as_u64().is_none_or(|id| id > u8::MAX as u64))
                })
        }
        "probe_core" => object["index"].as_u64().is_none_or(|index| index >= 3),
        "read_gen3_save" | "refresh_gen3_save" => !matches!(
            object["format"].as_str(),
            Some("ruby_sapphire" | "emerald" | "firered_leafgreen")
        ),
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
#[serde(deny_unknown_fields)]
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
    /// Build the terminal acknowledgement for an accepted asynchronous command.
    pub fn terminal(
        command_id: impl Into<String>,
        request_id: Option<String>,
        result: Result<Option<Value>, AppError>,
    ) -> Self {
        let mut ack = match result {
            Ok(result) => Self::success(command_id, result),
            Err(error) => Self::failure(command_id, error),
        };
        ack.request_id = request_id;
        ack
    }

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

/// Parse and validate a versioned command envelope.
pub fn parse_ui_command(body: &str) -> Result<UiCommandEnvelope, AppError> {
    const MAX_ENVELOPE_BYTES: usize = 32 * 1024;
    if body.len() > MAX_ENVELOPE_BYTES {
        return Err(AppError::new("payload_too_large", "Mensaje demasiado grande").retryable());
    }
    let payload: Value = serde_json::from_str(body)
        .map_err(|e| AppError::new("invalid_json", format!("JSON inválido: {e}")))?;
    if payload.get("protocol_version").is_none() {
        return Err(AppError::new(
            "missing_protocol_version",
            "Falta la versión del protocolo",
        ));
    }
    if payload.get("action").is_none() {
        return Err(AppError::new("invalid_action", "Falta una acción válida"));
    }
    let command: UiCommandEnvelope = serde_json::from_value(payload)
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
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn flat_commands_are_rejected_without_protocol_version() {
        let error = parse_ui_command(r#"{"action":"scan"}"#).unwrap_err();
        assert_eq!(error.code, "missing_protocol_version");
    }

    #[test]
    fn versioned_envelopes_reject_unsupported_versions_and_missing_ids() {
        for (body, code) in [
            (
                r#"{"protocol_version":1,"command_id":"x","payload":{}}"#,
                "invalid_action",
            ),
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
            (
                r#"{"protocol_version":1,"command_id":"x","action":"scan","payload":{},"debug_path":"C:/private"}"#,
                "invalid_envelope",
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
    fn play_requires_bounded_source_index_with_storage_identity() {
        for payload in [
            serde_json::json!({"storage_id":"g1"}),
            serde_json::json!({"storage_id":"g1","source_index":-1}),
            serde_json::json!({"storage_id":"g1","source_index":4294967296u64}),
        ] {
            let body = serde_json::json!({"protocol_version":1,"command_id":"play-source","action":"play","payload":payload}).to_string();
            assert_eq!(parse_ui_command(&body).unwrap_err().code, "invalid_payload");
        }
        let body = r#"{"protocol_version":1,"command_id":"play-source-ok","action":"play","payload":{"storage_id":"g1","source_index":2}}"#;
        assert!(parse_ui_command(body).is_ok());
    }

    #[test]
    fn versioned_payloads_reject_values_outside_action_contracts() {
        for (action, payload) in [
            ("library_view", serde_json::json!({"value":"tiles"})),
            ("library_filter", serde_json::json!({"value":"GBC+"})),
            ("library_selection", serde_json::json!({"value":""})),
            (
                "type_matchup",
                serde_json::json!({"attack":256,"defenses":[2]}),
            ),
            (
                "type_matchup",
                serde_json::json!({"attack":1,"defenses":[]}),
            ),
            (
                "type_matchup",
                serde_json::json!({"attack":1,"defenses":[1,2,3]}),
            ),
            ("set", serde_json::json!({"field":"volume","value":101})),
            ("set", serde_json::json!({"field":"autosave","value":5})),
            (
                "set",
                serde_json::json!({"field":"unknown_setting","value":true}),
            ),
            ("choose", serde_json::json!({"field":"core3"})),
            ("open_folder", serde_json::json!({"field":"arbitrary"})),
            (
                "play",
                serde_json::json!({"storage_id":"../game","source_index":0}),
            ),
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
            (
                "play",
                serde_json::json!({"storage_id":"a1-b2","source_index":0}),
            ),
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
    fn every_supported_action_accepts_its_declared_versioned_payload() {
        for action in SUPPORTED_ACTIONS {
            let payload = match *action {
                "reset_settings_section" => serde_json::json!({"section":"general"}),
                "favorite" => serde_json::json!({"storage_id":"g1","value":true}),
                "game_note" => serde_json::json!({"storage_id":"g1","value":"note"}),
                "game_smoothing" => serde_json::json!({"storage_id":"g1","value":false}),
                "system_smoothing" => serde_json::json!({"system":"GBA","value":false}),
                "game_volume" => serde_json::json!({"storage_id":"g1","value":40}),
                "system_volume" => serde_json::json!({"system":"GBA","value":40}),
                "game_fast_forward" => serde_json::json!({"storage_id":"g1","value":3}),
                "system_fast_forward" => serde_json::json!({"system":"GBA","value":3}),
                "game_autosave" => serde_json::json!({"storage_id":"g1","value":30}),
                "system_autosave" => serde_json::json!({"system":"GBA","value":30}),
                "library_view" => serde_json::json!({"value":"grid"}),
                "library_filter" => serde_json::json!({"value":"Todos"}),
                "library_selection" => serde_json::json!({"value":"g1"}),
                "relink" => serde_json::json!({"storage_id":"g1"}),
                "play" => serde_json::json!({"storage_id":"g1","source_index":0}),
                "type_matchup" => serde_json::json!({"attack":10,"defenses":[12,8]}),
                "read_gen3_save" => serde_json::json!({"format":"emerald"}),
                "refresh_gen3_save" => serde_json::json!({"format":"emerald"}),
                "open_companion_window" => serde_json::json!({}),
                "choose" => serde_json::json!({"field":"library"}),
                "set" => serde_json::json!({"field":"volume","value":80}),
                "pause_player" => serde_json::json!({"value":false}),
                "export_vault_bundle" | "export_save" => {
                    serde_json::json!({"file":"C:/saves/game.sav"})
                }
                "probe_core" => serde_json::json!({"index":0}),
                "open_folder" => serde_json::json!({"field":"config"}),
                _ => serde_json::json!({}),
            };
            let body = serde_json::json!({
                "protocol_version": CONTRACT_VERSION,
                "command_id": format!("schema-{action}"),
                "action": action,
                "payload": payload,
            })
            .to_string();
            assert!(
                parse_ui_command(&body).is_ok(),
                "supported action `{action}` rejected its declared payload"
            );
        }
    }

    #[test]
    fn game_smoothing_accepts_override_or_global_inheritance_only() {
        for value in [
            serde_json::json!(true),
            serde_json::json!(false),
            Value::Null,
        ] {
            let body = serde_json::json!({
                "protocol_version": CONTRACT_VERSION,
                "command_id": "game-smoothing",
                "action": "game_smoothing",
                "payload": {"storage_id":"a1b2c3d4e5f60718","value":value},
            })
            .to_string();
            assert!(parse_ui_command(&body).is_ok());
        }
        let invalid = r#"{"protocol_version":1,"command_id":"bad-smoothing","action":"game_smoothing","payload":{"storage_id":"a1b2c3d4e5f60718","value":"yes"}}"#;
        assert_eq!(
            parse_ui_command(invalid).unwrap_err().code,
            "invalid_payload"
        );
    }

    #[test]
    fn system_smoothing_accepts_only_known_systems_and_nullable_booleans() {
        for value in [
            serde_json::json!(true),
            serde_json::json!(false),
            Value::Null,
        ] {
            let body = serde_json::json!({
                "protocol_version": CONTRACT_VERSION,
                "command_id": "system-smoothing",
                "action": "system_smoothing",
                "payload": {"system":"GBA", "value":value},
            })
            .to_string();
            assert!(parse_ui_command(&body).is_ok());
        }
        let invalid = r#"{"protocol_version":1,"command_id":"bad-system-smoothing","action":"system_smoothing","payload":{"system":"PC","value":false}}"#;
        assert_eq!(
            parse_ui_command(invalid).unwrap_err().code,
            "invalid_payload"
        );
    }

    #[test]
    fn oversized_payload_rejected() {
        let big = format!(r#"{{"action":"x","d":"{}"}}"#, "a".repeat(40_000));
        let err = parse_ui_command(&big).unwrap_err();
        assert_eq!(err.code, "payload_too_large");
    }

    #[test]
    fn malformed_and_truncated_json_return_a_structured_error() {
        for body in [r#"{"action":"scan""#, "{"] {
            let error = parse_ui_command(body).unwrap_err();
            assert_eq!(error.code, "invalid_json");
            assert!(!error.reason.is_empty());
        }
    }

    #[test]
    fn missing_and_unknown_actions_are_rejected_with_stable_codes() {
        for (body, code) in [
            (
                r#"{"protocol_version":1,"command_id":"x","action":"  ","payload":{}}"#,
                "invalid_action",
            ),
            (
                r#"{"protocol_version":1,"command_id":"x","action":"run_shell","payload":{}}"#,
                "unknown_action",
            ),
        ] {
            assert_eq!(parse_ui_command(body).unwrap_err().code, code);
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
    fn asynchronous_terminal_ack_preserves_command_and_request_ids_on_failure() {
        let ack = UiAck::terminal(
            "vault-export-17",
            Some("request-42".into()),
            Err(AppError::new(
                "command_failed",
                "No se pudo escribir el paquete",
            )),
        );
        let encoded = serde_json::to_value(ack).unwrap();
        assert_eq!(encoded["command_id"], "vault-export-17");
        assert_eq!(encoded["request_id"], "request-42");
        assert_eq!(encoded["ok"], false);
        assert_eq!(encoded["error"]["code"], "command_failed");

        let success = UiAck::terminal(
            "vault-import-18",
            Some("request-43".into()),
            Ok(Some(serde_json::json!({"vault_import":{"restored":true}}))),
        );
        assert!(success.ok);
        assert_eq!(success.command_id, "vault-import-18");
        assert_eq!(success.request_id.as_deref(), Some("request-43"));
        assert_eq!(success.result.unwrap()["vault_import"]["restored"], true);
    }

    #[test]
    fn command_ack_cache_replays_only_the_same_revisioned_command() {
        let command = parse_ui_command(
            r#"{"protocol_version":1,"command_id":"scan-1","action":"scan","expected_revision":7,"payload":{}}"#,
        )
        .unwrap();
        let mut cache = CommandAckCache::new(2);
        let ack = UiAck::success("scan-1", Some(serde_json::json!({"started":true})));
        cache.remember(&command, ack.clone());

        assert_eq!(cache.lookup(&command), Some(&ack));
        let next_revision = parse_ui_command(
            r#"{"protocol_version":1,"command_id":"scan-2","action":"scan","expected_revision":8,"payload":{}}"#,
        )
        .unwrap();
        assert!(cache.lookup(&next_revision).is_none());
        let replayed_command = parse_ui_command(
            r#"{"protocol_version":1,"command_id":"scan-3","action":"scan","session_id":"other-session","expected_revision":7,"payload":{}}"#,
        )
        .unwrap();
        assert!(cache.lookup(&replayed_command).is_none());

        let second = parse_ui_command(r#"{"protocol_version":1,"command_id":"scan-2","action":"scan","expected_revision":8,"payload":{}}"#).unwrap();
        let third = parse_ui_command(r#"{"protocol_version":1,"command_id":"scan-3","action":"scan","expected_revision":9,"payload":{}}"#).unwrap();
        cache.remember(&second, UiAck::success("second", None));
        cache.remember(&third, UiAck::success("third", None));
        assert!(cache.lookup(&command).is_none());
        assert_eq!(
            cache.lookup(&third).map(|ack| ack.command_id.as_str()),
            Some("third")
        );
    }
}
