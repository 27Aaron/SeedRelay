use seedrelay::web::{http_response, INDEX_HTML};

#[test]
fn index_html_uses_seed_asr_realtime_endpoint() {
    assert!(INDEX_HTML.contains("<title>SeedRelay</title>"));
    assert!(INDEX_HTML.contains("/v1/realtime?model=seed-asr-2.0"));
    assert!(INDEX_HTML.contains("input_audio_buffer.append"));
    assert!(INDEX_HTML.contains("conversation.item.input_audio_transcription.delta"));
}

#[test]
fn index_html_stops_capture_before_committing_audio() {
    let guard = "if (!isRecording || !ws || ws.readyState !== WebSocket.OPEN) return;";
    let stop_capture = "isRecording = false;";
    let commit = r#"sendJson({ type: "input_audio_buffer.commit" });"#;
    let stop_function_start = INDEX_HTML.find("function stop()").expect("stop function");
    let stop_function = &INDEX_HTML[stop_function_start..];

    assert!(INDEX_HTML.contains("let isRecording = false;"));
    assert!(INDEX_HTML.contains(guard));
    assert!(
        stop_function.find(stop_capture).expect("stop capture")
            < stop_function.find(commit).expect("commit audio")
    );
}

#[test]
fn index_html_keeps_brand_on_one_line() {
    assert!(INDEX_HTML.contains(r#"<h1 class="wordmark">Seed Relay</h1>"#));
    assert!(INDEX_HTML.contains("white-space: nowrap;"));
    assert!(INDEX_HTML.contains("grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);"));
    assert!(INDEX_HTML.contains("@media (max-width: 520px)"));
    assert!(INDEX_HTML.contains("flex-direction: column;"));
    assert!(!INDEX_HTML.contains("Seed<br />Relay"));
}

#[test]
fn index_html_renders_transcript_sections() {
    assert!(INDEX_HTML.contains(r#"<div class="label label-row">"#));
    assert!(INDEX_HTML.contains(r#"<span class="live-dot" aria-hidden="true"></span>"#));
    assert!(INDEX_HTML.contains(r#"id="partial""#));
    assert!(INDEX_HTML.contains("transcript-line"));
    assert!(INDEX_HTML.contains("line-index"));
    assert!(INDEX_HTML.contains("line-text"));
    assert!(INDEX_HTML.contains(r#"data-placeholder="Listening transcript appears here""#));
    assert!(INDEX_HTML.contains(r#"<div class="final-wrap">"#));
    assert!(INDEX_HTML.contains(r#"<div class="final-label">Final</div>"#));
}

#[test]
fn index_html_streams_transcript_as_rows() {
    assert!(INDEX_HTML.contains("const MAX_TRANSCRIPT_LINE = 12;"));
    assert!(INDEX_HTML.contains("function splitTranscript(text)"));
    assert!(INDEX_HTML.contains("function renderTranscript(text)"));
    assert!(INDEX_HTML.contains("appendTranscriptDelta(event.delta || \"\")"));
    assert!(INDEX_HTML.contains("renderTranscript(event.transcript || transcriptText)"));
    assert!(!INDEX_HTML.contains("els.partial.textContent + (event.delta || \"\")"));
}

#[test]
fn renders_index_page_response() {
    let response = http_response("GET", "/").expect("index response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-type: text/html; charset=utf-8\r\n"));
    assert!(response.contains("SeedRelay"));
}

#[test]
fn rejects_unknown_http_path() {
    let response = http_response("GET", "/missing").expect("404 response");

    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
}
