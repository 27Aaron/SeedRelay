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
