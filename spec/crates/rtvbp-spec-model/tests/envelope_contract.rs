use rtvbp_spec_model::{
    ConstantField, ControlFrame, EnvelopeFixture, EnvelopeSpec, ErrorSpec, FieldSpec, FrameKind,
    FrameSpec,
};
use serde_json::json;

fn synthetic() -> EnvelopeSpec {
    EnvelopeSpec {
        id: "synthetic.v2".into(),
        constants: vec![ConstantField {
            name: "protocol".into(),
            value: "two".into(),
        }],
        frames: vec![
            FrameSpec {
                kind: FrameKind::Request,
                discriminator: FieldSpec::required("command"),
                id: Some(FieldSpec::required("request_id")),
                payload: FieldSpec::required("arguments"),
                error: None,
            },
            FrameSpec {
                kind: FrameKind::Event,
                discriminator: FieldSpec::required("notification"),
                id: Some(FieldSpec::required("notification_id")),
                payload: FieldSpec::optional("body"),
                error: None,
            },
            FrameSpec {
                kind: FrameKind::Response,
                discriminator: FieldSpec::required("reply_to"),
                id: None,
                payload: FieldSpec::optional("value"),
                error: Some(FieldSpec::optional("failure")),
            },
        ],
        error: ErrorSpec {
            code: FieldSpec::required("status"),
            message: FieldSpec::required("detail"),
            data: FieldSpec::optional("metadata"),
        },
        error_codes: Vec::new(),
        fixtures: Vec::new(),
    }
}

#[test]
fn a_second_envelope_is_entirely_data_driven() {
    let spec = synthetic();
    spec.validate().unwrap();
    let frame = ControlFrame::Request {
        id: "r-1".into(),
        method: "demo.run".into(),
        params: Some(json!({"input": "hello"})),
    };
    let bytes = br#"{"protocol":"two","request_id":"r-1","command":"demo.run","arguments":{"input":"hello"}}"#;

    assert_eq!(spec.encode(&frame).unwrap(), bytes);
    assert_eq!(spec.decode(bytes).unwrap(), frame);
}

#[test]
fn validation_rejects_missing_kinds_and_duplicate_discriminators() {
    let mut spec = synthetic();
    spec.frames.retain(|frame| frame.kind != FrameKind::Event);
    spec.frames[1].discriminator.name = "command".into();

    let error = spec.validate().unwrap_err().to_string();
    assert!(error.contains("no Event frame"), "{error}");
    assert!(
        error.contains("duplicate structural discriminator"),
        "{error}"
    );
    assert!(
        spec.encode(&ControlFrame::Response {
            correlation_id: "r-1".into(),
            result: None,
            error: None,
        })
        .is_err()
    );
}

#[test]
fn validation_proves_fixture_bytes_and_semantics() {
    let mut spec = synthetic();
    let frame = ControlFrame::Event {
        id: "e-1".into(),
        event: "ready".into(),
        data: None,
    };
    spec.fixtures.push(EnvelopeFixture::new(
        "synthetic/event.json",
        br#"{"protocol":"two","notification_id":"e-1","notification":"ready"}"#,
        frame,
    ));
    spec.validate().unwrap();

    spec.fixtures[0].bytes.push(b' ');
    let error = spec.validate().unwrap_err().to_string();
    assert!(error.contains("changed bytes"), "{error}");
}

#[test]
fn a_required_response_error_is_null_in_every_codec_projection() {
    let mut spec = synthetic();
    let response = spec
        .frames
        .iter_mut()
        .find(|frame| frame.kind == FrameKind::Response)
        .unwrap();
    response.error = Some(FieldSpec::required("failure"));
    let frame = ControlFrame::Response {
        correlation_id: "r-1".into(),
        result: None,
        error: None,
    };
    let bytes = br#"{"protocol":"two","reply_to":"r-1","failure":null}"#;

    spec.validate().unwrap();
    assert_eq!(spec.encode(&frame).unwrap(), bytes);
    assert_eq!(spec.decode(bytes).unwrap(), frame);
}
