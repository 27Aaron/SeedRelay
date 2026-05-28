const DOCKERFILE: &str = include_str!("../Dockerfile");
const COMPOSE_YML: &str = include_str!("../compose.yml");

#[test]
fn docker_image_listens_on_all_interfaces_by_default() {
    assert!(DOCKERFILE.contains(r#""--host", "0.0.0.0""#));
    assert!(!DOCKERFILE.contains("--api-key"));
    assert!(!DOCKERFILE.contains("your-secret-key"));
}

#[test]
fn compose_exposes_public_bind_without_persisting_generated_credentials() {
    assert!(COMPOSE_YML.contains(r#"- "0.0.0.0""#));
    assert!(COMPOSE_YML.contains(r#"- "your-secret-key""#));
    assert!(!COMPOSE_YML.contains("SEEDRELAY_API_KEY"));
    assert!(!COMPOSE_YML.contains("seedrelay-data"));
    assert!(!COMPOSE_YML.contains("/app/.seedrelay"));
}
