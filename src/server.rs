use std::net::SocketAddr;
use std::path::Path;
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
use crate::config::ServerConfig;
use crate::credentials::{ensure_credentials, CachedCredentials, USER_AGENT};
use crate::realtime::query_param;
use crate::realtime::{
    decode_client_event, error_event, input_audio_cleared_event, input_audio_committed_event,
    session_created_event, session_updated_event, transcript_completed_event,
    transcript_delta_event, validate_realtime_target, ClientEvent, RealtimeSession,
};
use crate::response::AsrEvent;
use crate::web::{http_response_with_config, WebRuntimeConfig};

#[derive(Debug, Clone)]
struct RuntimeConfig {
    model: String,
    api_key: Option<String>,
}

impl RuntimeConfig {
    fn new(config: ServerConfig) -> Self {
        Self {
            model: config.model,
            api_key: config.api_key,
        }
    }

    fn web_config(&self) -> WebRuntimeConfig {
        WebRuntimeConfig {
            model: self.model.clone(),
            api_key: self.api_key.clone(),
        }
    }

    fn realtime_url(&self, bind: SocketAddr) -> String {
        format!(
            "ws://{bind}/v1/realtime?model={}",
            encode_query_component(&self.model)
        )
    }

    fn startup_lines(&self, bind: SocketAddr, web_enabled: bool) -> Vec<String> {
        let lines = vec![
            "SeedRelay ready".to_string(),
            format!("  Realtime  {}", self.realtime_url(bind)),
            format!("  Model     {}", self.model),
            format!(
                "  Auth      {}",
                if self.api_key.is_some() {
                    "API key required"
                } else {
                    "disabled"
                }
            ),
            format!(
                "  Web UI    {}",
                if web_enabled {
                    format!("http://{bind}/")
                } else {
                    "disabled".to_string()
                }
            ),
        ];
        lines
    }

    fn print_startup(&self, bind: SocketAddr, web_enabled: bool) {
        for line in self.startup_lines(bind, web_enabled) {
            eprintln!("{line}");
        }
    }
}

pub async fn serve_realtime(
    config: ServerConfig,
    credentials_path: &Path,
    reset_credentials: bool,
    web_enabled: bool,
) -> Result<()> {
    let bind = config.bind;
    let runtime_config = Arc::new(RuntimeConfig::new(config));
    let http = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
    let credentials = ensure_credentials(&http, credentials_path, reset_credentials).await?;
    let credentials = Arc::new(credentials);
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;

    runtime_config.print_startup(bind, web_enabled);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let credentials = Arc::clone(&credentials);
        let runtime_config = Arc::clone(&runtime_config);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(
                stream,
                credentials,
                Arc::clone(&runtime_config),
                web_enabled,
            )
            .await
            {
                eprintln!("connection {peer_addr} closed: {error}");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    credentials: Arc<CachedCredentials>,
    runtime_config: Arc<RuntimeConfig>,
    web_enabled: bool,
) -> Result<()> {
    if !is_websocket_upgrade(&stream).await? {
        return serve_http_connection(stream, runtime_config.as_ref(), web_enabled).await;
    }

    let handshake_config = Arc::clone(&runtime_config);
    let callback = move |request: &Request,
                         response: Response|
          -> std::result::Result<Response, ErrorResponse> {
        validate_realtime_request(request, response, handshake_config.as_ref())
    };
    let ws = accept_hdr_async(stream, callback)
        .await
        .context("websocket handshake failed")?;

    handle_realtime_socket(ws, credentials.as_ref().clone(), runtime_config).await
}

#[allow(clippy::result_large_err)]
fn validate_realtime_request(
    request: &Request,
    response: Response,
    runtime_config: &RuntimeConfig,
) -> std::result::Result<Response, ErrorResponse> {
    let target = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("");
    validate_realtime_target(target, &runtime_config.model).map_err(bad_request_response)?;
    let authorization = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    validate_api_key(authorization, target, runtime_config.api_key.as_deref())
        .map_err(unauthorized_response)?;
    Ok(response)
}

async fn is_websocket_upgrade(stream: &TcpStream) -> Result<bool> {
    let mut buffer = [0u8; 1024];
    let read = stream.peek(&mut buffer).await?;
    let header = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
    Ok(header.contains("\r\nupgrade: websocket") || header.contains("\nupgrade: websocket"))
}

async fn serve_http_connection(
    mut stream: TcpStream,
    runtime_config: &RuntimeConfig,
    web_enabled: bool,
) -> Result<()> {
    let request = read_http_header(&mut stream).await?;
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| anyhow!("empty HTTP request"))?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let response = http_response_with_config(
        method,
        target,
        &runtime_config.web_config(),
        web_enabled,
    )
    .unwrap_or_else(|| {
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
    runtime_config: Arc<RuntimeConfig>,
) -> Result<()> {
    let (mut write, mut read) = ws.split();
    let (pcm_tx, pcm_rx) = mpsc::channel::<Vec<u8>>(256);
    let (asr_tx, mut asr_rx) = mpsc::channel::<AsrEvent>(256);
    let mut pcm_tx = Some(pcm_tx);
    let mut session = RealtimeSession::new(runtime_config.model.clone());
    let mut converter = Pcm16Converter::new(session.input_sample_rate);
    let item_id = format!("item_{}", Uuid::new_v4().simple());
    let mut interim_transcript = String::new();
    let mut final_transcript = String::new();

    let asr_task =
        tokio::spawn(async move { transcribe_pcm_channel(&credentials, pcm_rx, asr_tx).await });
    tokio::pin!(asr_task);

    send_json(&mut write, session_created_event(&session)).await?;

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
                            runtime_config.as_ref(),
                            &mut session,
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
    runtime_config: &RuntimeConfig,
    session: &mut RealtimeSession,
    converter: &mut Pcm16Converter,
    pcm_tx: &mut Option<mpsc::Sender<Vec<u8>>>,
) -> Result<()> {
    match decode_client_event(raw)? {
        ClientEvent::SessionUpdate(update) => {
            let rate_changed = update.apply_to(session, &runtime_config.model)?;
            if rate_changed {
                *converter = Pcm16Converter::new(session.input_sample_rate);
            }
            send_json(write, session_updated_event(session)).await?;
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
        ClientEvent::Clear => {
            reset_audio_buffer_converter(converter, session);
            send_json(write, input_audio_cleared_event()).await?;
        }
        ClientEvent::Commit => {
            drop(pcm_tx.take());
            send_json(write, input_audio_committed_event(item_id)).await?;
        }
        ClientEvent::Close => {
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

fn reset_audio_buffer_converter(converter: &mut Pcm16Converter, session: &RealtimeSession) {
    *converter = Pcm16Converter::new(session.input_sample_rate);
}

fn bad_request_response(error: anyhow::Error) -> ErrorResponse {
    error_response(StatusCode::BAD_REQUEST, error)
}

fn unauthorized_response(error: anyhow::Error) -> ErrorResponse {
    error_response(StatusCode::UNAUTHORIZED, error)
}

fn error_response(status: StatusCode, error: anyhow::Error) -> ErrorResponse {
    Response::builder()
        .status(status)
        .body(Some(error.to_string()))
        .expect("valid error response")
}

fn validate_api_key(
    authorization: Option<&str>,
    target: &str,
    expected_api_key: Option<&str>,
) -> Result<()> {
    let Some(expected_api_key) = expected_api_key else {
        return Ok(());
    };

    let bearer_matches = authorization
        .and_then(|value| value.trim().split_once(' '))
        .map(|(scheme, token)| {
            scheme.eq_ignore_ascii_case("bearer") && token.trim() == expected_api_key
        })
        .unwrap_or(false);
    let query_matches = query_param(target, "api_key").as_deref() == Some(expected_api_key);

    if bearer_matches || query_matches {
        Ok(())
    } else {
        Err(anyhow!("missing or invalid API key"))
    }
}

#[cfg(test)]
fn redact_api_key(target: &str) -> String {
    let Some((path, query)) = target.split_once('?') else {
        return target.to_string();
    };
    let query = query
        .split('&')
        .map(|pair| {
            let (key, _) = pair.split_once('=').unwrap_or((pair, ""));
            if key == "api_key" {
                "api_key=***".to_string()
            } else {
                pair.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
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
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::config::ServerConfig;
    use crate::realtime::RealtimeSession;

    use super::{
        append_final_transcript, encode_query_component, interim_delta, redact_api_key,
        reset_audio_buffer_converter, validate_api_key, Pcm16Converter, RuntimeConfig,
    };

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

    #[test]
    fn reset_audio_buffer_converter_drops_partial_resampler_state() {
        let mut session = RealtimeSession::with_id("sess_test", "seed-asr");
        session.input_sample_rate = 24_000;
        let mut converter = Pcm16Converter::new(session.input_sample_rate);

        assert!(converter.push(&1i16.to_le_bytes()).is_empty());
        reset_audio_buffer_converter(&mut converter, &session);
        assert!(converter.push(&2i16.to_le_bytes()).is_empty());
    }

    #[test]
    fn api_key_is_optional_when_not_configured() {
        validate_api_key(None, "/v1/realtime?model=seed-asr", None).expect("no api key");
    }

    #[test]
    fn api_key_accepts_bearer_authorization_or_query_param() {
        validate_api_key(
            Some("Bearer local-secret"),
            "/v1/realtime?model=seed-asr",
            Some("local-secret"),
        )
        .expect("bearer key");
        validate_api_key(
            None,
            "/v1/realtime?model=seed-asr&api_key=local-secret",
            Some("local-secret"),
        )
        .expect("query key");
    }

    #[test]
    fn api_key_rejects_missing_or_invalid_key_when_configured() {
        assert!(
            validate_api_key(None, "/v1/realtime?model=seed-asr", Some("local-secret")).is_err()
        );
        assert!(validate_api_key(
            Some("Bearer wrong"),
            "/v1/realtime?model=seed-asr",
            Some("local-secret"),
        )
        .is_err());
    }

    #[test]
    fn startup_lines_are_readable_and_hide_key() {
        let runtime = RuntimeConfig::new(ServerConfig {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
            model: "custom/asr".to_string(),
            api_key: Some("local-secret".to_string()),
        });
        let lines =
            runtime.startup_lines(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000), true);

        assert_eq!(lines[0], "SeedRelay ready");
        assert!(lines.contains(
            &"  Realtime  ws://127.0.0.1:8000/v1/realtime?model=custom%2Fasr".to_string()
        ));
        assert!(lines.contains(&"  Auth      API key required".to_string()));
        assert!(lines.contains(&"  Web UI    http://127.0.0.1:8000/".to_string()));
        assert!(!lines.join("\n").contains("local-secret"));
    }

    #[test]
    fn startup_lines_show_disabled_web_and_auth_by_default() {
        let runtime = RuntimeConfig::new(ServerConfig {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
            model: "seed-asr".to_string(),
            api_key: None,
        });
        let lines = runtime.startup_lines(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
            false,
        );

        assert!(lines
            .contains(&"  Realtime  ws://127.0.0.1:8000/v1/realtime?model=seed-asr".to_string()));
        assert!(lines.contains(&"  Auth      disabled".to_string()));
        assert!(lines.contains(&"  Web UI    disabled".to_string()));
        assert!(!lines.iter().any(|line| line.contains("Debug")));
    }

    #[test]
    fn redacts_api_key_from_logged_targets() {
        assert_eq!(
            redact_api_key("/v1/realtime?model=seed-asr&api_key=local-secret"),
            "/v1/realtime?model=seed-asr&api_key=***"
        );
        assert_eq!(
            redact_api_key("/v1/realtime?api_key=local-secret&model=seed-asr"),
            "/v1/realtime?api_key=***&model=seed-asr"
        );
    }

    #[test]
    fn encodes_model_names_for_log_urls() {
        assert_eq!(encode_query_component("seed-asr"), "seed-asr");
        assert_eq!(encode_query_component("custom/asr 1"), "custom%2Fasr%201");
    }
}
