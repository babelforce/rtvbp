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

fn cases() -> Vec<(&'static str, ControlFrame)> {
    vec![
        (
            "request.json",
            ControlFrame::Request {
                id: "request-1".into(),
                method: "session.get".into(),
                params: None,
            },
        ),
        (
            "request-with-params.json",
            ControlFrame::Request {
                id: "request-terminate-1".into(),
                method: "session.terminate".into(),
                params: Some(json!({"reason": "completed"})),
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
            "response-ok-no-result.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: None,
                error: None,
            },
        ),
        (
            "response-ok-null-result.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: Some(serde_json::Value::Null),
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
            "response-error-unknown.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: None,
                error: Some(WireError {
                    code: -1,
                    message: "unknown failure".into(),
                    data: None,
                }),
            },
        ),
        (
            "response-error-internal.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: None,
                error: Some(WireError {
                    code: 500,
                    message: "internal failure".into(),
                    data: None,
                }),
            },
        ),
        (
            "response-error-not-implemented.json",
            ControlFrame::Response {
                correlation_id: "request-terminate-1".into(),
                result: None,
                error: Some(WireError {
                    code: 501,
                    message: "session.terminate is not supported. please use application.move or call.hangup instead".into(),
                    data: None,
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
    assert_eq!(
        spec.error_codes
            .iter()
            .map(|error| (error.name.as_str(), error.code))
            .collect::<Vec<_>>(),
        vec![
            ("unknown", -1),
            ("bad_request", 400),
            ("internal_server_error", 500),
            ("not_implemented", 501),
        ]
    );
    assert!(
        spec.error_codes
            .iter()
            .all(|error| !error.description.is_empty())
    );
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

#[test]
fn classic_v1_matches_deployed_response_permissiveness_and_error_validation() {
    let spec = classic_v1();
    let both = br#"{"version":"1","response":"request-1","result":{},"error":{"code":500,"message":"failed"}}"#;
    let neither = br#"{"version":"1","response":"request-1"}"#;

    assert_eq!(spec.encode(&spec.decode(both).unwrap()).unwrap(), both);
    assert_eq!(
        spec.encode(&spec.decode(neither).unwrap()).unwrap(),
        neither
    );

    let unknown =
        br#"{"version":"1","response":"request-1","error":{"code":777,"message":"extension"}}"#;
    assert!(matches!(
        spec.decode(unknown).unwrap(),
        ControlFrame::Response {
            error: Some(WireError { code: 777, .. }),
            ..
        }
    ));

    assert!(
        spec.decode(
            br#"{"version":"1","response":"request-1","error":{"code":0,"message":"unset"}}"#
        )
        .is_err()
    );
    assert!(
        spec.decode(br#"{"version":"1","response":"request-1","error":{"code":500,"message":""}}"#)
            .is_err()
    );

    for error in [
        WireError {
            code: 0,
            message: "unset".into(),
            data: None,
        },
        WireError {
            code: 500,
            message: String::new(),
            data: None,
        },
    ] {
        assert!(
            spec.encode(&ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: None,
                error: Some(error),
            })
            .is_err()
        );
    }

    let error_null = br#"{"version":"1","response":"request-1","error":null}"#;
    assert_eq!(
        spec.encode(&spec.decode(error_null).unwrap()).unwrap(),
        neither
    );

    let data_null = br#"{"version":"1","response":"request-1","error":{"code":500,"message":"failed","any":null}}"#;
    assert_eq!(
        spec.encode(&spec.decode(data_null).unwrap()).unwrap(),
        data_null
    );
}
