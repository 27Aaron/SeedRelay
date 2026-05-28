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
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use uuid::Uuid;

use crate::audio::LinearPcmResampler;
use crate::client::transcribe_pcm_channel;
use crate::config::ServerConfig;
use crate::credentials::{ensure_credentials, CachedCredentials, USER_AGENT};
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

#[derive(Debug, Default)]
struct TurnTranscriptState {
    interim_transcript: String,
    final_transcript: String,
    completed: bool,
}

impl TurnTranscriptState {
    fn interim_delta(&mut self, current: &str) -> Option<String> {
        let delta = current
            .strip_prefix(self.interim_transcript.as_str())
            .unwrap_or(current)
            .to_string();
        self.interim_transcript.clear();
        self.interim_transcript.push_str(current);

        (!delta.is_empty()).then_some(delta)
    }

    fn append_final(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.starts_with(&self.final_transcript) {
            self.final_transcript.clear();
            self.final_transcript.push_str(text);
            return;
        }
        self.final_transcript.push_str(text);
    }

    fn completed_transcript(&mut self, fallback: Option<String>) -> Option<&str> {
        if self.completed {
            return None;
        }
        if self.final_transcript.is_empty() {
            if let Some(fallback) = fallback.filter(|transcript| !transcript.is_empty()) {
                self.final_transcript = fallback;
            }
        }
        self.completed = true;
        Some(self.final_transcript.as_str())
    }
}

#[derive(Debug)]
struct ActiveTurn {
    item_id: String,
    input: TurnInput,
    transcript: TurnTranscriptState,
}

#[derive(Debug)]
enum TurnInput {
    Idle,
    Open(mpsc::Sender<Vec<u8>>),
    Closed,
}

impl ActiveTurn {
    fn new_idle() -> Self {
        Self {
            item_id: format!("item_{}", Uuid::new_v4().simple()),
            input: TurnInput::Idle,
            transcript: TurnTranscriptState::default(),
        }
    }

    fn ensure_started(
        &mut self,
        credentials: CachedCredentials,
        outputs: mpsc::Sender<TurnOutput>,
    ) -> Result<&mpsc::Sender<Vec<u8>>> {
        self.ensure_started_with(|item_id| spawn_turn_for_item(item_id, credentials, outputs))
    }

    fn ensure_started_with<F>(&mut self, start: F) -> Result<&mpsc::Sender<Vec<u8>>>
    where
        F: FnOnce(String) -> mpsc::Sender<Vec<u8>>,
    {
        match self.input {
            TurnInput::Idle => {
                self.input = TurnInput::Open(start(self.item_id.clone()));
            }
            TurnInput::Open(_) => {}
            TurnInput::Closed => return Err(anyhow!("audio was appended after commit")),
        }

        match &self.input {
            TurnInput::Open(pcm_tx) => Ok(pcm_tx),
            TurnInput::Idle | TurnInput::Closed => unreachable!("turn input was just opened"),
        }
    }

    fn close_input(&mut self) {
        self.input = TurnInput::Closed;
    }

    fn replace_with_idle(&mut self) {
        self.close_input();
        *self = Self::new_idle();
    }
}

#[derive(Debug)]
enum TurnOutput {
    AsrEvent {
        item_id: String,
        event: AsrEvent,
    },
    Finished {
        item_id: String,
        result: Result<String, String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientAction {
    Continue,
    CloseSocket,
}

enum SocketMessage {
    ClientEvent(String),
    Close,
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
    let protocols = request
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok());
    validate_api_key(authorization, protocols, runtime_config.api_key.as_deref())
        .map_err(unauthorized_response)?;
    if let Some(protocol) =
        response_realtime_subprotocol(protocols, runtime_config.api_key.as_deref())
    {
        let mut response = response;
        response.headers_mut().insert(
            "sec-websocket-protocol",
            HeaderValue::from_str(protocol).map_err(|error| {
                error_response(
                    StatusCode::BAD_REQUEST,
                    anyhow!("invalid websocket subprotocol: {error}"),
                )
            })?,
        );
        return Ok(response);
    }
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

fn spawn_turn_for_item(
    item_id: String,
    credentials: CachedCredentials,
    outputs: mpsc::Sender<TurnOutput>,
) -> mpsc::Sender<Vec<u8>> {
    let (pcm_tx, pcm_rx) = mpsc::channel::<Vec<u8>>(256);
    let (asr_tx, mut asr_rx) = mpsc::channel::<AsrEvent>(256);

    tokio::spawn(async move {
        let asr_item_id = item_id.clone();
        let mut asr_task =
            tokio::spawn(async move { transcribe_pcm_channel(&credentials, pcm_rx, asr_tx).await });
        let mut sent_finished = false;

        loop {
            tokio::select! {
                maybe_event = asr_rx.recv() => {
                    let Some(event) = maybe_event else {
                        break;
                    };
                    if outputs
                        .send(TurnOutput::AsrEvent {
                            item_id: item_id.clone(),
                            event,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                result = &mut asr_task => {
                    send_turn_finished(&outputs, &asr_item_id, result).await;
                    sent_finished = true;
                    break;
                }
            }
        }

        if !sent_finished {
            let result = asr_task.await;
            send_turn_finished(&outputs, &asr_item_id, result).await;
        }
    });

    pcm_tx
}

async fn send_turn_finished(
    outputs: &mpsc::Sender<TurnOutput>,
    item_id: &str,
    result: std::result::Result<Result<String>, tokio::task::JoinError>,
) {
    let result = match result {
        Ok(Ok(transcript)) => Ok(transcript),
        Ok(Err(error)) => Err(error.to_string()),
        Err(error) => Err(format!("ASR task join failed: {error}")),
    };
    let _ = outputs
        .send(TurnOutput::Finished {
            item_id: item_id.to_string(),
            result,
        })
        .await;
}

async fn handle_realtime_socket(
    ws: WebSocketStream<TcpStream>,
    credentials: CachedCredentials,
    runtime_config: Arc<RuntimeConfig>,
) -> Result<()> {
    let (mut write, mut read) = ws.split();
    let (turn_output_tx, mut turn_output_rx) = mpsc::channel::<TurnOutput>(256);
    let mut active_turn = ActiveTurn::new_idle();
    let mut session = RealtimeSession::new(runtime_config.model.clone());
    let mut converter = Pcm16Converter::new(session.input_sample_rate);

    send_json(&mut write, session_created_event(&session)).await?;

    loop {
        tokio::select! {
            maybe_message = read.next() => {
                let Some(message) = maybe_message else {
                    active_turn.close_input();
                    break;
                };
                match decode_socket_message(message?) {
                    Ok(SocketMessage::ClientEvent(raw)) => {
                        match handle_client_event(
                            &mut write,
                            &raw,
                            &credentials,
                            &turn_output_tx,
                            runtime_config.as_ref(),
                            &mut session,
                            &mut converter,
                            &mut active_turn,
                        ).await {
                            Ok(ClientAction::Continue) => {}
                            Ok(ClientAction::CloseSocket) => {
                                break;
                            }
                            Err(error) => {
                                send_json(&mut write, error_event(error.to_string())).await?;
                            }
                        }
                    }
                    Ok(SocketMessage::Close) => {
                        active_turn.close_input();
                        break;
                    }
                    Err(error) => {
                        send_json(&mut write, error_event(error.to_string())).await?;
                    }
                }
            }
            maybe_output = turn_output_rx.recv() => {
                let Some(output) = maybe_output else {
                    break;
                };
                match output {
                    TurnOutput::AsrEvent { item_id, event } => {
                        if item_id != active_turn.item_id {
                            continue;
                        }
                        match event {
                            AsrEvent::InterimResult(text) if !text.is_empty() => {
                                if let Some(delta) = active_turn.transcript.interim_delta(&text) {
                                    send_json(&mut write, transcript_delta_event(&item_id, &delta)).await?;
                                }
                            }
                            AsrEvent::FinalResult(text) if !text.is_empty() => {
                                active_turn.transcript.append_final(&text);
                            }
                            AsrEvent::SessionFinished => {
                                if let Some(transcript) = active_turn.transcript.completed_transcript(None) {
                                    send_json(
                                        &mut write,
                                        transcript_completed_event(&item_id, transcript),
                                    )
                                    .await?;
                                }
                                active_turn = ActiveTurn::new_idle();
                            }
                            AsrEvent::Error(message) => {
                                send_json(&mut write, error_event(message)).await?;
                                active_turn = ActiveTurn::new_idle();
                            }
                            _ => {}
                        }
                    }
                    TurnOutput::Finished { item_id, result } => {
                        if item_id != active_turn.item_id {
                            continue;
                        }
                        match result {
                            Ok(transcript) => {
                                if let Some(transcript) =
                                    active_turn.transcript.completed_transcript(Some(transcript))
                                {
                                    send_json(
                                        &mut write,
                                        transcript_completed_event(&item_id, transcript),
                                    )
                                    .await?;
                                }
                                active_turn = ActiveTurn::new_idle();
                            }
                            Err(message) => {
                                send_json(&mut write, error_event(message)).await?;
                                active_turn = ActiveTurn::new_idle();
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = write.close().await;
    Ok(())
}

async fn handle_client_event(
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    raw: &str,
    credentials: &CachedCredentials,
    turn_output_tx: &mpsc::Sender<TurnOutput>,
    runtime_config: &RuntimeConfig,
    session: &mut RealtimeSession,
    converter: &mut Pcm16Converter,
    active_turn: &mut ActiveTurn,
) -> Result<ClientAction> {
    let event = decode_client_event(raw)?;
    let action = client_action_for_event(&event);

    match event {
        ClientEvent::SessionUpdate(update) => {
            validate_session_update_timing(active_turn, session, &update)?;
            let rate_changed = update.apply_to(session, &runtime_config.model)?;
            if rate_changed {
                *converter = Pcm16Converter::new(session.input_sample_rate);
            }
            send_json(write, session_updated_event(session)).await?;
        }
        ClientEvent::AppendAudio(audio) => {
            let tx = active_turn.ensure_started(credentials.clone(), turn_output_tx.clone())?;
            let pcm = converter.push(&audio);
            if !pcm.is_empty() {
                tx.send(pcm).await.context("ASR audio channel closed")?;
            }
        }
        ClientEvent::Clear => {
            reset_audio_buffer_converter(converter, session);
            active_turn.replace_with_idle();
            send_json(write, input_audio_cleared_event()).await?;
        }
        ClientEvent::Commit => {
            let tx = active_turn.ensure_started(credentials.clone(), turn_output_tx.clone())?;
            let pcm = converter.finish();
            if !pcm.is_empty() {
                tx.send(pcm).await.context("ASR audio channel closed")?;
            }
            active_turn.close_input();
            send_json(write, input_audio_committed_event(&active_turn.item_id)).await?;
        }
        ClientEvent::Close => {
            active_turn.close_input();
        }
    }

    Ok(action)
}

fn client_action_for_event(event: &ClientEvent) -> ClientAction {
    match event {
        ClientEvent::Close => ClientAction::CloseSocket,
        _ => ClientAction::Continue,
    }
}

async fn send_json(
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    value: Value,
) -> Result<()> {
    write.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

fn decode_socket_message(message: Message) -> Result<SocketMessage> {
    match message {
        Message::Text(text) => Ok(SocketMessage::ClientEvent(text.to_string())),
        Message::Binary(bytes) => Ok(SocketMessage::ClientEvent(
            String::from_utf8(bytes.to_vec()).context("binary message is not UTF-8")?,
        )),
        Message::Close(_) => Ok(SocketMessage::Close),
        _ => Err(anyhow!("unsupported websocket message")),
    }
}

fn reset_audio_buffer_converter(converter: &mut Pcm16Converter, session: &RealtimeSession) {
    *converter = Pcm16Converter::new(session.input_sample_rate);
}

fn validate_session_update_timing(
    active_turn: &ActiveTurn,
    session: &RealtimeSession,
    update: &crate::realtime::SessionUpdateConfig,
) -> Result<()> {
    let rate_changes = update
        .input_sample_rate
        .is_some_and(|rate| rate != session.input_sample_rate);
    let format_changes = update
        .input_audio_format_type
        .as_ref()
        .is_some_and(|format| format != &session.input_audio_format_type);

    if (rate_changes || format_changes) && !matches!(active_turn.input, TurnInput::Idle) {
        return Err(anyhow!(
            "cannot change audio input format while a turn is active"
        ));
    }

    Ok(())
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
    protocols: Option<&str>,
    expected_api_key: Option<&str>,
) -> Result<()> {
    let Some(expected_api_key) = expected_api_key else {
        return Ok(());
    };

    let bearer_matches = authorization
        .and_then(|value| value.trim().split_once(' '))
        .filter(|(scheme, token)| {
            scheme.eq_ignore_ascii_case("bearer") && token.trim() == expected_api_key
        })
        .is_some();
    let protocol_matches = matching_realtime_subprotocol(protocols, expected_api_key).is_some();

    if bearer_matches || protocol_matches {
        return Ok(());
    }

    Err(anyhow!("missing or invalid API key"))
}

#[cfg(test)]
fn selected_realtime_subprotocol(protocols: Option<&str>) -> Option<&str> {
    protocols?
        .split(',')
        .map(str::trim)
        .find(|protocol| protocol.starts_with("openai-insecure-api-key."))
}

fn matching_realtime_subprotocol<'a>(
    protocols: Option<&'a str>,
    expected_api_key: &str,
) -> Option<&'a str> {
    protocols?.split(',').map(str::trim).find(|protocol| {
        protocol
            .strip_prefix("openai-insecure-api-key.")
            .is_some_and(|key| !key.is_empty() && key == expected_api_key)
    })
}

fn matching_realtime_protocol(protocols: Option<&str>) -> Option<&str> {
    protocols?
        .split(',')
        .map(str::trim)
        .find(|protocol| protocol.eq_ignore_ascii_case("realtime"))
}

fn response_realtime_subprotocol<'a>(
    protocols: Option<&'a str>,
    expected_api_key: Option<&str>,
) -> Option<&'a str> {
    matching_realtime_protocol(protocols).or_else(|| {
        expected_api_key.and_then(|api_key| matching_realtime_subprotocol(protocols, api_key))
    })
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

    fn finish(&mut self) -> Vec<u8> {
        if self.input_sample_rate == 16_000 {
            return Vec::new();
        }

        self.resampler.finish()
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
    use crate::realtime::{decode_client_event, RealtimeSession, SessionUpdateConfig};
    use tokio::sync::mpsc;
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::http::StatusCode;
    use tokio_tungstenite::tungstenite::Message;

    use super::{
        client_action_for_event, decode_socket_message, encode_query_component,
        reset_audio_buffer_converter, selected_realtime_subprotocol, validate_api_key,
        validate_realtime_request, validate_session_update_timing, ActiveTurn, ClientAction,
        Pcm16Converter, RuntimeConfig, SocketMessage, TurnInput, TurnTranscriptState,
    };

    #[test]
    fn turn_transcript_state_emits_incremental_delta() {
        let mut state = TurnTranscriptState::default();

        assert_eq!(state.interim_delta("你好"), Some("你好".to_string()));
        assert_eq!(state.interim_delta("你好世界"), Some("世界".to_string()));
        assert_eq!(state.interim_delta("你好世界"), None);
    }

    #[test]
    fn turn_transcript_state_accumulates_final_text() {
        let mut state = TurnTranscriptState::default();

        state.append_final("你好");
        state.append_final("，世界");
        state.append_final("");

        assert_eq!(state.completed_transcript(None), Some("你好，世界"));
        assert_eq!(state.completed_transcript(None), None);
    }

    #[test]
    fn turn_transcript_state_deduplicates_cumulative_final_text() {
        let mut state = TurnTranscriptState::default();

        state.append_final("hello");
        state.append_final("hello world");

        assert_eq!(state.completed_transcript(None), Some("hello world"));
    }

    #[test]
    fn turn_transcript_state_uses_finished_transcript_when_no_final_text_exists() {
        let mut state = TurnTranscriptState::default();

        assert_eq!(
            state.completed_transcript(Some("backend final".to_string())),
            Some("backend final")
        );
        assert_eq!(
            state.completed_transcript(Some("duplicate".to_string())),
            None
        );
    }

    #[test]
    fn active_turn_generates_distinct_item_ids() {
        let first = ActiveTurn::new_idle();
        let second = ActiveTurn::new_idle();

        assert!(first.item_id.starts_with("item_"));
        assert!(second.item_id.starts_with("item_"));
        assert_ne!(first.item_id, second.item_id);
    }

    #[test]
    fn active_turn_starts_idle_with_item_id() {
        let turn = ActiveTurn::new_idle();

        assert!(turn.item_id.starts_with("item_"));
        assert!(matches!(turn.input, TurnInput::Idle));
    }

    #[test]
    fn active_turn_closing_idle_marks_closed_and_prevents_start() {
        let mut turn = ActiveTurn::new_idle();

        turn.close_input();

        assert!(matches!(turn.input, TurnInput::Closed));
        let (pcm_tx, _pcm_rx) = mpsc::channel::<Vec<u8>>(1);
        assert!(turn.ensure_started_with(|_| pcm_tx).is_err());
    }

    #[test]
    fn active_turn_first_start_opens_once_without_changing_item_id() {
        let mut turn = ActiveTurn::new_idle();
        let item_id = turn.item_id.clone();
        let mut starts = 0;

        turn.ensure_started_with(|_| {
            starts += 1;
            let (pcm_tx, _pcm_rx) = mpsc::channel::<Vec<u8>>(1);
            pcm_tx
        })
        .expect("first start");
        turn.ensure_started_with(|_| {
            starts += 1;
            let (pcm_tx, _pcm_rx) = mpsc::channel::<Vec<u8>>(1);
            pcm_tx
        })
        .expect("second start reuses open input");

        assert_eq!(starts, 1);
        assert_eq!(turn.item_id, item_id);
        assert!(matches!(turn.input, TurnInput::Open(_)));
    }

    #[test]
    fn active_turn_clear_replaces_open_turn_with_fresh_idle_item() {
        let mut turn = ActiveTurn::new_idle();
        let old_item_id = turn.item_id.clone();
        let (pcm_tx, pcm_rx) = mpsc::channel::<Vec<u8>>(1);

        turn.ensure_started_with(|_| pcm_tx).expect("open input");
        turn.replace_with_idle();

        assert_ne!(turn.item_id, old_item_id);
        assert!(matches!(turn.input, TurnInput::Idle));
        assert!(pcm_rx.is_closed());
    }

    #[test]
    fn client_action_closes_socket_for_session_close() {
        let event = decode_client_event(r#"{"type":"session.close"}"#).expect("close event");

        assert_eq!(client_action_for_event(&event), ClientAction::CloseSocket);
    }

    #[test]
    fn websocket_close_decodes_to_socket_close_control() {
        let message = decode_socket_message(Message::Close(None)).expect("close frame");

        assert!(matches!(message, SocketMessage::Close));
    }

    #[test]
    fn websocket_text_and_binary_messages_decode_to_client_events() {
        let text = decode_socket_message(Message::Text(
            r#"{"type":"input_audio_buffer.commit"}"#.into(),
        ))
        .expect("text message");
        let binary = decode_socket_message(Message::Binary(
            br#"{"type":"input_audio_buffer.clear"}"#.to_vec().into(),
        ))
        .expect("binary message");

        assert!(matches!(text, SocketMessage::ClientEvent(raw) if raw.contains("commit")));
        assert!(matches!(binary, SocketMessage::ClientEvent(raw) if raw.contains("clear")));
    }

    #[test]
    fn websocket_binary_messages_must_be_utf8() {
        let error = match decode_socket_message(Message::Binary(vec![0xff, 0xfe].into())) {
            Ok(_) => panic!("invalid utf8 should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("binary message is not UTF-8"));
    }

    #[test]
    fn websocket_control_messages_are_rejected_except_close() {
        let error = match decode_socket_message(Message::Ping(Vec::new().into())) {
            Ok(_) => panic!("unsupported control message should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("unsupported websocket message"));
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
    fn pcm16_converter_finish_flushes_resampler_tail() {
        let mut converter = Pcm16Converter::new(48_000);

        assert!(converter.push(&1234i16.to_le_bytes()).is_empty());
        assert_eq!(converter.finish().len(), 2);
        assert!(converter.finish().is_empty());
    }

    #[test]
    fn active_turn_rejects_audio_rate_change_after_audio_started() {
        let mut turn = ActiveTurn::new_idle();
        let (pcm_tx, _pcm_rx) = mpsc::channel::<Vec<u8>>(1);
        turn.ensure_started_with(|_| pcm_tx).expect("open input");
        let session = RealtimeSession::with_id("sess_test", "seed-asr");
        let update = SessionUpdateConfig {
            input_sample_rate: Some(session.input_sample_rate + 1),
            ..SessionUpdateConfig::default()
        };

        let error = validate_session_update_timing(&turn, &session, &update)
            .expect_err("active turn should reject rate changes");

        assert!(error
            .to_string()
            .contains("cannot change audio input format while a turn is active"));
    }

    #[test]
    fn api_key_is_optional_when_not_configured() {
        validate_api_key(None, None, None).expect("no api key");
    }

    #[test]
    fn api_key_accepts_bearer_authorization() {
        validate_api_key(Some("Bearer local-secret"), None, Some("local-secret"))
            .expect("bearer key");
        validate_api_key(Some("Bearer  local-secret "), None, Some("local-secret"))
            .expect("bearer key with token whitespace");
    }

    #[test]
    fn api_key_query_parameter_is_not_supported() {
        assert!(validate_api_key(None, None, Some("local-secret"),).is_err());
    }

    #[test]
    fn accepts_api_key_from_openai_realtime_subprotocol() {
        validate_api_key(
            None,
            Some("realtime, openai-insecure-api-key.local-secret, other"),
            Some("local-secret"),
        )
        .expect("subprotocol api key");
    }

    #[test]
    fn accepts_api_key_from_later_openai_realtime_subprotocol() {
        validate_api_key(
            None,
            Some("openai-insecure-api-key.bad, openai-insecure-api-key.local-secret"),
            Some("local-secret"),
        )
        .expect("later subprotocol api key");
    }

    #[test]
    fn rejects_wrong_openai_realtime_subprotocol_api_key() {
        assert!(validate_api_key(
            None,
            Some("openai-insecure-api-key.wrong-secret"),
            Some("local-secret"),
        )
        .is_err());
    }

    #[test]
    fn selects_openai_realtime_api_key_subprotocol_for_response() {
        assert_eq!(
            selected_realtime_subprotocol(Some("realtime, openai-insecure-api-key.local-secret")),
            Some("openai-insecure-api-key.local-secret")
        );
        assert_eq!(selected_realtime_subprotocol(Some("realtime")), None);
        assert_eq!(selected_realtime_subprotocol(None), None);
    }

    #[test]
    fn bearer_auth_with_wrong_subprotocol_does_not_echo_subprotocol() {
        let runtime = RuntimeConfig {
            model: "seed-asr".to_string(),
            api_key: Some("local-secret".to_string()),
        };
        let request = Request::builder()
            .uri("/v1/realtime?model=seed-asr")
            .header("authorization", "Bearer local-secret")
            .header(
                "sec-websocket-protocol",
                "realtime, openai-insecure-api-key.wrong-secret",
            )
            .body(())
            .expect("request");
        let response = Response::builder().body(()).expect("response");

        let response =
            validate_realtime_request(&request, response, &runtime).expect("valid request");

        assert_eq!(
            response
                .headers()
                .get("sec-websocket-protocol")
                .and_then(|value| value.to_str().ok()),
            Some("realtime")
        );
    }

    #[test]
    fn handshake_echoes_realtime_subprotocol_after_subprotocol_auth() {
        let runtime = RuntimeConfig {
            model: "seed-asr".to_string(),
            api_key: Some("local-secret".to_string()),
        };
        let request = Request::builder()
            .uri("/v1/realtime?model=seed-asr")
            .header(
                "sec-websocket-protocol",
                "realtime, openai-insecure-api-key.bad, openai-insecure-api-key.local-secret",
            )
            .body(())
            .expect("request");
        let response = Response::builder().body(()).expect("response");

        let response =
            validate_realtime_request(&request, response, &runtime).expect("valid request");

        assert_eq!(
            response
                .headers()
                .get("sec-websocket-protocol")
                .and_then(|value| value.to_str().ok()),
            Some("realtime")
        );
    }

    #[test]
    fn disabled_auth_echoes_realtime_without_echoing_credential_subprotocol() {
        let runtime = RuntimeConfig {
            model: "seed-asr".to_string(),
            api_key: None,
        };
        let request = Request::builder()
            .uri("/v1/realtime?model=seed-asr")
            .header(
                "sec-websocket-protocol",
                "realtime, openai-insecure-api-key.local-secret",
            )
            .body(())
            .expect("request");
        let response = Response::builder().body(()).expect("response");

        let response =
            validate_realtime_request(&request, response, &runtime).expect("valid request");

        assert_eq!(
            response
                .headers()
                .get("sec-websocket-protocol")
                .and_then(|value| value.to_str().ok()),
            Some("realtime")
        );
    }

    #[test]
    fn disabled_auth_does_not_echo_credential_subprotocol_without_realtime() {
        let runtime = RuntimeConfig {
            model: "seed-asr".to_string(),
            api_key: None,
        };
        let request = Request::builder()
            .uri("/v1/realtime?model=seed-asr")
            .header(
                "sec-websocket-protocol",
                "openai-insecure-api-key.local-secret",
            )
            .body(())
            .expect("request");
        let response = Response::builder().body(()).expect("response");

        let response =
            validate_realtime_request(&request, response, &runtime).expect("valid request");

        assert_eq!(response.headers().get("sec-websocket-protocol"), None);
    }

    #[test]
    fn realtime_request_rejects_wrong_model_before_auth_echo() {
        let runtime = RuntimeConfig {
            model: "seed-asr".to_string(),
            api_key: Some("local-secret".to_string()),
        };
        let request = Request::builder()
            .uri("/v1/realtime?model=other-asr")
            .header(
                "sec-websocket-protocol",
                "realtime, openai-insecure-api-key.local-secret",
            )
            .body(())
            .expect("request");
        let response = Response::builder().body(()).expect("response");

        let error = validate_realtime_request(&request, response, &runtime)
            .expect_err("wrong model should fail");

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert!(error
            .body()
            .as_deref()
            .is_some_and(|body| body.contains("only model=seed-asr is supported")));
    }

    #[test]
    fn api_key_rejects_missing_or_invalid_key_when_configured() {
        assert!(validate_api_key(None, None, Some("local-secret")).is_err());
        assert!(validate_api_key(Some("Bearer wrong"), None, Some("local-secret")).is_err());
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
    fn encodes_model_names_for_log_urls() {
        assert_eq!(encode_query_component("seed-asr"), "seed-asr");
        assert_eq!(encode_query_component("custom/asr 1"), "custom%2Fasr%201");
    }
}
