pub const INDEX_HTML: &str = include_str!("../web/index.html");

pub fn http_response(method: &str, target: &str) -> Option<String> {
    let path = target.split('?').next().unwrap_or(target);
    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => Some(response(
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML,
            false,
        )),
        ("HEAD", "/") | ("HEAD", "/index.html") => Some(response(
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML,
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
