pub const INDEX_HTML: &str = include_str!("../web/index.html");
pub const STYLES_CSS: &str = include_str!("../web/styles.css");
pub const APP_JS: &str = include_str!("../web/app.js");

pub fn http_response(method: &str, target: &str) -> Option<String> {
    let path = target.split('?').next().unwrap_or(target);
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
        ("GET", "/app.js") => Some(response(
            "200 OK",
            "application/javascript; charset=utf-8",
            APP_JS,
            false,
        )),
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
        ("HEAD", "/app.js") => Some(response(
            "200 OK",
            "application/javascript; charset=utf-8",
            APP_JS,
            true,
        )),
        ("GET", "/health") => Some(response(
            "200 OK",
            "application/json; charset=utf-8",
            r#"{"ok":true}"#,
            false,
        )),
        _ => Some(response(
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found",
            false,
        )),
    }
}

fn response(status: &str, content_type: &str, body: &str, head_only: bool) -> String {
    let content_length = body.len();
    let body = if head_only { "" } else { body };
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {content_length}\r\ncache-control: no-store\r\nconnection: close\r\n\r\n{body}"
    )
}
