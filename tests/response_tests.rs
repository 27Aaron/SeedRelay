use seedrelay::response::{classify_response, AsrEvent, WireResponse};

#[test]
fn classifies_final_result_from_vad_finished_payload() {
    let response = WireResponse {
        message_type: "".to_string(),
        status_message: "".to_string(),
        result_json: r#"{"results":[{"text":"你好","is_interim":false,"is_vad_finished":true}]}"#
            .to_string(),
    };

    assert_eq!(
        classify_response(&response),
        AsrEvent::FinalResult("你好".to_string())
    );
}

#[test]
fn classifies_empty_result_json_as_heartbeat() {
    let response = WireResponse {
        message_type: "".to_string(),
        status_message: "".to_string(),
        result_json: "".to_string(),
    };

    assert_eq!(classify_response(&response), AsrEvent::Heartbeat);
}
