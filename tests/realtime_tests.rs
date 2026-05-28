use base64::{engine::general_purpose::STANDARD, Engine};
use seedrelay::config::DEFAULT_MODEL;
use seedrelay::realtime::{
    decode_client_event, error_event, model_list_response, model_not_found_error,
    model_object_response, session_created_event, session_updated_event,
    transcript_completed_event, transcript_delta_event, validate_realtime_target, ClientEvent,
    RealtimeSession, SessionUpdateConfig,
};

#[test]
fn accepts_only_configured_realtime_model_target() {
    validate_realtime_target("/v1/realtime?model=seed-asr", DEFAULT_MODEL).expect("seed target");
    validate_realtime_target("/v1/realtime?model=custom-asr", "custom-asr").expect("custom target");
    validate_realtime_target("/v1/realtime?model=custom%2Fasr", "custom/asr")
        .expect("encoded target");

    assert!(validate_realtime_target("/v1/realtime?model=other-model", DEFAULT_MODEL).is_err());
    assert!(
        validate_realtime_target("/v1/audio/transcriptions?model=seed-asr", DEFAULT_MODEL).is_err()
    );
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
fn decodes_nested_session_update_fields() {
    let event = decode_client_event(
        r#"{
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": { "type": "audio/pcm", "rate": 16000 },
                        "transcription": {
                            "model": "seed-asr",
                            "language": "zh"
                        },
                        "turn_detection": null
                    }
                },
                "include": []
            }
        }"#,
    )
    .expect("session update");

    assert_eq!(
        event,
        ClientEvent::SessionUpdate(SessionUpdateConfig {
            input_sample_rate: Some(16_000),
            input_audio_format_type: Some("audio/pcm".into()),
            transcription_model: Some("seed-asr".into()),
            language: Some("zh".into()),
            delay: None,
            turn_detection_disabled: true,
            include: Vec::new(),
        })
    );
}

#[test]
fn rejects_unsupported_session_update_type() {
    let error = decode_client_event(r#"{"type":"session.update","session":{"type":"realtime"}}"#)
        .expect_err("unsupported session type");

    assert!(error
        .to_string()
        .contains("only transcription sessions are supported"));
}

#[test]
fn rejects_non_object_session_update_session() {
    for raw in [
        r#"{"type":"session.update","session":null}"#,
        r#"{"type":"session.update","session":[]}"#,
    ] {
        let error = decode_client_event(raw).expect_err("non-object session");

        assert!(error.to_string().contains("session must be an object"));
    }
}

#[test]
fn rejects_non_string_session_update_type() {
    let error = decode_client_event(r#"{"type":"session.update","session":{"type":123}}"#)
        .expect_err("unsupported session type");

    assert!(error
        .to_string()
        .contains("only transcription sessions are supported"));
}

#[test]
fn rejects_malformed_session_update_include() {
    let error = decode_client_event(
        r#"{"type":"session.update","session":{"type":"transcription","include":123}}"#,
    )
    .expect_err("non-array include");

    assert!(error
        .to_string()
        .contains("session.include must be an array"));

    let error = decode_client_event(
        r#"{"type":"session.update","session":{"type":"transcription","include":[123]}}"#,
    )
    .expect_err("non-string include entry");

    assert!(error
        .to_string()
        .contains("session.include entries must be strings"));
}

#[test]
fn rejects_malformed_session_update_supported_fields() {
    for (raw, expected) in [
        (
            r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"format":{"rate":"16000"}}}}}"#,
            "rate",
        ),
        (
            r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"format":{"rate":0}}}}}"#,
            "rate",
        ),
        (
            r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"format":{"type":123}}}}}"#,
            "format.type",
        ),
        (
            r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"transcription":{"model":123}}}}}"#,
            "model",
        ),
        (
            r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"transcription":{"language":123}}}}}"#,
            "language",
        ),
        (
            r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"transcription":{"delay":{}}}}}}"#,
            "delay",
        ),
    ] {
        let error = decode_client_event(raw).expect_err("malformed supported field");

        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {error}"
        );
    }
}

#[test]
fn rejects_non_null_session_update_turn_detection() {
    let error = decode_client_event(
        r#"{"type":"session.update","session":{"type":"transcription","audio":{"input":{"turn_detection":{"type":"server_vad"}}}}}"#,
    )
    .expect_err("unsupported turn_detection");

    assert!(error.to_string().contains("turn_detection"));
}

#[test]
fn rejected_session_update_does_not_mutate_session() {
    let mut session = RealtimeSession::with_id("sess_test", "seed-asr");
    session.language = Some("en".into());
    let original = session.clone();
    let update = SessionUpdateConfig {
        input_sample_rate: Some(16_000),
        input_audio_format_type: Some("audio/pcm".into()),
        transcription_model: Some("seed-asr".into()),
        language: Some("zh".into()),
        delay: None,
        turn_detection_disabled: false,
        include: vec!["item.input_audio_transcription.logprobs".into()],
    };

    assert!(update.apply_to(&mut session, "seed-asr").is_err());
    assert_eq!(session, original);
}

#[test]
fn decodes_clear_event() {
    let event = decode_client_event(r#"{"type":"input_audio_buffer.clear"}"#).expect("clear event");

    assert_eq!(event, ClientEvent::Clear);
}

#[test]
fn renders_openai_style_transcript_events() {
    let session = RealtimeSession::with_id("sess_test", "custom-asr");
    let updated = session_updated_event(&session);
    let delta = transcript_delta_event("item-1", "你好");
    let completed = transcript_completed_event("item-1", "你好，世界");

    assert_eq!(updated["type"], "session.updated");
    assert_eq!(updated["session"]["id"], "sess_test");
    assert_eq!(updated["session"]["model"], "custom-asr");
    assert_eq!(
        updated["session"]["audio"]["input"]["transcription"]["model"],
        "custom-asr"
    );
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
fn defaults_public_model_to_seed_asr() {
    assert_eq!(DEFAULT_MODEL, "seed-asr");
}

#[test]
fn renders_openai_style_model_list_response() {
    let response = model_list_response("seed-asr");

    assert_eq!(response["object"], "list");
    assert_eq!(response["data"][0]["id"], "seed-asr");
    assert_eq!(response["data"][0]["object"], "model");
    assert_eq!(response["data"][0]["created"], 0);
    assert_eq!(response["data"][0]["owned_by"], "seedrelay");
}

#[test]
fn renders_openai_style_model_object_response() {
    let response = model_object_response("custom-asr");

    assert_eq!(response["id"], "custom-asr");
    assert_eq!(response["object"], "model");
    assert_eq!(response["created"], 0);
    assert_eq!(response["owned_by"], "seedrelay");
}

#[test]
fn renders_model_not_found_error_response() {
    let response = model_not_found_error("missing-asr");

    assert_eq!(response["error"]["type"], "invalid_request_error");
    assert_eq!(response["error"]["param"], "model");
    assert_eq!(response["error"]["code"], "model_not_found");
    assert!(response["error"]["message"]
        .as_str()
        .expect("message")
        .contains("missing-asr"));
}

#[test]
fn renders_session_created_event_with_session_id() {
    let session = RealtimeSession::with_id("sess_test", "seed-asr");
    let event = session_created_event(&session);

    assert_eq!(event["type"], "session.created");
    assert!(event["event_id"]
        .as_str()
        .expect("event id")
        .starts_with("event_"));
    assert_eq!(event["session"]["id"], "sess_test");
    assert_eq!(event["session"]["type"], "transcription");
    assert_eq!(event["session"]["model"], "seed-asr");
    assert_eq!(
        event["session"]["audio"]["input"]["format"]["type"],
        "audio/pcm"
    );
    assert_eq!(event["session"]["audio"]["input"]["format"]["rate"], 24_000);
}

#[test]
fn renders_error_events_with_openai_style_fields() {
    let event = error_event("bad audio");

    assert_eq!(event["type"], "error");
    assert!(event["event_id"]
        .as_str()
        .expect("event id")
        .starts_with("event_"));
    assert_eq!(event["error"]["type"], "invalid_request_error");
    assert_eq!(event["error"]["code"], "invalid_request");
    assert_eq!(event["error"]["message"], "bad audio");
}
