use std::fs;
use std::path::PathBuf;

use rtvbp_spec_model::{ControlFrame, FrameKind, WireError, classic_v1};
use serde_json::json;

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../conformance/babelforce.v1/golden/envelope/classic.v1")
        .join(name);
    fs::read(path).unwrap()
}

fn cases() -> [(&'static str, ControlFrame); 4] {
    [
        (
            "request.json",
            ControlFrame::Request {
                id: "request-1".into(),
                method: "session.get".into(),
                params: None,
            },
        ),
        (
            "response-ok.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: Some(json!({})),
                error: None,
            },
        ),
        (
            "response-error.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: None,
                error: Some(WireError {
                    code: 400,
                    message: "invalid request".into(),
                    data: Some(json!({"field": "reason", "retryable": false})),
                }),
            },
        ),
        (
            "event.json",
            ControlFrame::Event {
                id: "event-1".into(),
                event: "dtmf".into(),
                data: Some(json!({
                    "seq": 7,
                    "pressed_at": 1_700_000_000_000_i64,
                    "released_at": 1_700_000_000_120_i64,
                    "digit": "5"
                })),
            },
        ),
    ]
}

#[test]
fn classic_v1_spec_pins_the_wire_description() {
    let spec = classic_v1();

    assert_eq!(spec.id, "classic.v1");
    assert_eq!(spec.constants[0].name, "version");
    assert_eq!(spec.constants[0].value, "1");
    assert_eq!(
        spec.frames
            .iter()
            .map(|frame| frame.kind)
            .collect::<Vec<_>>(),
        vec![FrameKind::Event, FrameKind::Request, FrameKind::Response]
    );
    assert_eq!(spec.error.data.name, "any");
}

#[test]
fn classic_v1_encodes_and_decodes_the_frozen_envelopes_byte_exactly() {
    let spec = classic_v1();

    for (name, frame) in cases() {
        let want = golden(name);
        assert_eq!(spec.encode(&frame).unwrap(), want, "encode {name}");
        assert_eq!(spec.decode(&want).unwrap(), frame, "decode {name}");
    }
}

#[test]
fn classic_v1_uses_structural_precedence_and_rejects_malformed_frames() {
    let spec = classic_v1();
    let ambiguous = br#"{"version":"1","id":"event-1","event":"dtmf","method":"ping","data":{}}"#;

    assert!(matches!(
        spec.decode(ambiguous).unwrap(),
        ControlFrame::Event { event, .. } if event == "dtmf"
    ));
    assert!(
        spec.decode(br#"{"version":"2","id":"x","method":"ping"}"#)
            .is_err()
    );
    assert!(spec.decode(br#"{"version":"1","method":"ping"}"#).is_err());
    assert!(spec.decode(br#"{"version":"1"}"#).is_err());
    assert!(spec.decode(b"not json").is_err());
}
