use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use uuid::Uuid;

use crate::audio::LinearPcmResampler;
use crate::client::transcribe_pcm_channel;
use crate::credentials::{ensure_credentials, CachedCredentials, USER_AGENT};
use crate::realtime::{
    decode_client_event, error_event, input_audio_committed_event, session_updated_event,
    transcript_completed_event, transcript_delta_event, validate_realtime_target, ClientEvent,
    SUPPORTED_MODEL,
};
use crate::response::AsrEvent;
use crate::web::http_response;

pub async fn serve_realtime(
    bind: SocketAddr,
    env_path: PathBuf,
    reset_credentials: bool,
    web_enabled: bool,
) -> Result<()> {
    let http = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
    let credentials = ensure_credentials(&http, &env_path, reset_credentials).await?;
    let credentials = Arc::new(credentials);
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;

    eprintln!("listening on ws://{bind}/v1/realtime?model={SUPPORTED_MODEL}");
    if web_enabled {
        eprintln!("test page available at http://{bind}/");
    }

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let credentials = Arc::clone(&credentials);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, credentials, web_enabled).await {
                eprintln!("realtime connection {peer_addr} closed: {error:#}");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    credentials: Arc<CachedCredentials>,
    web_enabled: bool,
) -> Result<()> {
    if web_enabled && !is_websocket_upgrade(&stream).await? {
        return serve_http_connection(stream).await;
    }

    let ws = accept_hdr_async(stream, validate_realtime_request)
        .await
        .context("websocket handshake failed")?;

    handle_realtime_socket(ws, credentials.as_ref().clone()).await
}

#[allow(clippy::result_large_err)]
fn validate_realtime_request(
    request: &Request,
    response: Response,
) -> std::result::Result<Response, ErrorResponse> {
    let target = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("");
    validate_realtime_target(target).map_err(bad_request_response)?;
    Ok(response)
}

async fn is_websocket_upgrade(stream: &TcpStream) -> Result<bool> {
    let mut buffer = [0u8; 1024];
    let read = stream.peek(&mut buffer).await?;
    let header = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
    Ok(header.contains("\r\nupgrade: websocket") || header.contains("\nupgrade: websocket"))
}

async fn serve_http_connection(mut stream: TcpStream) -> Result<()> {
    let request = read_http_header(&mut stream).await?;
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| anyhow!("empty HTTP request"))?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let response = http_response(method, target).unwrap_or_else(|| {
        "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            .to_string()
    });
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn read_http_header(stream: &mut TcpStream) -> Result<String> {
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];

    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > 8192 {
            return Err(anyhow!("HTTP header too large"));
        }
    }

    String::from_utf8(request).context("HTTP request is not UTF-8")
}

async fn handle_realtime_socket(
    ws: WebSocketStream<TcpStream>,
    credentials: CachedCredentials,
) -> Result<()> {
    let (mut write, mut read) = ws.split();
    let (pcm_tx, pcm_rx) = mpsc::channel::<Vec<u8>>(256);
    let (asr_tx, mut asr_rx) = mpsc::channel::<AsrEvent>(256);
    let mut pcm_tx = Some(pcm_tx);
    let mut converter = Pcm16Converter::new(24_000);
    let item_id = format!("item_{}", Uuid::new_v4().simple());
    let mut interim_transcript = String::new();
    let mut final_transcript = String::new();

    let asr_task =
        tokio::spawn(async move { transcribe_pcm_channel(&credentials, pcm_rx, asr_tx).await });
    tokio::pin!(asr_task);

    send_json(&mut write, session_updated_event()).await?;

    loop {
        tokio::select! {
            maybe_message = read.next() => {
                let Some(message) = maybe_message else {
                    drop(pcm_tx.take());
                    break;
                };
                match decode_socket_message(message?) {
                    Ok(raw) => {
                        if let Err(error) = handle_client_event(
                            &mut write,
                            &raw,
                            &item_id,
                            &mut converter,
                            &mut pcm_tx,
                        ).await {
                            send_json(&mut write, error_event(error.to_string())).await?;
                        }
                    }
                    Err(error) => {
                        send_json(&mut write, error_event(error.to_string())).await?;
                    }
                }
            }
            maybe_event = asr_rx.recv() => {
                let Some(event) = maybe_event else {
                    continue;
                };
                match event {
                    AsrEvent::InterimResult(text) if !text.is_empty() => {
                        let delta = interim_delta(&mut interim_transcript, &text);
                        if !delta.is_empty() {
                            send_json(&mut write, transcript_delta_event(&item_id, &delta)).await?;
                        }
                    }
                    AsrEvent::FinalResult(text) if !text.is_empty() => {
                        if let Some(transcript) =
                            append_final_transcript(&mut final_transcript, &text)
                        {
                            send_json(&mut write, transcript_completed_event(&item_id, &transcript)).await?;
                        }
                    }
                    AsrEvent::SessionFinished => {
                        send_json(&mut write, transcript_completed_event(&item_id, &final_transcript)).await?;
                        break;
                    }
                    AsrEvent::Error(message) => {
                        send_json(&mut write, error_event(message)).await?;
                        break;
                    }
                    _ => {}
                }
            }
            result = &mut asr_task => {
                match result.context("ASR task join failed")? {
                    Ok(transcript) => {
                        if final_transcript.is_empty() {
                            final_transcript = transcript;
                        }
                        send_json(&mut write, transcript_completed_event(&item_id, &final_transcript)).await?;
                    }
                    Err(error) => {
                        send_json(&mut write, error_event(error.to_string())).await?;
                    }
                }
                break;
            }
        }
    }

    let _ = write.close().await;
    Ok(())
}

async fn handle_client_event(
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    raw: &str,
    item_id: &str,
    converter: &mut Pcm16Converter,
    pcm_tx: &mut Option<mpsc::Sender<Vec<u8>>>,
) -> Result<()> {
    match decode_client_event(raw)? {
        ClientEvent::SessionUpdate { input_sample_rate } => {
            if let Some(rate) = input_sample_rate {
                *converter = Pcm16Converter::new(rate);
            }
            send_json(write, session_updated_event()).await?;
        }
        ClientEvent::AppendAudio(audio) => {
            let Some(tx) = pcm_tx else {
                return Err(anyhow!("audio was appended after commit"));
            };
            let pcm = converter.push(&audio);
            if !pcm.is_empty() {
                tx.send(pcm).await.context("ASR audio channel closed")?;
            }
        }
        ClientEvent::Commit | ClientEvent::Close => {
            drop(pcm_tx.take());
            send_json(write, input_audio_committed_event(item_id)).await?;
        }
    }

    Ok(())
}

async fn send_json(
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    value: Value,
) -> Result<()> {
    write.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

fn decode_socket_message(message: Message) -> Result<String> {
    match message {
        Message::Text(text) => Ok(text.to_string()),
        Message::Binary(bytes) => {
            String::from_utf8(bytes.to_vec()).context("binary message is not UTF-8")
        }
        Message::Close(_) => Ok(r#"{"type":"session.close"}"#.to_string()),
        _ => Err(anyhow!("unsupported websocket message")),
    }
}

fn interim_delta(previous: &mut String, current: &str) -> String {
    let delta = current
        .strip_prefix(previous.as_str())
        .unwrap_or(current)
        .to_string();
    previous.clear();
    previous.push_str(current);
    delta
}

fn append_final_transcript(transcript: &mut String, text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    transcript.push_str(text);
    Some(transcript.clone())
}

fn bad_request_response(error: anyhow::Error) -> ErrorResponse {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Some(error.to_string()))
        .expect("valid bad request response")
}

struct Pcm16Converter {
    input_sample_rate: u32,
    resampler: LinearPcmResampler,
}

impl Pcm16Converter {
    fn new(input_sample_rate: u32) -> Self {
        Self {
            input_sample_rate,
            resampler: LinearPcmResampler::new(input_sample_rate, 16_000),
        }
    }

    fn push(&mut self, pcm16: &[u8]) -> Vec<u8> {
        if self.input_sample_rate == 16_000 {
            return even_pcm_bytes(pcm16);
        }

        let samples = pcm16
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect::<Vec<_>>();
        self.resampler.push_mono_f32(&samples)
    }
}

fn even_pcm_bytes(pcm16: &[u8]) -> Vec<u8> {
    let even_len = pcm16.len() - (pcm16.len() % 2);
    pcm16[..even_len].to_vec()
}

#[cfg(test)]
mod tests {
    use super::{append_final_transcript, interim_delta};

    #[test]
    fn interim_delta_sends_only_new_suffix_for_growing_snapshots() {
        let mut previous = String::new();

        assert_eq!(interim_delta(&mut previous, "你好"), "你好");
        assert_eq!(interim_delta(&mut previous, "你好啊"), "啊");
        assert_eq!(interim_delta(&mut previous, "你好啊，你是谁"), "，你是谁");
        assert_eq!(previous, "你好啊，你是谁");
    }

    #[test]
    fn interim_delta_resends_current_text_when_snapshot_is_rewritten() {
        let mut previous = "你好啊".to_string();

        assert_eq!(interim_delta(&mut previous, "您好"), "您好");
        assert_eq!(previous, "您好");
    }

    #[test]
    fn append_final_transcript_returns_live_snapshot() {
        let mut transcript = String::new();

        assert_eq!(
            append_final_transcript(&mut transcript, "你好啊，"),
            Some("你好啊，".to_string())
        );
        assert_eq!(
            append_final_transcript(&mut transcript, "hello。"),
            Some("你好啊，hello。".to_string())
        );
        assert_eq!(transcript, "你好啊，hello。");
    }
}
