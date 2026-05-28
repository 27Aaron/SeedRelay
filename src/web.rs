use serde_json::json;

use crate::config::DEFAULT_MODEL;
use crate::realtime::{
    decode_url_component, model_list_response, model_not_found_error, model_object_response,
};

pub const INDEX_HTML: &str = include_str!("../web/index.html");
pub const FONT_CSS: &str = include_str!("../web/font.css");
pub const STYLES_CSS: &str = include_str!("../web/styles.css");
pub const APP_JS: &str = include_str!("../web/app.js");

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WebRuntimeConfig {
    pub model: String,
    pub api_key: Option<String>,
}

impl Default for WebRuntimeConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            api_key: None,
        }
    }
}

pub fn http_response_with_config(
    method: &str,
    target: &str,
    config: &WebRuntimeConfig,
    web_enabled: bool,
) -> Option<String> {
    let path = target.split('?').next().unwrap_or(target);

    // Health endpoint is always available regardless of web_enabled.
    if matches!(method, "GET" | "HEAD") && path == "/health" {
        return Some(json_response("200 OK", r#"{"ok":true}"#, method == "HEAD"));
    }

    if matches!(method, "GET" | "HEAD") {
        if path == "/v1/models" {
            let body = model_list_response(&config.model).to_string();
            return Some(json_response("200 OK", &body, method == "HEAD"));
        }

        if let Some(raw_model) = path.strip_prefix("/v1/models/") {
            let requested_model = decode_url_component(raw_model);
            if requested_model == config.model {
                let body = model_object_response(&config.model).to_string();
                return Some(json_response("200 OK", &body, method == "HEAD"));
            }

            let body = model_not_found_error(&requested_model).to_string();
            return Some(json_response("404 Not Found", &body, method == "HEAD"));
        }
    }

    if !web_enabled {
        return Some(response(
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found",
            false,
        ));
    }

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => Some(response(
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML,
            false,
        )),
        ("GET", "/styles.css") => Some(response(
            "200 OK",
            "text/css; charset=utf-8",
            STYLES_CSS,
            false,
        )),
        ("GET", "/font.css") => Some(response(
            "200 OK",
            "text/css; charset=utf-8",
            FONT_CSS,
            false,
        )),
        ("GET", "/app.js") => Some(response(
            "200 OK",
            "application/javascript; charset=utf-8",
            APP_JS,
            false,
        )),
        ("GET", "/config.json") => {
            let body = config_json(config);
            Some(response(
                "200 OK",
                "application/json; charset=utf-8",
                &body,
                false,
            ))
        }
        ("HEAD", "/") | ("HEAD", "/index.html") => Some(response(
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML,
            true,
        )),
        ("HEAD", "/styles.css") => Some(response(
            "200 OK",
            "text/css; charset=utf-8",
            STYLES_CSS,
            true,
        )),
        ("HEAD", "/font.css") => Some(response(
            "200 OK",
            "text/css; charset=utf-8",
            FONT_CSS,
            true,
        )),
        ("HEAD", "/app.js") => Some(response(
            "200 OK",
            "application/javascript; charset=utf-8",
            APP_JS,
            true,
        )),
        ("HEAD", "/config.json") => {
            let body = config_json(config);
            Some(response(
                "200 OK",
                "application/json; charset=utf-8",
                &body,
                true,
            ))
        }
        _ => Some(response(
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found",
            false,
        )),
    }
}

fn config_json(config: &WebRuntimeConfig) -> String {
    json!({
        "model": config.model,
        "authRequired": config.api_key.is_some(),
    })
    .to_string()
}

fn json_response(status: &str, body: &str, head_only: bool) -> String {
    response(status, "application/json; charset=utf-8", body, head_only)
}

fn response(status: &str, content_type: &str, body: &str, head_only: bool) -> String {
    let content_length = body.len();
    let body = if head_only { "" } else { body };
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {content_length}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{body}"
    )
}
