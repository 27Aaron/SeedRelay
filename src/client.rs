use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, Stream, StreamExt};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use uuid::Uuid;

use crate::audio::{silence_frame, AudioConfig, OpusFrameEncoder};
use crate::credentials::{CachedCredentials, AID, USER_AGENT};
use crate::protocol::{
    build_finish_session, build_start_session, build_start_task, build_task_request,
    decode_response, AsrResponse, FrameState,
};
use crate::response::{classify_response, AsrEvent, WireResponse};

pub const WEBSOCKET_URL: &str = "wss://frontier-audio-ime-ws.doubao.com/ocean/api/v1/ws";

pub async fn transcribe_pcm_channel(
    credentials: &CachedCredentials,
    mut pcm_rx: mpsc::Receiver<Vec<u8>>,
    event_tx: mpsc::Sender<AsrEvent>,
) -> Result<String> {
    let config = AudioConfig::default();
    let mut encoder = OpusFrameEncoder::new(config)?;

    let url = format!(
        "{WEBSOCKET_URL}?aid={AID}&device_id={}",
        credentials.device_id
    );
    let mut request = url.into_client_request()?;
    request
        .headers_mut()
        .insert("User-Agent", HeaderValue::from_str(USER_AGENT)?);
    request
        .headers_mut()
        .insert("proto-version", HeaderValue::from_static("v2"));
    request
        .headers_mut()
        .insert("x-custom-keepalive", HeaderValue::from_static("true"));

    let (ws, _) = connect_async(request)
        .await
        .context("failed to open Doubao WebSocket")?;
    let (mut write, mut read) = ws.split();
    let request_id = Uuid::new_v4().to_string();

    write
        .send(Message::Binary(
            build_start_task(&request_id, &credentials.token).into(),
        ))
        .await?;
    if let Err(error) = expect_lifecycle_message(&mut read, "TaskStarted").await {
        let _ = event_tx.send(AsrEvent::Error(error.to_string())).await;
        return Err(error);
    }

    write
        .send(Message::Binary(
            build_start_session(
                &request_id,
                &credentials.token,
                session_payload(&credentials.device_id),
            )
            .into(),
        ))
        .await?;
    if let Err(error) = expect_lifecycle_message(&mut read, "SessionStarted").await {
        let _ = event_tx.send(AsrEvent::Error(error.to_string())).await;
        return Err(error);
    }

    let receiver_request_id = request_id.clone();
    let receiver = tokio::spawn(async move {
        let mut final_text = String::new();
        while let Some(message) = read.next().await {
            let message = message?;
            let data = message_to_bytes(message);
            if data.is_empty() {
                continue;
            }
            let response = decode_response(&data)?;
            if !response.request_id.is_empty() && response.request_id != receiver_request_id {
                continue;
            }
            let event = classify_response(&WireResponse {
                message_type: response.message_type,
                status_message: response.status_message,
                result_json: response.result_json,
            });
            match event {
                AsrEvent::InterimResult(text) if !text.is_empty() => {
                    let _ = event_tx.send(AsrEvent::InterimResult(text)).await;
                }
                AsrEvent::FinalResult(text) if !text.is_empty() => {
                    final_text.push_str(&text);
                    let _ = event_tx.send(AsrEvent::FinalResult(text)).await;
                }
                AsrEvent::SessionFinished => {
                    let _ = event_tx.send(AsrEvent::SessionFinished).await;
                    break;
                }
                AsrEvent::Error(message) => {
                    let _ = event_tx.send(AsrEvent::Error(message.clone())).await;
                    return Err(anyhow!(message));
                }
                _ => {}
            }
        }
        Ok::<String, anyhow::Error>(final_text)
    });

    let start_ms = now_millis();
    let mut frame_index = 0u64;
    let mut pcm_buffer = Vec::new();

    while let Some(chunk) = pcm_rx.recv().await {
        pcm_buffer.extend_from_slice(&chunk);
        while pcm_buffer.len() >= config.bytes_per_frame {
            let frame: Vec<u8> = pcm_buffer.drain(..config.bytes_per_frame).collect();
            let frame_state = if frame_index == 0 {
                FrameState::First
            } else {
                FrameState::Middle
            };
            let opus = encoder.encode(&frame)?;
            let timestamp_ms = start_ms + frame_index * config.frame_duration_ms as u64;
            write
                .send(Message::Binary(
                    build_task_request(&request_id, opus, frame_state, timestamp_ms).into(),
                ))
                .await?;
            frame_index += 1;
        }
    }

    let final_pcm = if pcm_buffer.is_empty() {
        silence_frame(config)
    } else {
        crate::audio::pad_frame(&pcm_buffer, config)
    };
    let last_opus = encoder.encode(&final_pcm)?;
    write
        .send(Message::Binary(
            build_task_request(
                &request_id,
                last_opus,
                FrameState::Last,
                start_ms + frame_index * config.frame_duration_ms as u64,
            )
            .into(),
        ))
        .await?;
    write
        .send(Message::Binary(
            build_finish_session(&request_id, &credentials.token).into(),
        ))
        .await?;

    let final_text = timeout(Duration::from_secs(20), receiver)
        .await
        .context("timed out waiting for SessionFinished")?
        .context("receiver task failed")??;
    Ok(final_text)
}

async fn expect_lifecycle_message<S>(read: &mut S, expected: &str) -> Result<()>
where
    S: Stream<Item = Result<Message, WsError>> + Unpin,
{
    let response = read_next_response(read).await?;
    if response.message_type == expected {
        return Ok(());
    }
    Err(anyhow!(format_lifecycle_error(expected, &response)))
}

pub fn format_lifecycle_error(expected: &str, response: &AsrResponse) -> String {
    let backend_message = if response.status_message.is_empty() {
        "<empty>".to_string()
    } else {
        response.status_message.clone()
    };
    format!(
        "expected {expected}, got {}: status_code={}, service_name={}, request_id={}, message={backend_message}",
        response.message_type, response.status_code, response.service_name, response.request_id
    )
}

async fn read_next_response<S>(read: &mut S) -> Result<AsrResponse>
where
    S: Stream<Item = Result<Message, WsError>> + Unpin,
{
    while let Some(message) = read.next().await {
        let data = message_to_bytes(message?);
        if data.is_empty() {
            continue;
        }
        return Ok(decode_response(&data)?);
    }
    Err(anyhow!("websocket closed before lifecycle response"))
}

fn message_to_bytes(message: Message) -> Vec<u8> {
    match message {
        Message::Binary(data) => data.to_vec(),
        Message::Text(text) => text.to_string().into_bytes(),
        _ => Vec::new(),
    }
}

fn session_payload(device_id: &str) -> String {
    serde_json::json!({
        "audio_info": {
            "channel": 1,
            "format": "speech_opus",
            "sample_rate": 16_000
        },
        "enable_punctuation": true,
        "enable_speech_rejection": false,
        "extra": {
            "app_name": "com.android.chrome",
            "cell_compress_rate": 8,
            "did": device_id,
            "enable_asr_threepass": true,
            "enable_asr_twopass": true,
            "input_mode": "tool"
        }
    })
    .to_string()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis() as u64
}
