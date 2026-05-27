use seedrelay::web::{http_response, APP_JS, INDEX_HTML, STYLES_CSS};

#[test]
fn index_html_uses_seed_asr_realtime_endpoint() {
    assert!(INDEX_HTML.contains("<title>SeedRelay</title>"));
    assert!(APP_JS.contains("/v1/realtime?model=seed-asr-2.0"));
    assert!(APP_JS.contains("input_audio_buffer.append"));
    assert!(APP_JS.contains("conversation.item.input_audio_transcription.delta"));
}

#[test]
fn index_html_loads_external_assets() {
    assert!(INDEX_HTML.contains(r#"<link rel="stylesheet" href="/styles.css" />"#));
    assert!(INDEX_HTML.contains(r#"<script src="/app.js" defer></script>"#));
    assert!(!INDEX_HTML.contains("<style>"));
    assert!(!INDEX_HTML.contains("const els ="));
}

#[test]
fn index_html_stops_capture_before_committing_audio() {
    let guard = "if (!isRecording || !ws || ws.readyState !== WebSocket.OPEN) return;";
    let stop_capture = "isRecording = false;";
    let commit = r#"sendJson({ type: "input_audio_buffer.commit" });"#;
    let stop_function_start = APP_JS.find("function stop()").expect("stop function");
    let stop_function = &APP_JS[stop_function_start..];

    assert!(APP_JS.contains("let isRecording = false;"));
    assert!(APP_JS.contains(guard));
    assert!(
        stop_function.find(stop_capture).expect("stop capture")
            < stop_function.find(commit).expect("commit audio")
    );
}

#[test]
fn index_html_keeps_brand_on_one_line() {
    assert!(INDEX_HTML.contains(r#"<h1 class="wordmark" aria-label="Seed Relay">"#));
    assert!(INDEX_HTML.contains(r#"<span>Seed</span>"#));
    assert!(INDEX_HTML.contains(r#"<span>Relay</span>"#));
    assert!(INDEX_HTML.contains(r#"<div class="tag">Seed-ASR</div>"#));
    assert!(STYLES_CSS.contains("white-space: nowrap;"));
    assert!(STYLES_CSS.contains("grid-template-columns: minmax(0, 1fr) auto;"));
    assert!(STYLES_CSS.contains("align-items: end;"));
    assert!(STYLES_CSS.contains("padding-bottom: 12px;"));
    assert!(STYLES_CSS.contains("font-size: clamp(28px, 3.5vw, 46px);"));
    assert!(STYLES_CSS.contains("gap: clamp(10px, 1.6vw, 22px);"));
    assert!(STYLES_CSS.contains("justify-self: end;"));
    assert!(STYLES_CSS.contains("align-self: end;"));
    assert!(STYLES_CSS.contains("margin: 16px 0 18px;"));
    assert!(STYLES_CSS.contains("grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);"));
    assert!(STYLES_CSS.contains("@media (max-width: 520px)"));
    assert!(!INDEX_HTML.contains("Seed<br />Relay"));
    assert!(!INDEX_HTML.contains("ByteDance/Seed-ASR"));
    assert!(!INDEX_HTML.contains("seed-asr-2.0</div>"));
}

#[test]
fn index_html_renders_transcript_sections() {
    assert!(INDEX_HTML.contains(r#"<div class="label label-row">"#));
    assert!(INDEX_HTML.contains(r#"<span class="live-dot" aria-hidden="true"></span>"#));
    assert!(INDEX_HTML.contains(r#"id="partial""#));
    assert!(APP_JS.contains("transcript-line"));
    assert!(APP_JS.contains("line-index"));
    assert!(APP_JS.contains("line-text"));
    assert!(INDEX_HTML.contains(r#"data-placeholder="Listening transcript appears here""#));
    assert!(INDEX_HTML.contains(r#"<div class="final-wrap">"#));
    assert!(INDEX_HTML.contains(r#"<div class="final-label">Final</div>"#));
}

#[test]
fn index_html_streams_transcript_as_rows() {
    assert!(APP_JS.contains("const MAX_TRANSCRIPT_LINE = 12;"));
    assert!(APP_JS.contains("function splitTranscript(text)"));
    assert!(APP_JS.contains("function renderTranscript(text)"));
    assert!(APP_JS.contains("appendTranscriptDelta(event.delta || \"\")"));
    assert!(APP_JS.contains("renderTranscript(event.transcript || transcriptText)"));
    assert!(APP_JS.contains("if (!isRecording) setSocket(\"completed\", true);"));
    assert!(!APP_JS.contains("els.partial.textContent + (event.delta || \"\")"));
    assert!(!APP_JS.contains("              setSocket(\"completed\", true);"));
}

#[test]
fn index_html_places_events_under_signal_with_buttons_at_bottom() {
    let control = INDEX_HTML
        .find(r#"<section class="panel control">"#)
        .expect("control panel");
    let signal = INDEX_HTML
        .find(r#"<section class="signal-panel">"#)
        .expect("signal panel");
    let events = INDEX_HTML
        .find(r#"<section class="events">"#)
        .expect("events block");
    let buttons = INDEX_HTML
        .find(r#"<div class="buttons">"#)
        .expect("buttons");
    let readout = INDEX_HTML
        .find(r#"<section class="panel readout">"#)
        .expect("readout panel");

    assert!(control < signal);
    assert!(signal < events);
    assert!(events < buttons);
    assert!(buttons < readout);
    assert!(STYLES_CSS.contains("block-size: var(--final-panel-block);"));
    assert!(APP_JS.contains("const MAX_EVENT_LINES = 5;"));
    assert!(APP_JS.contains("slice(0, MAX_EVENT_LINES)"));
    assert!(STYLES_CSS.contains("margin-top: auto;"));
    assert!(!INDEX_HTML.contains(r#"<section class="panel events">"#));
    assert!(!INDEX_HTML.contains(r#"<section class="transcript">"#));
}

#[test]
fn index_html_emphasizes_signal_activity() {
    assert!(INDEX_HTML.contains(r#"<strong id="signalValue">0%</strong>"#));
    assert!(INDEX_HTML.contains(r#"<span id="signalState">quiet</span>"#));
    assert!(STYLES_CSS.contains("--signal-level: 0%;"));
    assert!(APP_JS.contains("let signalTargetLevel = 0;"));
    assert!(APP_JS.contains("let signalDisplayLevel = 0;"));
    assert!(APP_JS.contains("function signalLevelFromPeak(peak)"));
    assert!(APP_JS.contains("Math.sqrt(Math.min(1, Math.max(0, peak))) * 125"));
    assert!(APP_JS.contains("if (peak >= 0.7) return \"clipping\";"));
    assert!(APP_JS.contains("if (peak >= 0.01) return \"voice\";"));
    assert!(APP_JS.contains(
        "signalTargetLevel = peak === 0 ? 0 : Math.max(signalTargetLevel * 0.86, level);"
    ));
    assert!(APP_JS.contains("function signalStateForPeak(peak)"));
    assert!(APP_JS.contains("els.signalValue.textContent = `${Math.round(signalDisplayLevel)}%`;"));
    assert!(APP_JS.contains(
        "els.signalPanel.style.setProperty(\"--signal-level\", `${signalDisplayLevel}%`);"
    ));
}

#[test]
fn index_html_renders_signal_as_oscilloscope_waveform() {
    assert!(INDEX_HTML.contains(r#"<svg class="scope-wave""#));
    assert!(INDEX_HTML.contains(r#"<polyline id="signalWaveGlow""#));
    assert!(INDEX_HTML.contains(r#"<polyline id="signalWaveTrail""#));
    assert!(INDEX_HTML.contains(r#"<polyline id="signalWavePrimary""#));
    assert!(APP_JS.contains("signalWaveGlow: document.querySelector(\"#signalWaveGlow\")"));
    assert!(APP_JS.contains("let signalPhase = 0;"));
    assert!(APP_JS.contains("function buildWavePoints(level, phase, offset = 0)"));
    assert!(APP_JS.contains("function drawSignalFrame(now)"));
    assert!(APP_JS.contains("els.signalWavePrimary.setAttribute(\"points\", primary);"));
    assert!(APP_JS.contains("requestAnimationFrame(drawSignalFrame);"));
}

#[test]
fn index_html_keeps_signal_waveform_subtle_and_smooth() {
    assert!(APP_JS.contains("const SIGNAL_EASING = 0.18;"));
    assert!(APP_JS.contains("const SIGNAL_IDLE_THRESHOLD = 0.08;"));
    assert!(APP_JS.contains("let signalAnimationFrame = null;"));
    assert!(APP_JS.contains(
        "signalDisplayLevel += (signalTargetLevel - signalDisplayLevel) * SIGNAL_EASING;"
    ));
    assert!(APP_JS
        .contains("const frameMs = Math.min(34, Math.max(0, now - lastSignalFrameAt || 16));"));
    assert!(APP_JS.contains("signalPhase = (signalPhase + frameMs * (0.0008 + signalDisplayLevel / 52000)) % (Math.PI * 2);"));
    assert!(APP_JS.contains("if (isRecording || signalTargetLevel > SIGNAL_IDLE_THRESHOLD || signalDisplayLevel > SIGNAL_IDLE_THRESHOLD)"));
    assert!(!APP_JS.contains("SIGNAL_WAVE_INTERVAL_MS"));
    assert!(STYLES_CSS.contains("stroke-width: 3.5;"));
    assert!(STYLES_CSS.contains("opacity: 0.24;"));
    assert!(INDEX_HTML.contains(r#"<feGaussianBlur stdDeviation="1.0" result="blur" />"#));
}

#[test]
fn index_html_scrolls_transcript_rows_within_a_fixed_panel() {
    assert!(STYLES_CSS.contains("--final-panel-block: calc((100vh - 64px) / 6);"));
    assert!(
        STYLES_CSS.contains("grid-template-rows: auto minmax(0, 1fr) var(--final-panel-block);")
    );
    assert!(STYLES_CSS.contains("block-size: 100%;"));
    assert!(STYLES_CSS.contains("overflow: auto;"));
    assert!(!APP_JS.contains("return lines.slice(-12);"));
}

#[test]
fn index_html_uses_smaller_transcript_body_type() {
    assert!(STYLES_CSS.contains("--transcript-content-size: clamp(14px, 1.05vw, 18px);"));
    assert!(
        STYLES_CSS.contains("--final-body-color: color-mix(in srgb, var(--ink) 94%, transparent);")
    );
    assert!(STYLES_CSS.contains("font-size: var(--transcript-content-size);"));
    assert!(!STYLES_CSS.contains("--transcript-body-size"));
    assert!(!STYLES_CSS.contains("--final-body-size"));
    assert!(STYLES_CSS.contains("color: var(--final-body-color);"));
    assert!(APP_JS.contains("els.partial.scrollTop = els.partial.scrollHeight;"));
    assert!(!STYLES_CSS.contains("font-size: clamp(20px, 2.2vw, 32px);"));
    assert!(!STYLES_CSS.contains("font-size: clamp(18px, 2vw, 28px);"));
}

#[test]
fn renders_index_page_response() {
    let response = http_response("GET", "/").expect("index response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-type: text/html; charset=utf-8\r\n"));
    assert!(response.contains("SeedRelay"));
}

#[test]
fn serves_static_web_assets() {
    let css = http_response("GET", "/styles.css").expect("css response");
    let js = http_response("GET", "/app.js").expect("js response");

    assert!(css.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(css.contains("content-type: text/css; charset=utf-8\r\n"));
    assert!(css.contains(".signal-wave-primary"));
    assert!(js.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(js.contains("content-type: application/javascript; charset=utf-8\r\n"));
    assert!(js.contains("requestAnimationFrame(drawSignalFrame);"));
}

#[test]
fn rejects_unknown_http_path() {
    let response = http_response("GET", "/missing").expect("404 response");

    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
}
