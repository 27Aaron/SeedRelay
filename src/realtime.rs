use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

pub const SUPPORTED_MODEL: &str = "seed-asr-2.0";
pub const REALTIME_PATH: &str = "/v1/realtime";

#[derive(Debug, Eq, PartialEq)]
pub enum ClientEvent {
    SessionUpdate { input_sample_rate: Option<u32> },
    AppendAudio(Vec<u8>),
    Commit,
    Close,
}

pub fn validate_realtime_target(target: &str) -> Result<()> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != REALTIME_PATH {
        return Err(anyhow!("expected {REALTIME_PATH}, got {path}"));
    }

    let has_supported_model = query.split('&').any(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        key == "model" && value == SUPPORTED_MODEL
    });
    if !has_supported_model {
        return Err(anyhow!("only model={SUPPORTED_MODEL} is supported"));
    }

    Ok(())
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

pub fn session_updated_event() -> Value {
    json!({
        "type": "session.updated",
        "session": {
            "type": "transcription",
            "model": SUPPORTED_MODEL,
            "audio": {
                "input": {
                    "format": {
                        "type": "audio/pcm",
                        "rate": 24_000
                    },
                    "transcription": {
                        "model": SUPPORTED_MODEL
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
