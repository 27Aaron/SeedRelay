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
    assert!(INDEX_HTML.contains("if (!isRecording) setSocket(\"completed\", true);"));
    assert!(!INDEX_HTML.contains("els.partial.textContent + (event.delta || \"\")"));
    assert!(!INDEX_HTML.contains("              setSocket(\"completed\", true);"));
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
    assert!(INDEX_HTML.contains("block-size: var(--final-panel-block);"));
    assert!(INDEX_HTML.contains("const MAX_EVENT_LINES = 5;"));
    assert!(INDEX_HTML.contains("slice(0, MAX_EVENT_LINES)"));
    assert!(INDEX_HTML.contains("margin-top: auto;"));
    assert!(!INDEX_HTML.contains(r#"<section class="panel events">"#));
    assert!(!INDEX_HTML.contains(r#"<section class="transcript">"#));
}

#[test]
fn index_html_emphasizes_signal_activity() {
    assert!(INDEX_HTML.contains(r#"<strong id="signalValue">0%</strong>"#));
    assert!(INDEX_HTML.contains(r#"<span id="signalState">quiet</span>"#));
    assert!(INDEX_HTML.contains("--signal-level: 0%;"));
    assert!(INDEX_HTML.contains("let signalPeak = 0;"));
    assert!(INDEX_HTML.contains("function signalLevelFromPeak(peak)"));
    assert!(INDEX_HTML.contains("Math.sqrt(Math.min(1, Math.max(0, peak))) * 125"));
    assert!(INDEX_HTML.contains("if (peak >= 0.7) return \"clipping\";"));
    assert!(INDEX_HTML.contains("if (peak >= 0.01) return \"voice\";"));
    assert!(
        INDEX_HTML.contains("signalPeak = peak === 0 ? 0 : Math.max(signalPeak * 0.86, level);")
    );
    assert!(INDEX_HTML.contains("function signalStateForPeak(peak)"));
    assert!(INDEX_HTML.contains("els.signalState.textContent = signalStateForPeak(peak);"));
    assert!(INDEX_HTML.contains("els.signalValue.textContent = `${Math.round(signalPeak)}%`;"));
    assert!(INDEX_HTML
        .contains("els.signalPanel.style.setProperty(\"--signal-level\", `${signalPeak}%`);"));
}

#[test]
fn index_html_renders_signal_as_oscilloscope_waveform() {
    assert!(INDEX_HTML.contains(r#"<svg class="scope-wave""#));
    assert!(INDEX_HTML.contains(r#"<polyline id="signalWaveGlow""#));
    assert!(INDEX_HTML.contains(r#"<polyline id="signalWaveTrail""#));
    assert!(INDEX_HTML.contains(r#"<polyline id="signalWavePrimary""#));
    assert!(INDEX_HTML.contains("signalWaveGlow: document.querySelector(\"#signalWaveGlow\")"));
    assert!(INDEX_HTML.contains("let signalPhase = 0;"));
    assert!(INDEX_HTML.contains("function buildWavePoints(level, phase, offset = 0)"));
    assert!(INDEX_HTML.contains("function updateSignalWave(level, force = false)"));
    assert!(INDEX_HTML.contains("els.signalWavePrimary.setAttribute(\"points\", primary);"));
    assert!(INDEX_HTML.contains("updateSignalWave(signalPeak, peak === 0);"));
}

#[test]
fn index_html_keeps_signal_waveform_subtle_and_slow() {
    assert!(INDEX_HTML.contains("const SIGNAL_WAVE_INTERVAL_MS = 140;"));
    assert!(INDEX_HTML.contains("let lastSignalWaveAt = 0;"));
    assert!(INDEX_HTML.contains("let lastSignalWaveLevel = 0;"));
    assert!(INDEX_HTML.contains("const amplitude = level === 0 ? 0 : 0.8 + level * 0.1;"));
    assert!(INDEX_HTML.contains("function updateSignalWave(level, force = false)"));
    assert!(INDEX_HTML.contains("const becameActive = lastSignalWaveLevel === 0 && level > 0;"));
    assert!(INDEX_HTML.contains(
        "if (!force && !becameActive && now - lastSignalWaveAt < SIGNAL_WAVE_INTERVAL_MS) return;"
    ));
    assert!(INDEX_HTML.contains("lastSignalWaveLevel = level;"));
    assert!(
        INDEX_HTML.contains("signalPhase = (signalPhase + 0.018 + level / 900) % (Math.PI * 2);")
    );
    assert!(INDEX_HTML.contains("updateSignalWave(signalPeak, peak === 0);"));
    assert!(INDEX_HTML.contains("stroke-width: 3.5;"));
    assert!(INDEX_HTML.contains("opacity: 0.24;"));
    assert!(INDEX_HTML.contains(r#"<feGaussianBlur stdDeviation="1.0" result="blur" />"#));
}

#[test]
fn index_html_scrolls_transcript_rows_within_a_fixed_panel() {
    assert!(INDEX_HTML.contains("--final-panel-block: calc((100vh - 64px) / 6);"));
    assert!(
        INDEX_HTML.contains("grid-template-rows: auto minmax(0, 1fr) var(--final-panel-block);")
    );
    assert!(INDEX_HTML.contains("block-size: 100%;"));
    assert!(INDEX_HTML.contains("overflow: auto;"));
    assert!(!INDEX_HTML.contains("return lines.slice(-12);"));
}

#[test]
fn index_html_uses_smaller_transcript_body_type() {
    assert!(INDEX_HTML.contains("--transcript-body-size: clamp(18px, 1.45vw, 24px);"));
    assert!(INDEX_HTML.contains("--final-body-size: clamp(16px, 1.2vw, 20px);"));
    assert!(
        INDEX_HTML.contains("--final-body-color: color-mix(in srgb, var(--ink) 94%, transparent);")
    );
    assert!(INDEX_HTML.contains("font-size: var(--transcript-body-size);"));
    assert!(INDEX_HTML.contains("font-size: var(--final-body-size);"));
    assert!(INDEX_HTML.contains("color: var(--final-body-color);"));
    assert!(INDEX_HTML.contains("els.partial.scrollTop = els.partial.scrollHeight;"));
    assert!(!INDEX_HTML.contains("font-size: clamp(20px, 2.2vw, 32px);"));
    assert!(!INDEX_HTML.contains("font-size: clamp(18px, 2vw, 28px);"));
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
