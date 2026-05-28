use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

pub const REALTIME_PATH: &str = "/v1/realtime";
pub const MODEL_OBJECT: &str = "model";
pub const MODEL_OWNER: &str = "seedrelay";
pub const MODEL_CREATED_AT: u64 = 0;

#[derive(Debug, Eq, PartialEq)]
pub enum ClientEvent {
    SessionUpdate { input_sample_rate: Option<u32> },
    AppendAudio(Vec<u8>),
    Commit,
    Close,
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

pub(crate) fn query_param(target: &str, key: &str) -> Option<String> {
    let (_, query) = target.split_once('?')?;
    query_param_from_query(query, key)
}

pub fn decode_client_event(raw: &str) -> Result<ClientEvent> {
    let value: Value =
        serde_json::from_str(raw).map_err(|error| anyhow!("invalid JSON: {error}"))?;
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("client event missing type"))?;

    match event_type {
        "session.update" => Ok(ClientEvent::SessionUpdate {
            input_sample_rate: parse_input_sample_rate(&value),
        }),
        "input_audio_buffer.append" | "session.input_audio_buffer.append" => {
            let audio = value
                .get("audio")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("append event missing audio"))?;
            let bytes = STANDARD
                .decode(audio)
                .map_err(|error| anyhow!("invalid base64 audio: {error}"))?;
            Ok(ClientEvent::AppendAudio(bytes))
        }
        "input_audio_buffer.commit" | "session.input_audio_buffer.commit" => {
            Ok(ClientEvent::Commit)
        }
        "session.close" => Ok(ClientEvent::Close),
        other => Err(anyhow!("unsupported client event type: {other}")),
    }
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

pub fn session_updated_event(model: &str) -> Value {
    json!({
        "type": "session.updated",
        "session": {
            "type": "transcription",
            "model": model,
            "audio": {
                "input": {
                    "format": {
                        "type": "audio/pcm",
                        "rate": 24_000
                    },
                    "transcription": {
                        "model": model
                    }
                }
            }
        }
    })
}

pub fn input_audio_committed_event(item_id: &str) -> Value {
    json!({
        "type": "input_audio_buffer.committed",
        "item_id": item_id,
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

pub fn error_event(message: impl Into<String>) -> Value {
    json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": message.into(),
        }
    })
}

fn parse_input_sample_rate(value: &Value) -> Option<u32> {
    value
        .pointer("/session/audio/input/format/rate")
        .and_then(Value::as_u64)
        .and_then(|rate| u32::try_from(rate).ok())
}

fn query_param_from_query(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (entry_key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (decode_query_component(entry_key) == key).then(|| decode_query_component(value))
    })
}

fn decode_query_component(value: &str) -> String {
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
