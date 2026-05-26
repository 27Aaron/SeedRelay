use base64::{engine::general_purpose::STANDARD, Engine};
use seedrelay::realtime::{
    decode_client_event, transcript_completed_event, transcript_delta_event,
    validate_realtime_target, ClientEvent, SUPPORTED_MODEL,
};

#[test]
fn accepts_only_seed_asr_realtime_target() {
    validate_realtime_target("/v1/realtime?model=seed-asr-2.0").expect("seed target");

    assert!(validate_realtime_target("/v1/realtime?model=other-model").is_err());
    assert!(validate_realtime_target("/v1/audio/transcriptions?model=seed-asr-2.0").is_err());
}

#[test]
fn decodes_append_event_audio_payload() {
    let audio = STANDARD.encode([1u8, 2, 3, 4]);
    let event = decode_client_event(&format!(
        r#"{{"type":"input_audio_buffer.append","audio":"{audio}"}}"#
    ))
    .expect("append event");

    assert_eq!(event, ClientEvent::AppendAudio(vec![1, 2, 3, 4]));
}

#[test]
fn renders_openai_style_transcript_events() {
    let delta = transcript_delta_event("item-1", "你好");
    let completed = transcript_completed_event("item-1", "你好，世界");

    assert_eq!(
        delta["type"],
        "conversation.item.input_audio_transcription.delta"
    );
    assert_eq!(delta["item_id"], "item-1");
    assert_eq!(delta["content_index"], 0);
    assert_eq!(delta["delta"], "你好");
    assert_eq!(
        completed["type"],
        "conversation.item.input_audio_transcription.completed"
    );
    assert_eq!(completed["item_id"], "item-1");
    assert_eq!(completed["transcript"], "你好，世界");
}

#[test]
fn exposes_seed_asr_as_the_only_supported_model() {
    assert_eq!(SUPPORTED_MODEL, "seed-asr-2.0");
}
