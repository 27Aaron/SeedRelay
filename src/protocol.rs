use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct AsrRequest {
    #[prost(string, tag = "2")]
    pub token: String,
    #[prost(string, tag = "3")]
    pub service_name: String,
    #[prost(string, tag = "5")]
    pub method_name: String,
    #[prost(string, tag = "6")]
    pub payload: String,
    #[prost(bytes, tag = "7")]
    pub audio_data: Vec<u8>,
    #[prost(string, tag = "8")]
    pub request_id: String,
    #[prost(enumeration = "FrameState", tag = "9")]
    pub frame_state: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct AsrResponse {
    #[prost(string, tag = "1")]
    pub request_id: String,
    #[prost(string, tag = "2")]
    pub task_id: String,
    #[prost(string, tag = "3")]
    pub service_name: String,
    #[prost(string, tag = "4")]
    pub message_type: String,
    #[prost(int32, tag = "5")]
    pub status_code: i32,
    #[prost(string, tag = "6")]
    pub status_message: String,
    #[prost(string, tag = "7")]
    pub result_json: String,
    #[prost(int32, tag = "9")]
    pub unknown_field_9: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum FrameState {
    Unspecified = 0,
    First = 1,
    Middle = 3,
    Last = 9,
}

fn encode_request(request: AsrRequest) -> Vec<u8> {
    request.encode_to_vec()
}

pub fn build_start_task(request_id: &str, token: &str) -> Vec<u8> {
    encode_request(AsrRequest {
        token: token.to_string(),
        service_name: "ASR".to_string(),
        method_name: "StartTask".to_string(),
        payload: String::new(),
        audio_data: Vec::new(),
        request_id: request_id.to_string(),
        frame_state: FrameState::Unspecified as i32,
    })
}

pub fn build_start_session(request_id: &str, token: &str, payload: String) -> Vec<u8> {
    encode_request(AsrRequest {
        token: token.to_string(),
        service_name: "ASR".to_string(),
        method_name: "StartSession".to_string(),
        payload,
        audio_data: Vec::new(),
        request_id: request_id.to_string(),
        frame_state: FrameState::Unspecified as i32,
    })
}

pub fn build_task_request(
    request_id: &str,
    audio_data: Vec<u8>,
    frame_state: FrameState,
    timestamp_ms: u64,
) -> Vec<u8> {
    let payload = serde_json::json!({
        "extra": {},
        "timestamp_ms": timestamp_ms,
    })
    .to_string();

    encode_request(AsrRequest {
        token: String::new(),
        service_name: "ASR".to_string(),
        method_name: "TaskRequest".to_string(),
        payload,
        audio_data,
        request_id: request_id.to_string(),
        frame_state: frame_state as i32,
    })
}

pub fn build_finish_session(request_id: &str, token: &str) -> Vec<u8> {
    encode_request(AsrRequest {
        token: token.to_string(),
        service_name: "ASR".to_string(),
        method_name: "FinishSession".to_string(),
        payload: String::new(),
        audio_data: Vec::new(),
        request_id: request_id.to_string(),
        frame_state: FrameState::Unspecified as i32,
    })
}

pub fn decode_response(data: &[u8]) -> Result<AsrResponse, prost::DecodeError> {
    AsrResponse::decode(data)
}
