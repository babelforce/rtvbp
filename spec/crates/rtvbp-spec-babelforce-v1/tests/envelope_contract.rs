use rtvbp_spec_babelforce_v1::envelope;
use rtvbp_spec_model::{ControlFrame, FrameKind, WireError};

#[test]
fn classic_v1_declaration_pins_the_frozen_grammar_and_fixtures() {
    let spec = envelope();
    spec.validate().unwrap();

    assert_eq!(spec.id, "classic.v1");
    assert_eq!(spec.constants.len(), 1);
    assert_eq!(spec.constants[0].name, "version");
    assert_eq!(spec.constants[0].value, "1");
    assert_eq!(
        spec.frames
            .iter()
            .map(|frame| frame.kind)
            .collect::<Vec<_>>(),
        [FrameKind::Event, FrameKind::Request, FrameKind::Response]
    );

    let event = &spec.frames[0];
    assert_eq!(event.discriminator.name, "event");
    assert_eq!(event.id.as_ref().unwrap().name, "id");
    assert_eq!(event.payload.name, "data");
    let request = &spec.frames[1];
    assert_eq!(request.discriminator.name, "method");
    assert_eq!(request.id.as_ref().unwrap().name, "id");
    assert_eq!(request.payload.name, "params");
    assert!(request.payload.omit_when_none);
    let response = &spec.frames[2];
    assert_eq!(response.discriminator.name, "response");
    assert!(response.id.is_none());
    assert_eq!(response.payload.name, "result");
    assert_eq!(response.error.as_ref().unwrap().name, "error");
    assert_eq!(spec.error.data.name, "any");
    assert_eq!(
        spec.error_codes
            .iter()
            .map(|error| (error.name.as_str(), error.code))
            .collect::<Vec<_>>(),
        [
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

    assert_eq!(spec.fixtures.len(), 10);
    assert!(
        spec.fixtures
            .iter()
            .all(|fixture| fixture.path.starts_with("envelope/classic.v1/"))
    );
}

#[test]
fn event_wins_the_frozen_structural_precedence() {
    let spec = envelope();
    let ambiguous = br#"{"version":"1","id":"event-1","event":"dtmf","method":"ping","response":"request-1","data":{}}"#;

    assert!(matches!(
        spec.decode(ambiguous).unwrap(),
        ControlFrame::Event { event, .. } if event == "dtmf"
    ));
    assert!(spec.decode(b"not json").is_err());
    assert!(
        spec.decode(br#"{"version":"2","id":"x","method":"ping"}"#)
            .is_err()
    );
    assert!(spec.decode(br#"{"version":"1","method":"ping"}"#).is_err());
    assert!(spec.decode(br#"{"version":"1"}"#).is_err());
}

#[test]
fn classic_v1_keeps_deployed_response_permissiveness() {
    let spec = envelope();
    let both = br#"{"version":"1","response":"request-1","result":{},"error":{"code":500,"message":"failed"}}"#;
    let neither = br#"{"version":"1","response":"request-1"}"#;

    assert_eq!(spec.encode(&spec.decode(both).unwrap()).unwrap(), both);
    assert_eq!(
        spec.encode(&spec.decode(neither).unwrap()).unwrap(),
        neither
    );
    assert!(matches!(
        spec.decode(
            br#"{"version":"1","response":"request-1","error":{"code":777,"message":"extension"}}"#
        )
        .unwrap(),
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
