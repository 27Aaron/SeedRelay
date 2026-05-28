use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use uuid::Uuid;

pub const REALTIME_PATH: &str = "/v1/realtime";
pub const MODEL_OBJECT: &str = "model";
pub const MODEL_OWNER: &str = "seedrelay";
pub const MODEL_CREATED_AT: u64 = 0;
pub const MIN_INPUT_SAMPLE_RATE: u32 = 8_000;
pub const MAX_INPUT_SAMPLE_RATE: u32 = 96_000;
const MAX_CLIENT_EVENT_BYTES: usize = 768 * 1024;
const MAX_APPEND_AUDIO_BYTES: usize = 512 * 1024;
const MAX_APPEND_AUDIO_BASE64_BYTES: usize = MAX_APPEND_AUDIO_BYTES.div_ceil(3) * 4;

#[derive(Debug, Eq, PartialEq)]
pub enum ClientEvent {
    SessionUpdate(SessionUpdateConfig),
    AppendAudio(Vec<u8>),
    Commit,
    Clear,
    Close,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SessionUpdateConfig {
    pub input_sample_rate: Option<u32>,
    pub input_audio_format_type: Option<String>,
    pub transcription_model: Option<String>,
    pub language: Option<String>,
    pub delay: Option<String>,
    pub include: Vec<String>,
}

pub fn validate_realtime_target(target: &str, model: &str) -> Result<()> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != REALTIME_PATH {
        return Err(anyhow!("expected {REALTIME_PATH}, got {path}"));
    }

    if query_param_from_query(query, "model").as_deref() != Some(model) {
        return Err(anyhow!("only model={model} is supported"));
    }

    Ok(())
}

pub fn decode_client_event(raw: &str) -> Result<ClientEvent> {
    if raw.len() > MAX_CLIENT_EVENT_BYTES {
        return Err(anyhow!("client event too large"));
    }

    let value: Value =
        serde_json::from_str(raw).map_err(|error| anyhow!("invalid JSON: {error}"))?;
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("client event missing type"))?;

    match event_type {
        "session.update" => Ok(ClientEvent::SessionUpdate(parse_session_update(&value)?)),
        "input_audio_buffer.append" | "session.input_audio_buffer.append" => {
            let audio = value
                .get("audio")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("append event missing audio"))?;
            if audio.len() > MAX_APPEND_AUDIO_BASE64_BYTES {
                return Err(anyhow!("audio payload too large"));
            }
            let bytes = STANDARD
                .decode(audio)
                .map_err(|error| anyhow!("invalid base64 audio: {error}"))?;
            if bytes.len() > MAX_APPEND_AUDIO_BYTES {
                return Err(anyhow!("audio payload too large"));
            }
            Ok(ClientEvent::AppendAudio(bytes))
        }
        "input_audio_buffer.commit" | "session.input_audio_buffer.commit" => {
            Ok(ClientEvent::Commit)
        }
        "input_audio_buffer.clear" | "session.input_audio_buffer.clear" => Ok(ClientEvent::Clear),
        "session.close" => Ok(ClientEvent::Close),
        other => Err(anyhow!("unsupported client event type: {other}")),
    }
}

fn parse_session_update(value: &Value) -> Result<SessionUpdateConfig> {
    let session = value
        .get("session")
        .ok_or_else(|| anyhow!("session.update missing session"))?;
    let session = session
        .as_object()
        .ok_or_else(|| anyhow!("session must be an object"))?;

    if session
        .get("type")
        .is_some_and(|session_type| session_type.as_str() != Some("transcription"))
    {
        return Err(anyhow!("only transcription sessions are supported"));
    }

    let include = match session.get("include") {
        Some(include) => include
            .as_array()
            .ok_or_else(|| anyhow!("session.include must be an array"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| anyhow!("session.include entries must be strings"))
            })
            .collect::<Result<Vec<_>>>()?,
        None => Vec::new(),
    };

    let turn_detection = value.pointer("/session/audio/input/turn_detection");
    if let Some(turn_detection) = turn_detection {
        if turn_detection != &Value::Null {
            return Err(anyhow!(
                "session.audio.input.turn_detection is not supported"
            ));
        }
    }

    Ok(SessionUpdateConfig {
        input_sample_rate: parse_input_sample_rate(value)?,
        input_audio_format_type: optional_string(
            value,
            "/session/audio/input/format/type",
            "session.audio.input.format.type",
        )?,
        transcription_model: optional_string(
            value,
            "/session/audio/input/transcription/model",
            "session.audio.input.transcription.model",
        )?,
        language: optional_string(
            value,
            "/session/audio/input/transcription/language",
            "session.audio.input.transcription.language",
        )?,
        delay: optional_string(
            value,
            "/session/audio/input/transcription/delay",
            "session.audio.input.transcription.delay",
        )?,
        include,
    })
}

fn optional_string(root: &Value, pointer: &str, label: &str) -> Result<Option<String>> {
    match root.pointer(pointer) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(anyhow!("{label} must be a string")),
        None => Ok(None),
    }
}

fn optional_u32(root: &Value, pointer: &str, label: &str) -> Result<Option<u32>> {
    let Some(value) = root.pointer(pointer) else {
        return Ok(None);
    };
    let Some(value) = value.as_u64().and_then(|value| u32::try_from(value).ok()) else {
        return Err(anyhow!("{label} must be a u32 integer"));
    };
    if value == 0 {
        return Err(anyhow!("{label} must be greater than 0"));
    }
    Ok(Some(value))
}

pub fn model_object_response(model: &str) -> Value {
    json!({
        "id": model,
        "object": MODEL_OBJECT,
        "created": MODEL_CREATED_AT,
        "owned_by": MODEL_OWNER,
    })
}

pub fn model_list_response(model: &str) -> Value {
    json!({
        "object": "list",
        "data": [model_object_response(model)],
    })
}

pub fn model_not_found_error(model: &str) -> Value {
    json!({
        "error": {
            "message": format!("The model `{model}` does not exist or is not supported by this SeedRelay instance."),
            "type": "invalid_request_error",
            "param": "model",
            "code": "model_not_found",
        }
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RealtimeSession {
    pub id: String,
    pub model: String,
    pub input_sample_rate: u32,
    pub input_audio_format_type: String,
    pub language: Option<String>,
}

impl RealtimeSession {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            id: format!("sess_{}", Uuid::new_v4().simple()),
            model: model.into(),
            input_sample_rate: 24_000,
            input_audio_format_type: "audio/pcm".into(),
            language: None,
        }
    }

    pub fn with_id(id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            input_sample_rate: 24_000,
            input_audio_format_type: "audio/pcm".into(),
            language: None,
        }
    }
}

impl SessionUpdateConfig {
    pub fn apply_to(&self, session: &mut RealtimeSession, configured_model: &str) -> Result<bool> {
        if let Some(format_type) = &self.input_audio_format_type {
            if format_type != "audio/pcm" {
                return Err(anyhow!("only audio/pcm input format is supported"));
            }
        }

        if let Some(model) = &self.transcription_model {
            if model != configured_model {
                return Err(anyhow!(
                    "only transcription model `{configured_model}` is supported"
                ));
            }
        }

        if !self.include.is_empty() {
            return Err(anyhow!("session.include is not supported by SeedRelay"));
        }

        if self.delay.is_some() {
            return Err(anyhow!("transcription delay is not supported by SeedRelay"));
        }

        if let Some(format_type) = &self.input_audio_format_type {
            session.input_audio_format_type = format_type.clone();
        }

        if let Some(model) = &self.transcription_model {
            session.model = model.clone();
        } else {
            session.model = configured_model.to_string();
        }

        if let Some(language) = &self.language {
            session.language = Some(language.clone());
        }

        if let Some(rate) = self.input_sample_rate {
            session.input_sample_rate = rate;
            return Ok(true);
        }

        Ok(false)
    }
}

fn event_id() -> String {
    format!("event_{}", Uuid::new_v4().simple())
}

pub fn session_created_event(session: &RealtimeSession) -> Value {
    json!({
        "type": "session.created",
        "event_id": event_id(),
        "session": session_json(session),
    })
}

pub fn session_updated_event(session: &RealtimeSession) -> Value {
    json!({
        "type": "session.updated",
        "event_id": event_id(),
        "session": session_json(session),
    })
}

fn session_json(session: &RealtimeSession) -> Value {
    let transcription = match &session.language {
        Some(language) => json!({
            "model": session.model.as_str(),
            "language": language.as_str(),
        }),
        None => json!({
            "model": session.model.as_str(),
        }),
    };

    json!({
        "id": session.id.as_str(),
        "type": "transcription",
        "model": session.model.as_str(),
        "audio": {
            "input": {
                "format": {
                    "type": session.input_audio_format_type.as_str(),
                    "rate": session.input_sample_rate
                },
                "transcription": transcription
            }
        }
    })
}

pub fn error_event(message: impl Into<String>) -> Value {
    json!({
        "type": "error",
        "event_id": event_id(),
        "error": {
            "type": "invalid_request_error",
            "code": "invalid_request",
            "message": message.into(),
        }
    })
}

pub fn input_audio_committed_event(item_id: &str) -> Value {
    json!({
        "type": "input_audio_buffer.committed",
        "item_id": item_id,
    })
}

pub fn input_audio_cleared_event() -> Value {
    json!({
        "type": "input_audio_buffer.cleared",
        "event_id": event_id(),
    })
}

pub fn transcript_delta_event(item_id: &str, delta: &str) -> Value {
    json!({
        "type": "conversation.item.input_audio_transcription.delta",
        "item_id": item_id,
        "content_index": 0,
        "delta": delta,
    })
}

pub fn transcript_completed_event(item_id: &str, transcript: &str) -> Value {
    json!({
        "type": "conversation.item.input_audio_transcription.completed",
        "item_id": item_id,
        "content_index": 0,
        "transcript": transcript,
    })
}

fn parse_input_sample_rate(value: &Value) -> Result<Option<u32>> {
    let rate = optional_u32(
        value,
        "/session/audio/input/format/rate",
        "session.audio.input.format.rate",
    )?;
    if let Some(rate) = rate {
        if !(MIN_INPUT_SAMPLE_RATE..=MAX_INPUT_SAMPLE_RATE).contains(&rate) {
            return Err(anyhow!(
                "session.audio.input.format.rate must be between {MIN_INPUT_SAMPLE_RATE} and {MAX_INPUT_SAMPLE_RATE}"
            ));
        }
    }
    Ok(rate)
}

fn query_param_from_query(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (entry_key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (decode_url_component(entry_key) == key).then(|| decode_url_component(value))
    })
}

pub(crate) fn decode_url_component(value: &str) -> String {
    let mut output = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    output.push(byte);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(output).unwrap_or_else(|_| value.to_string())
}
