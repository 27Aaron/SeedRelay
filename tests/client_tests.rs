use seedrelay::client::format_lifecycle_error;
use seedrelay::protocol::AsrResponse;

#[test]
fn lifecycle_error_includes_message_type_status_code_and_backend_message() {
    let response = AsrResponse {
        request_id: "req-1".to_string(),
        task_id: String::new(),
        service_name: "ASR".to_string(),
        message_type: "TaskFailed".to_string(),
        status_code: 2,
        status_message:
            "read backend response: rpc error: code = 2 desc = service discovery failure"
                .to_string(),
        result_json: String::new(),
        unknown_field_9: 0,
    };

    let message = format_lifecycle_error("TaskStarted", &response);

    assert!(message.contains("expected TaskStarted"));
    assert!(message.contains("got TaskFailed"));
    assert!(message.contains("status_code=2"));
    assert!(message.contains("service discovery failure"));
}
