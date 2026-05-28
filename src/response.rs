use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireResponse {
    pub message_type: String,
    pub status_message: String,
    pub result_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AsrEvent {
    TaskStarted,
    SessionStarted,
    SessionFinished,
    VadStart,
    InterimResult(String),
    FinalResult(String),
    Heartbeat,
    Error(String),
}

pub fn classify_response(response: &WireResponse) -> AsrEvent {
    match response.message_type.as_str() {
        "TaskStarted" => return AsrEvent::TaskStarted,
        "SessionStarted" => return AsrEvent::SessionStarted,
        "SessionFinished" => return AsrEvent::SessionFinished,
        "TaskFailed" | "SessionFailed" => {
            return AsrEvent::Error(if response.status_message.is_empty() {
                response.message_type.clone()
            } else {
                response.status_message.clone()
            });
        }
        _ => {}
    }

    if response.result_json.is_empty() {
        return AsrEvent::Heartbeat;
    }

    let Ok(payload) = serde_json::from_str::<Value>(&response.result_json) else {
        return AsrEvent::Error("invalid result_json".to_string());
    };

    if payload
        .pointer("/extra/vad_start")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return AsrEvent::VadStart;
    }

    let Some(results) = payload.get("results").and_then(Value::as_array) else {
        return AsrEvent::Heartbeat;
    };
    if results.is_empty() {
        return AsrEvent::Heartbeat;
    }

    let text = results
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("");

    let is_final = results.iter().any(|item| {
        item.pointer("/extra/nonstream_result")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || (item
                .get("is_interim")
                .and_then(Value::as_bool)
                .map(|value| !value)
                .unwrap_or(false)
                && item
                    .get("is_vad_finished")
                    .and_then(Value::as_bool)
                    .unwrap_or(false))
    });

    if is_final {
        AsrEvent::FinalResult(text)
    } else {
        AsrEvent::InterimResult(text)
    }
}
