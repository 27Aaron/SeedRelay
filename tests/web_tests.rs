use seedrelay::web::{
    http_response_with_config, WebRuntimeConfig, APP_JS, FONT_CSS, INDEX_HTML, STYLES_CSS,
};

fn default_web_response(method: &str, target: &str) -> Option<String> {
    http_response_with_config(method, target, &WebRuntimeConfig::default(), true)
}

#[test]
fn index_html_loads_runtime_config_for_realtime_endpoint() {
    assert!(INDEX_HTML.contains("<title>SeedRelay</title>"));
    assert!(APP_JS.contains("fetch(\"/config.json\""));
    assert!(APP_JS.contains("model: \"seed-asr\""));
    assert!(APP_JS.contains("authRequired: false"));
    assert!(INDEX_HTML.contains(r#"id="apiKey""#));
    assert!(INDEX_HTML.contains(r#"type="password""#));
    assert!(APP_JS.contains("localStorage.getItem(API_KEY_STORAGE_KEY)"));
    assert!(APP_JS.contains("localStorage.setItem(API_KEY_STORAGE_KEY, apiKey)"));
    assert!(APP_JS.contains("url.searchParams.set(\"model\", runtimeConfig.model);"));
    assert!(APP_JS.contains("\"openai-insecure-api-key.\" + apiKey"));
    assert!(APP_JS.contains("new WebSocket(realtimeUrl(), realtimeProtocols())"));
    assert!(APP_JS.contains("displayRealtimeUrl()"));
    assert!(!APP_JS.contains("url.searchParams.set(\"api_key\""));
    assert!(!APP_JS.contains("runtimeConfig.apiKey"));
    assert!(!APP_JS.contains("/v1/realtime?model=seed-asr-2.0"));
    assert!(APP_JS.contains("input_audio_buffer.append"));
    assert!(APP_JS.contains("conversation.item.input_audio_transcription.delta"));
}

#[test]
fn index_html_loads_external_assets() {
    assert!(INDEX_HTML
        .contains(r#"<link rel="preconnect" href="https://gw.alipayobjects.com" crossorigin />"#));
    assert!(INDEX_HTML.contains(r#"<link rel="stylesheet" href="/font.css" />"#));
    assert!(INDEX_HTML.contains(r#"<link rel="stylesheet" href="/styles.css" />"#));
    assert!(INDEX_HTML.contains(r#"<script src="/app.js" defer></script>"#));
    assert!(!INDEX_HTML.contains("<style>"));
    assert!(!INDEX_HTML.contains("const els ="));
}

#[test]
fn index_html_uses_jinkai_font_across_the_ui() {
    assert!(FONT_CSS.contains("font-family: 'TsangerJinKai02';"));
    assert!(FONT_CSS.contains("https://gw.alipayobjects.com/os/k/jinkai/"));
    assert!(STYLES_CSS.contains(r#"--app-font: "TsangerJinKai02";"#));
    assert!(STYLES_CSS.contains("font-family: var(--app-font);"));
    assert!(STYLES_CSS.contains("font: 12px/1.2 var(--app-font);"));
    assert!(!STYLES_CSS.contains("--mono-font"));
    assert!(!STYLES_CSS.contains("--serif-font"));
    assert!(!STYLES_CSS.contains("monospace"));
    assert!(!STYLES_CSS.contains("Iowan Old Style"));
    assert!(!STYLES_CSS.contains("Songti SC"));
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
fn app_js_applies_audio_backpressure_and_batching() {
    assert!(APP_JS.contains("const MAX_WS_BUFFERED_BYTES = 512 * 1024;"));
    assert!(APP_JS.contains("const AUDIO_BATCH_MS = 20;"));
    assert!(APP_JS.contains("ws.bufferedAmount > MAX_WS_BUFFERED_BYTES"));
    assert!(APP_JS.contains("flushAudioBatch()"));
    assert!(APP_JS.contains("pendingAudioSamples"));
}

#[test]
fn app_js_reports_websocket_close_and_error_details() {
    assert!(APP_JS.contains("function formatCloseReason(event)"));
    assert!(APP_JS.contains("event.code"));
    assert!(APP_JS.contains("event.reason"));
    assert!(APP_JS.contains("event.wasClean"));
    assert!(APP_JS.contains("ws.addEventListener(\"close\", (event) =>"));
    assert!(APP_JS.contains("log(reason, \"socket\")"));
    assert!(APP_JS.contains("function formatSocketError(event)"));
    assert!(APP_JS.contains("typeof event.message === \"string\""));
    assert!(APP_JS.contains("log(formatSocketError(event), \"error\")"));
}

#[test]
fn index_html_keeps_brand_on_one_line() {
    assert!(INDEX_HTML.contains(r#"<h1 class="wordmark" aria-label="Seed Relay">"#));
    assert!(INDEX_HTML.contains(r#"<span>Seed</span>"#));
    assert!(INDEX_HTML.contains(r#"<span>Relay</span>"#));
    assert!(INDEX_HTML.contains(r#"<div class="tag" id="modelName">seed-asr</div>"#));
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
    assert!(!INDEX_HTML.contains("seed-asr-2.0</div>"));
}

#[test]
fn index_html_aligns_sidebar_metrics_and_signal_meter() {
    assert!(STYLES_CSS.contains("--hairline: 1px solid var(--line);"));
    assert!(STYLES_CSS.contains("--panel-padding: 22px;"));
    assert!(STYLES_CSS.contains("--inset-padding: 14px;"));
    assert!(STYLES_CSS.contains("display: grid;"));
    assert!(STYLES_CSS.contains("grid-template-rows: auto auto;"));
    assert!(STYLES_CSS.contains("align-content: space-between;"));
    assert!(STYLES_CSS.contains("margin-bottom: 0;"));
    assert!(STYLES_CSS.contains("grid-template-columns: minmax(0, 1fr) max-content;"));
    assert!(STYLES_CSS.contains("min-width: 4ch;"));
    assert!(STYLES_CSS.contains("font-family: var(--app-font);"));
    assert!(!STYLES_CSS.contains("grid-template-columns: minmax(0, 1fr) 54px;"));
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
    assert!(APP_JS.contains("let committedTranscriptText = \"\";"));
    assert!(APP_JS.contains("function partitionTranscript(text)"));
    assert!(APP_JS.contains("function splitTranscript(text)"));
    assert!(APP_JS.contains("function renderTranscript(text)"));
    assert!(APP_JS.contains("const nextTranscript ="));
    assert!(APP_JS.contains("event.transcript || transcriptText + (event.delta || \"\")"));
    assert!(APP_JS.contains("const { committed, active } = partitionTranscript(text);"));
    assert!(APP_JS.contains("renderTranscript(transcriptText);"));
    assert!(APP_JS.contains("renderFinalTranscript(committedTranscriptText);"));
    assert!(APP_JS.contains("commitFinalTranscript(event.transcript || transcriptText);"));
    assert!(APP_JS.contains("renderLiveTranscript(nextTranscript)"));
    assert!(APP_JS.contains("if (!isRecording) setSocket(\"completed\", true);"));
    assert!(!APP_JS.contains("els.partial.textContent + (event.delta || \"\")"));
    assert!(!APP_JS.contains("appendTranscriptDelta(event.delta || \"\")"));
    assert!(!APP_JS.contains("renderTranscript(active);"));
    assert!(!APP_JS.contains("renderFinalTranscript(transcriptText)"));
    assert!(!APP_JS.contains("              setSocket(\"completed\", true);"));
}

#[test]
fn app_js_handles_invalid_server_events() {
    let message_handler = APP_JS
        .split("ws.addEventListener(\"message\"")
        .nth(1)
        .expect("message handler");

    assert!(message_handler.contains("try {"));
    assert!(message_handler.contains("JSON.parse(message.data)"));
    assert!(message_handler.contains("invalid server event"));
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
    assert!(APP_JS.contains("signalTargetLevel ="));
    assert!(APP_JS.contains("Math.max(signalTargetLevel * 0.86, level)"));
    assert!(APP_JS.contains("function signalStateForPeak(peak)"));
    assert!(APP_JS.contains("els.signalValue.textContent = `${Math.round(signalDisplayLevel)}%`;"));
    assert!(APP_JS.contains(
        "els.signalPanel.style.setProperty(\"--signal-level\", `${signalDisplayLevel}%`);"
    ));
}

#[test]
fn index_html_renders_signal_as_oscilloscope_waveform() {
    assert!(INDEX_HTML.contains(r#"class="scope-wave""#));
    assert!(INDEX_HTML.contains(r#"id="signalWaveGlow""#));
    assert!(INDEX_HTML.contains(r#"id="signalWaveTrail""#));
    assert!(INDEX_HTML.contains(r#"id="signalWavePrimary""#));
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
    assert!(APP_JS.contains("signalDisplayLevel +="));
    assert!(APP_JS.contains("(signalTargetLevel - signalDisplayLevel) * SIGNAL_EASING"));
    assert!(APP_JS
        .contains("const frameMs = Math.min(34, Math.max(0, now - lastSignalFrameAt || 16));"));
    assert!(APP_JS.contains("signalPhase ="));
    assert!(APP_JS.contains("signalDisplayLevel / 52000"));
    assert!(APP_JS.contains("isRecording ||"));
    assert!(APP_JS.contains("signalTargetLevel > SIGNAL_IDLE_THRESHOLD"));
    assert!(APP_JS.contains("signalDisplayLevel > SIGNAL_IDLE_THRESHOLD"));
    assert!(!APP_JS.contains("SIGNAL_WAVE_INTERVAL_MS"));
    assert!(STYLES_CSS.contains("stroke-width: 3.5;"));
    assert!(STYLES_CSS.contains("opacity: 0.24;"));
    assert!(INDEX_HTML.contains(r#"stdDeviation="1.0""#));
    assert!(INDEX_HTML.contains(r#"result="blur""#));
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
fn index_html_uses_refined_scrollbars() {
    assert!(STYLES_CSS.contains("scrollbar-width: thin;"));
    assert!(STYLES_CSS.contains("scrollbar-gutter: stable;"));
    assert!(STYLES_CSS.contains("::-webkit-scrollbar"));
    assert!(STYLES_CSS.contains("::-webkit-scrollbar-thumb"));
    assert!(STYLES_CSS.contains("margin-block: 0;"));
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
    assert!(APP_JS.contains("els.final.scrollTop = els.final.scrollHeight;"));
    assert!(!STYLES_CSS.contains("font-size: clamp(20px, 2.2vw, 32px);"));
    assert!(!STYLES_CSS.contains("font-size: clamp(18px, 2vw, 28px);"));
}

#[test]
fn renders_index_page_response() {
    let response = default_web_response("GET", "/").expect("index response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-type: text/html; charset=utf-8\r\n"));
    assert!(response.contains("SeedRelay"));
}

#[test]
fn serves_static_web_assets() {
    let css = default_web_response("GET", "/styles.css").expect("css response");
    let font_css = default_web_response("GET", "/font.css").expect("font css response");
    let js = default_web_response("GET", "/app.js").expect("js response");

    assert!(css.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(css.contains("content-type: text/css; charset=utf-8\r\n"));
    assert!(css.contains(".signal-wave-primary"));
    assert!(font_css.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(font_css.contains("content-type: text/css; charset=utf-8\r\n"));
    assert!(font_css.contains("TsangerJinKai02"));
    assert!(js.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(js.contains("content-type: application/javascript; charset=utf-8\r\n"));
    assert!(js.contains("requestAnimationFrame(drawSignalFrame);"));
}

#[test]
fn serves_runtime_web_config() {
    let config = WebRuntimeConfig {
        model: "custom-asr".into(),
        api_key: Some("local-secret".into()),
    };
    let response =
        http_response_with_config("GET", "/config.json", &config, true).expect("config response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-type: application/json; charset=utf-8\r\n"));
    assert!(response.contains(r#""model":"custom-asr""#));
    assert!(response.contains(r#""authRequired":true"#));
    assert!(!response.contains("local-secret"));
    assert!(!response.contains("apiKey"));
}

#[test]
fn default_runtime_web_config_has_no_api_key() {
    let response = default_web_response("GET", "/config.json").expect("config response");

    assert!(response.contains(r#""model":"seed-asr""#));
    assert!(response.contains(r#""authRequired":false"#));
    assert!(!response.contains("apiKey"));
}

#[test]
fn rejects_unknown_http_path() {
    let response = default_web_response("GET", "/missing").expect("404 response");

    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
}

#[test]
fn health_endpoint_is_available_even_when_web_ui_is_disabled() {
    let config = WebRuntimeConfig::default();
    let get = http_response_with_config("GET", "/health", &config, false).expect("health get");
    let head = http_response_with_config("HEAD", "/health", &config, false).expect("health head");

    assert!(get.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(get.ends_with(r#"{"ok":true}"#));
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(head.contains("content-length: 11\r\n"));
    assert!(head.ends_with("\r\n\r\n"));
    assert!(!head.ends_with(r#"{"ok":true}"#));
}

#[test]
fn web_ui_assets_return_404_when_web_ui_is_disabled() {
    let config = WebRuntimeConfig::default();

    for target in [
        "/",
        "/index.html",
        "/font.css",
        "/styles.css",
        "/app.js",
        "/config.json",
    ] {
        let response =
            http_response_with_config("GET", target, &config, false).expect("disabled web");

        assert!(
            response.starts_with("HTTP/1.1 404 Not Found\r\n"),
            "{target} should be disabled"
        );
        assert!(response.ends_with("not found"));
    }
}

#[test]
fn serves_openai_model_list_endpoint() {
    let config = WebRuntimeConfig {
        model: "custom-asr".into(),
        api_key: None,
    };
    let response =
        http_response_with_config("GET", "/v1/models", &config, false).expect("models response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-type: application/json; charset=utf-8\r\n"));
    assert!(response.contains(r#""object":"list""#));
    assert!(response.contains(r#""id":"custom-asr""#));
    assert!(response.contains(r#""owned_by":"seedrelay""#));
}

#[test]
fn openai_model_endpoints_work_when_web_ui_is_disabled() {
    let config = WebRuntimeConfig {
        model: "seed-asr".into(),
        api_key: None,
    };
    let response =
        http_response_with_config("GET", "/v1/models", &config, false).expect("models response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains(r#""id":"seed-asr""#));
}

#[test]
fn serves_openai_single_model_endpoint() {
    let config = WebRuntimeConfig {
        model: "custom/asr".into(),
        api_key: None,
    };
    let response = http_response_with_config("GET", "/v1/models/custom%2Fasr", &config, false)
        .expect("model response");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains(r#""id":"custom/asr""#));
    assert!(response.contains(r#""object":"model""#));
}

#[test]
fn serves_encoded_openai_model_names_with_spaces() {
    let config = WebRuntimeConfig {
        model: "custom/asr v2".into(),
        api_key: None,
    };

    for target in ["/v1/models/custom%2Fasr%20v2", "/v1/models/custom%2Fasr+v2"] {
        let response =
            http_response_with_config("GET", target, &config, false).expect("model response");

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains(r#""id":"custom/asr v2""#));
    }
}

#[test]
fn returns_model_not_found_for_unknown_model_endpoint() {
    let config = WebRuntimeConfig {
        model: "seed-asr".into(),
        api_key: None,
    };
    let response = http_response_with_config("GET", "/v1/models/other", &config, false)
        .expect("missing model response");

    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(response.contains(r#""code":"model_not_found""#));
    assert!(response.contains(r#""param":"model""#));
}

#[test]
fn openai_model_endpoint_errors_include_requested_encoded_model() {
    let config = WebRuntimeConfig {
        model: "seed-asr".into(),
        api_key: None,
    };
    let response = http_response_with_config("GET", "/v1/models/missing%2Fasr+v1", &config, false)
        .expect("missing model response");

    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(response.contains("missing/asr v1"));
    assert!(response.contains(r#""code":"model_not_found""#));
}

#[test]
fn unsupported_methods_do_not_serve_openai_model_endpoints() {
    let config = WebRuntimeConfig {
        model: "seed-asr".into(),
        api_key: None,
    };

    for target in ["/v1/models", "/v1/models/seed-asr"] {
        let response =
            http_response_with_config("POST", target, &config, false).expect("method response");

        assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(!response.contains(r#""object":"list""#));
        assert!(!response.contains(r#""object":"model""#));
    }
}

#[test]
fn model_endpoints_support_head_without_body() {
    let config = WebRuntimeConfig {
        model: "seed-asr".into(),
        api_key: None,
    };
    let response =
        http_response_with_config("HEAD", "/v1/models", &config, false).expect("models head");

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("content-length: "));
    assert!(response.ends_with("\r\n\r\n"));
    assert!(!response.contains(r#""object":"list""#));
}
