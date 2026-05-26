use prost::Message;
use seedrelay::protocol::{build_start_task, build_task_request, AsrRequest, FrameState};

#[test]
fn start_task_message_contains_token_service_method_and_request_id() {
    let bytes = build_start_task("request-1", "token-1");
    let decoded = AsrRequest::decode(bytes.as_slice()).expect("decode start task");

    assert_eq!(decoded.token, "token-1");
    assert_eq!(decoded.service_name, "ASR");
    assert_eq!(decoded.method_name, "StartTask");
    assert_eq!(decoded.request_id, "request-1");
    assert_eq!(decoded.frame_state, FrameState::Unspecified as i32);
    assert!(decoded.payload.is_empty());
    assert!(decoded.audio_data.is_empty());
}

#[test]
fn task_request_message_carries_opus_audio_frame_state_and_timestamp_payload() {
    let bytes = build_task_request(
        "request-2",
        vec![1, 2, 3, 4],
        FrameState::First,
        1_770_000_000_123,
    );
    let decoded = AsrRequest::decode(bytes.as_slice()).expect("decode task request");
    let payload: serde_json::Value =
        serde_json::from_str(&decoded.payload).expect("task payload json");

    assert!(decoded.token.is_empty());
    assert_eq!(decoded.service_name, "ASR");
    assert_eq!(decoded.method_name, "TaskRequest");
    assert_eq!(decoded.request_id, "request-2");
    assert_eq!(decoded.audio_data, vec![1, 2, 3, 4]);
    assert_eq!(decoded.frame_state, FrameState::First as i32);
    assert_eq!(payload["extra"], serde_json::json!({}));
    assert_eq!(payload["timestamp_ms"], 1_770_000_000_123i64);
}
