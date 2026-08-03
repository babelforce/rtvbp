use rtvbp_spec_model::{
    ConstantField, ControlFrame, EnvelopeFixture, EnvelopeSpec, ErrorCodeSpec, ErrorSpec,
    FieldSpec, FrameKind, FrameSpec, WireError,
};
use serde_json::json;

macro_rules! envelope_bytes {
    ($name:literal) => {
        include_bytes!(concat!(
            "../../../../conformance/babelforce.v1/golden/envelope/classic.v1/",
            $name
        ))
        .as_slice()
    };
}

/// Build the frozen legacy flat JSON envelope and its byte-exact wire witnesses.
#[must_use]
pub fn envelope() -> EnvelopeSpec {
    let spec = EnvelopeSpec {
        id: "classic.v1".to_owned(),
        constants: vec![ConstantField {
            name: "version".to_owned(),
            value: "1".to_owned(),
        }],
        // Declaration order is structural discrimination precedence.
        frames: vec![
            FrameSpec {
                kind: FrameKind::Event,
                discriminator: FieldSpec::required("event"),
                id: Some(FieldSpec::required("id")),
                payload: FieldSpec::optional("data"),
                error: None,
            },
            FrameSpec {
                kind: FrameKind::Request,
                discriminator: FieldSpec::required("method"),
                id: Some(FieldSpec::required("id")),
                payload: FieldSpec::optional("params"),
                error: None,
            },
            FrameSpec {
                kind: FrameKind::Response,
                discriminator: FieldSpec::required("response"),
                id: None,
                payload: FieldSpec::optional("result"),
                error: Some(FieldSpec::optional("error")),
            },
        ],
        error: ErrorSpec {
            code: FieldSpec::required("code"),
            message: FieldSpec::required("message"),
            // The semantic error data field is named "any" on the frozen wire.
            data: FieldSpec::optional("any"),
        },
        error_codes: vec![
            ErrorCodeSpec {
                name: "unknown".to_owned(),
                code: -1,
                description: "Unclassified failure.".to_owned(),
            },
            ErrorCodeSpec {
                name: "bad_request".to_owned(),
                code: 400,
                description: "The request is invalid.".to_owned(),
            },
            ErrorCodeSpec {
                name: "internal_server_error".to_owned(),
                code: 500,
                description: "The handler failed internally.".to_owned(),
            },
            ErrorCodeSpec {
                name: "not_implemented".to_owned(),
                code: 501,
                description: "The requested operation is not implemented.".to_owned(),
            },
        ],
        fixtures: fixtures(),
    };
    spec.validate()
        .expect("authored classic.v1 envelope must be valid");
    spec
}

fn fixtures() -> Vec<EnvelopeFixture> {
    vec![
        fixture(
            "request.json",
            ControlFrame::Request {
                id: "request-1".into(),
                method: "session.get".into(),
                params: None,
            },
        ),
        fixture(
            "request-with-params.json",
            ControlFrame::Request {
                id: "request-terminate-1".into(),
                method: "session.terminate".into(),
                params: Some(json!({"reason": "completed"})),
            },
        ),
        fixture(
            "response-ok.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: Some(json!({})),
                error: None,
            },
        ),
        fixture(
            "response-ok-no-result.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: None,
                error: None,
            },
        ),
        fixture(
            "response-ok-null-result.json",
            ControlFrame::Response {
                correlation_id: "request-1".into(),
                result: Some(serde_json::Value::Null),
                error: None,
            },
        ),
        fixture(
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
        fixture(
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
        fixture(
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
        fixture(
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
        fixture(
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

fn fixture(name: &str, frame: ControlFrame) -> EnvelopeFixture {
    let bytes = match name {
        "request.json" => envelope_bytes!("request.json"),
        "request-with-params.json" => envelope_bytes!("request-with-params.json"),
        "response-ok.json" => envelope_bytes!("response-ok.json"),
        "response-ok-no-result.json" => envelope_bytes!("response-ok-no-result.json"),
        "response-ok-null-result.json" => envelope_bytes!("response-ok-null-result.json"),
        "response-error.json" => envelope_bytes!("response-error.json"),
        "response-error-unknown.json" => envelope_bytes!("response-error-unknown.json"),
        "response-error-internal.json" => envelope_bytes!("response-error-internal.json"),
        "response-error-not-implemented.json" => {
            envelope_bytes!("response-error-not-implemented.json")
        }
        "event.json" => envelope_bytes!("event.json"),
        _ => unreachable!("fixture name is declared above"),
    };
    EnvelopeFixture::new(format!("envelope/classic.v1/{name}"), bytes, frame)
}
