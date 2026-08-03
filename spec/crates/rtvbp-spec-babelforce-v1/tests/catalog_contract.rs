use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use rtvbp_spec_babelforce_v1::{
    AppInfo, ApplicationMoveRequest, AudioBufferClearRequest, AudioCodec, AudioInfoEvent,
    AudioSpeechStartedEvent, CallHangupEvent, CallInfo, DtmfEvent, OutputTranscriptDoneEvent,
    RecordingStartRequest, SessionInitializeRequest, SessionInitializeResponse,
    SessionUpdatedEvent, catalog,
};
use rtvbp_spec_model::{Nullable, Role};
use schemars::schema_for;
use serde_json::{Value, json};

fn golden(relative: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../conformance/babelforce.v1/golden")
        .join(relative);
    fs::read(path).unwrap()
}

#[test]
fn catalog_declares_every_operation_role_terminal_flag_doc_and_example() {
    let catalog = catalog();
    let operations = catalog
        .operations
        .iter()
        .map(|operation| {
            (
                operation.method.as_str(),
                (operation.handled_by, operation.terminal),
            )
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        operations,
        BTreeMap::from([
            ("application.move", (Role::Voice, true)),
            ("audio.buffer.clear", (Role::Voice, false)),
            ("call.hangup", (Role::Voice, true)),
            ("ping", (Role::Both, false)),
            ("recording.start", (Role::Voice, false)),
            ("recording.stop", (Role::Voice, false)),
            ("session.get", (Role::Voice, false)),
            ("session.initialize", (Role::Application, false)),
            ("session.set", (Role::Voice, false)),
            ("session.terminate", (Role::Application, true)),
        ])
    );
    assert!(catalog.operations.iter().all(|operation| {
        operation
            .docs
            .as_ref()
            .is_some_and(|docs| !docs.trim().is_empty())
            && !operation.examples.is_empty()
    }));
}

#[test]
fn catalog_declares_every_event_role_doc_and_example() {
    let catalog = catalog();
    let events = catalog
        .events
        .iter()
        .map(|event| (event.name.as_str(), event.emitted_by))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        events,
        BTreeMap::from([
            ("agent.tool.call", Role::Application),
            ("audio.info", Role::Voice),
            ("audio.speech.started", Role::Application),
            ("call.hangup", Role::Voice),
            ("dtmf", Role::Voice),
            ("input.transcript", Role::Application),
            ("output.transcript.delta", Role::Application),
            ("output.transcript.done", Role::Application),
            ("session.updated", Role::Voice),
        ])
    );
    assert!(catalog.events.iter().all(|event| {
        event
            .docs
            .as_ref()
            .is_some_and(|docs| !docs.trim().is_empty())
            && !event.examples.is_empty()
    }));
}

#[test]
fn catalog_and_every_typed_example_validate() {
    catalog().validate().unwrap();
}

#[test]
fn browser_event_examples_pin_the_additive_shapes() {
    let catalog = catalog();
    let examples = catalog
        .events
        .iter()
        .map(|event| (event.name.as_str(), event.examples[0].data.clone()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(examples["output.transcript.delta"], json!({"delta": "Hi "}));
    assert_eq!(examples["output.transcript.done"], json!({}));
    assert_eq!(
        examples["input.transcript"],
        json!({"delta": "hello there"})
    );
    assert_eq!(examples["agent.tool.call"], json!({"name": "lookup_order"}));
}

#[test]
fn canonical_deployed_examples_preserve_frozen_field_order_and_bytes() {
    let catalog = catalog();
    for operation in &catalog.operations {
        let example = &operation.examples[0];
        let base = format!("payloads/{}", operation.method);
        assert_eq!(
            serde_json::to_vec(&example.request).unwrap(),
            golden(&format!("{base}.request.json")),
            "{} request",
            operation.method
        );
        assert_eq!(
            serde_json::to_vec(&example.response).unwrap(),
            golden(&format!("{base}.response.json")),
            "{} response",
            operation.method
        );
    }

    for event in catalog.events.iter().filter(|event| {
        matches!(
            event.name.as_str(),
            "session.updated" | "dtmf" | "call.hangup" | "audio.info" | "audio.speech.started"
        )
    }) {
        assert_eq!(
            serde_json::to_vec(&event.examples[0].data).unwrap(),
            golden(&format!("events/{}.json", event.name)),
            "{} event",
            event.name
        );
    }
}

#[test]
fn presence_and_go_type_hints_match_the_legacy_contract() {
    let codec = AudioCodec {
        id: "L16/8000/1".into(),
        name: "L16".into(),
        sample_rate: 8_000,
        bit_depth: 16,
        channels: 1,
    };
    let request = SessionInitializeRequest {
        application: AppInfo { id: "app-1".into() },
        call: CallInfo {
            id: "call-1".into(),
            session_id: "session-1".into(),
            from: "+12025550100".into(),
            to: "+12025550101".into(),
        },
        audio_codec_offerings: vec![codec],
        metadata: Nullable::none(),
    };
    assert_eq!(
        serde_json::to_vec(&request).unwrap(),
        golden("payloads/session.initialize.request.json")
    );
    assert_eq!(
        serde_json::to_value(SessionInitializeResponse {
            audio_codec: Nullable::none()
        })
        .unwrap(),
        json!({"audio_codec": null})
    );
    assert_eq!(
        serde_json::to_value(SessionUpdatedEvent {
            audio_codec: Nullable::none()
        })
        .unwrap(),
        json!({"audio_codec": null})
    );

    assert_eq!(
        serde_json::to_value(ApplicationMoveRequest::default()).unwrap(),
        json!({})
    );
    assert_eq!(
        serde_json::to_value(CallHangupEvent::default()).unwrap(),
        json!({})
    );
    assert_eq!(
        serde_json::to_value(RecordingStartRequest::default()).unwrap(),
        json!({})
    );
    assert_eq!(
        serde_json::to_value(OutputTranscriptDoneEvent::default()).unwrap(),
        json!({})
    );
    assert_eq!(
        serde_json::to_value(AudioBufferClearRequest {}).unwrap(),
        json!({})
    );

    let hints = [
        (
            schema_for!(AudioCodec),
            vec!["sample_rate", "bit_depth", "channels"],
        ),
        (schema_for!(DtmfEvent), vec!["seq"]),
        (
            schema_for!(rtvbp_spec_babelforce_v1::AudioBufferClearResponse),
            vec!["len"],
        ),
    ];
    for (schema, fields) in hints {
        let schema: Value = schema.to_value();
        for field in fields {
            assert_eq!(schema["properties"][field]["x-go-type"], "int", "{field}");
        }
    }

    let audio_info: Value = schema_for!(AudioInfoEvent).to_value();
    assert_eq!(
        audio_info["properties"]["read"]["$ref"],
        "#/$defs/AudioInfoItem"
    );
    let speech: Value = schema_for!(AudioSpeechStartedEvent).to_value();
    assert!(
        speech["required"]
            .as_array()
            .unwrap()
            .contains(&json!("origin"))
    );
}
