use std::fmt::Write as _;
use std::path::PathBuf;

use rtvbp_spec_model::{ControlFrame, EnvelopeSpec, FrameKind};

use super::{RUST_BANNER, RustEmitError};
use crate::emit::GeneratedFile;

pub fn emit_rust_envelope(envelope: &EnvelopeSpec) -> Result<Vec<GeneratedFile>, RustEmitError> {
    let module = module_name(&envelope.id)?;
    Ok(vec![
        GeneratedFile {
            path: PathBuf::from("src")
                .join("envelope")
                .join(&module)
                .join("zz_generated_codec.rs"),
            bytes: render_codec(envelope).into_bytes(),
        },
        GeneratedFile {
            path: PathBuf::from("src")
                .join("envelope")
                .join(module)
                .join("zz_generated_golden_tests.rs"),
            bytes: render_tests(envelope)?.into_bytes(),
        },
    ])
}

fn render_codec(envelope: &EnvelopeSpec) -> String {
    let constants = envelope
        .constants
        .iter()
        .map(|constant| format!("    ({:?}, {:?}),", constant.name, constant.value))
        .collect::<Vec<_>>()
        .join("\n");
    let frames = envelope
        .frames
        .iter()
        .map(|frame| {
            format!(
                "    FrameSpec {{ kind: {}, discriminator: {}, id: {}, payload: {}, error: {} }},",
                rust_kind(frame.kind),
                field(&frame.discriminator),
                frame
                    .id
                    .as_ref()
                    .map(|field_spec| format!("Some({})", field(field_spec)))
                    .unwrap_or_else(|| "None".to_owned()),
                field(&frame.payload),
                frame
                    .error
                    .as_ref()
                    .map(|field_spec| format!("Some({})", field(field_spec)))
                    .unwrap_or_else(|| "None".to_owned()),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let error = format!(
        "ErrorSpec {{ code: {}, message: {}, data: {} }}",
        field(&envelope.error.code),
        field(&envelope.error.message),
        field(&envelope.error.data)
    );

    let mut output = String::from(RUST_BANNER);
    writeln!(output, "// Generated `{}` envelope codec.\n", envelope.id).unwrap();
    output.push_str("use serde::ser::{SerializeMap, Serializer};\nuse serde_json::Value;\n\n");
    output.push_str(
        "#[derive(Clone, Copy)]\nstruct FieldSpec {\n    name: &'static str,\n    omit_when_none: bool,\n}\n\n#[derive(Clone, Copy)]\nstruct FrameSpec {\n    kind: crate::FrameKind,\n    discriminator: FieldSpec,\n    id: Option<FieldSpec>,\n    payload: FieldSpec,\n    error: Option<FieldSpec>,\n}\n\n#[derive(Clone, Copy)]\nstruct ErrorSpec {\n    code: FieldSpec,\n    message: FieldSpec,\n    data: FieldSpec,\n}\n\n",
    );
    writeln!(output, "const ENVELOPE_NAME: &str = {:?};", envelope.id).unwrap();
    writeln!(
        output,
        "const CONSTANTS: &[(&str, &str)] = &[\n{constants}\n];"
    )
    .unwrap();
    writeln!(output, "const FRAMES: &[FrameSpec] = &[\n{frames}\n];").unwrap();
    writeln!(output, "const ERROR: ErrorSpec = {error};\n").unwrap();
    output.push_str(CODEC_IMPL);
    super::finish(output)
}

const CODEC_IMPL: &str = r#"/// The generated envelope implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Envelope;

impl crate::Envelope for Envelope {
    fn name(&self) -> &'static str { ENVELOPE_NAME }

    fn encode(&self, frame: &crate::ControlFrame) -> Result<Vec<u8>, crate::Error> {
        validate_frame(frame)?;
        let mut bytes = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut bytes);
        let mut object = serializer
            .serialize_map(None)
            .map_err(crate::Error::envelope)?;
        for (name, value) in CONSTANTS {
            object.serialize_entry(name, value).map_err(crate::Error::envelope)?;
        }
        let shape = frame_spec(frame.kind)?;
        match frame.kind {
            crate::FrameKind::Request | crate::FrameKind::Event => {
                let id = shape.id.ok_or_else(|| crate::Error::envelope_message("frame id mapping is missing"))?;
                object.serialize_entry(id.name, &frame.id).map_err(crate::Error::envelope)?;
                object.serialize_entry(shape.discriminator.name, &frame.method).map_err(crate::Error::envelope)?;
                serialize_optional(&mut object, shape.payload, frame.payload.as_ref())?;
            }
            crate::FrameKind::Response => {
                object.serialize_entry(shape.discriminator.name, &frame.correlation_id).map_err(crate::Error::envelope)?;
                serialize_optional(&mut object, shape.payload, frame.payload.as_ref())?;
                let error_field = shape.error.ok_or_else(|| crate::Error::envelope_message("response error mapping is missing"))?;
                match frame.error.as_ref() {
                    Some(error) => {
                        validate_wire_error(error)?;
                        let mut encoded = serde_json::Map::new();
                        encoded.insert(ERROR.code.name.to_owned(), Value::from(error.code));
                        encoded.insert(ERROR.message.name.to_owned(), Value::from(error.message.clone()));
                        if let Some(data) = error.data.as_ref() {
                            encoded.insert(ERROR.data.name.to_owned(), data.clone());
                        } else if !ERROR.data.omit_when_none {
                            encoded.insert(ERROR.data.name.to_owned(), Value::Null);
                        }
                        object.serialize_entry(error_field.name, &encoded).map_err(crate::Error::envelope)?;
                    }
                    None if !error_field.omit_when_none => {
                        object.serialize_entry(error_field.name, &Value::Null).map_err(crate::Error::envelope)?;
                    }
                    None => {}
                }
            }
        }
        object.end().map_err(crate::Error::envelope)?;
        Ok(bytes)
    }

    fn decode(&self, bytes: &[u8]) -> Result<crate::ControlFrame, crate::Error> {
        let value: Value = serde_json::from_slice(bytes).map_err(crate::Error::envelope)?;
        let object = value
            .as_object()
            .ok_or_else(|| crate::Error::envelope_message("envelope must be an object"))?;
        for (name, expected) in CONSTANTS {
            if object.get(*name).and_then(Value::as_str) != Some(*expected) {
                return Err(crate::Error::envelope_message(format!("{name} must equal {expected:?}")));
            }
        }
        for shape in FRAMES {
            let Some(discriminator) = object
                .get(shape.discriminator.name)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            return match shape.kind {
                crate::FrameKind::Request => Ok(crate::ControlFrame::request(
                    required_id(object, *shape, "request")?,
                    discriminator,
                    optional_value(object, shape.payload),
                )),
                crate::FrameKind::Event => Ok(crate::ControlFrame::event(
                    required_id(object, *shape, "event")?,
                    discriminator,
                    optional_value(object, shape.payload),
                )),
                crate::FrameKind::Response => Ok(crate::ControlFrame::response(
                    discriminator,
                    optional_value(object, shape.payload),
                    decode_error(object, *shape)?,
                )),
            };
        }
        Err(crate::Error::envelope_message(
            "envelope has no recognized frame discriminator",
        ))
    }
}

fn frame_spec(kind: crate::FrameKind) -> Result<FrameSpec, crate::Error> {
    FRAMES
        .iter()
        .copied()
        .find(|frame| frame.kind == kind)
        .ok_or_else(|| crate::Error::envelope_message("frame mapping is missing"))
}

fn validate_frame(frame: &crate::ControlFrame) -> Result<(), crate::Error> {
    match frame.kind {
        crate::FrameKind::Request | crate::FrameKind::Event => {
            if frame.id.is_empty() || frame.method.is_empty() {
                return Err(crate::Error::envelope_message("request/event id and method are required"));
            }
        }
        crate::FrameKind::Response if frame.correlation_id.is_empty() => {
            return Err(crate::Error::envelope_message("response correlation id is required"));
        }
        crate::FrameKind::Response => {}
    }
    Ok(())
}

fn validate_wire_error(error: &crate::WireError) -> Result<(), crate::Error> {
    if error.code == 0 || error.message.is_empty() {
        return Err(crate::Error::envelope_message("error code must be non-zero and message is required"));
    }
    Ok(())
}

fn serialize_optional<M>(
    object: &mut M,
    field: FieldSpec,
    value: Option<&Value>,
) -> Result<(), crate::Error>
where
    M: SerializeMap,
    M::Error: std::error::Error + Send + Sync + 'static,
{
    match value {
        Some(value) => object.serialize_entry(field.name, value).map_err(crate::Error::envelope),
        None if !field.omit_when_none => object.serialize_entry(field.name, &Value::Null).map_err(crate::Error::envelope),
        None => Ok(()),
    }
}

fn optional_value(object: &serde_json::Map<String, Value>, field: FieldSpec) -> Option<Value> {
    object.get(field.name).cloned()
}

fn required_id(
    object: &serde_json::Map<String, Value>,
    shape: FrameSpec,
    kind: &str,
) -> Result<String, crate::Error> {
    let field = shape.id.ok_or_else(|| crate::Error::envelope_message(format!("{kind} id mapping is missing")))?;
    object
        .get(field.name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| crate::Error::envelope_message(format!("{kind} id is required")))
}

fn decode_error(
    object: &serde_json::Map<String, Value>,
    shape: FrameSpec,
) -> Result<Option<crate::WireError>, crate::Error> {
    let Some(value) = shape.error.and_then(|field| object.get(field.name)) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let error = value
        .as_object()
        .ok_or_else(|| crate::Error::envelope_message("error must be an object"))?;
    let code = error
        .get(ERROR.code.name)
        .and_then(Value::as_i64)
        .filter(|code| *code != 0)
        .ok_or_else(|| crate::Error::envelope_message("error code must be a non-zero integer"))?;
    let message = error
        .get(ERROR.message.name)
        .and_then(Value::as_str)
        .filter(|message| !message.is_empty())
        .ok_or_else(|| crate::Error::envelope_message("error message is required"))?;
    Ok(Some(crate::WireError {
        code,
        message: message.to_owned(),
        data: optional_value(error, ERROR.data),
    }))
}
"#;

fn render_tests(envelope: &EnvelopeSpec) -> Result<String, RustEmitError> {
    let mut output = String::from(RUST_BANNER);
    output.push_str("use super::*;\nuse crate::Envelope as _;\n\n");
    output.push_str(
        "#[test]\nfn generated_golden_frames_are_exact() {\n    let envelope = Envelope;\n",
    );
    for fixture in &envelope.fixtures {
        let bytes =
            String::from_utf8(fixture.bytes.clone()).map_err(|_| RustEmitError::Envelope {
                envelope: envelope.id.clone(),
                message: format!("fixture {:?} is not UTF-8", fixture.path),
            })?;
        let frame = render_frame(&fixture.frame)?;
        writeln!(
            output,
            "    {{\n        let name = {:?};\n        let golden = {:?}.as_bytes();\n        let frame = {};\n        assert_eq!(envelope.encode(&frame).unwrap(), golden, \"{{name}} encode\");\n        assert_eq!(envelope.decode(golden).unwrap(), frame, \"{{name}} decode\");\n    }}",
            fixture.path, bytes, frame
        )
        .unwrap();
    }
    output.push_str("}\n\n");
    output.push_str(CONTRACT_TESTS);
    Ok(super::finish(output))
}

const CONTRACT_TESTS: &str = r#"fn base_object() -> serde_json::Map<String, serde_json::Value> {
    CONSTANTS
        .iter()
        .map(|(name, value)| ((*name).to_owned(), serde_json::Value::String((*value).to_owned())))
        .collect()
}

fn shape(kind: crate::FrameKind) -> FrameSpec {
    FRAMES.iter().copied().find(|shape| shape.kind == kind).unwrap()
}

fn add_frame_identity(
    object: &mut serde_json::Map<String, serde_json::Value>,
    shape: FrameSpec,
    discriminator: &str,
) {
    if let Some(id) = shape.id {
        object.insert(id.name.to_owned(), serde_json::json!(format!("{discriminator}-id")));
    }
    object.insert(shape.discriminator.name.to_owned(), serde_json::json!(discriminator));
}

fn add_required_response_nulls(
    object: &mut serde_json::Map<String, serde_json::Value>,
    response: FrameSpec,
) {
    if !response.payload.omit_when_none {
        object.insert(response.payload.name.to_owned(), serde_json::Value::Null);
    }
}

fn discriminator(frame: &crate::ControlFrame) -> &str {
    if frame.kind == crate::FrameKind::Response {
        &frame.correlation_id
    } else {
        &frame.method
    }
}

#[test]
fn generated_structural_precedence_fallback_and_malformed_input_contract() {
    let envelope = Envelope;
    assert_eq!(envelope.name(), ENVELOPE_NAME);
    let first = FRAMES[0];
    let second = FRAMES[1];

    let mut object = base_object();
    add_frame_identity(&mut object, first, "first");
    add_frame_identity(&mut object, second, "second");
    let frame = envelope.decode(&serde_json::to_vec(&object).unwrap()).unwrap();
    assert_eq!(frame.kind, first.kind);
    assert_eq!(discriminator(&frame), "first");

    object.insert(first.discriminator.name.to_owned(), serde_json::json!(7));
    let frame = envelope.decode(&serde_json::to_vec(&object).unwrap()).unwrap();
    assert_eq!(frame.kind, second.kind);
    assert_eq!(discriminator(&frame), "second");

    for wire in [b"not json".as_slice(), b"[]".as_slice(), b"{}".as_slice()] {
        assert!(envelope.decode(wire).is_err(), "invalid envelope decoded: {wire:?}");
    }

    let request = shape(crate::FrameKind::Request);
    let mut missing_id = base_object();
    missing_id.insert(request.discriminator.name.to_owned(), serde_json::json!("ping"));
    assert!(envelope.decode(&serde_json::to_vec(&missing_id).unwrap()).is_err());
    if let Some((name, _)) = CONSTANTS.first() {
        let mut wrong_constant = base_object();
        wrong_constant.insert((*name).to_owned(), serde_json::json!("wrong"));
        add_frame_identity(&mut wrong_constant, request, "ping");
        assert!(envelope.decode(&serde_json::to_vec(&wrong_constant).unwrap()).is_err());
    }
}

#[test]
fn generated_error_null_data_and_permissive_response_contract() {
    let envelope = Envelope;
    let response = shape(crate::FrameKind::Response);
    let error_field = response.error.unwrap();

    let mut null_error = base_object();
    add_frame_identity(&mut null_error, response, "request-1");
    add_required_response_nulls(&mut null_error, response);
    null_error.insert(error_field.name.to_owned(), serde_json::Value::Null);
    let frame = envelope.decode(&serde_json::to_vec(&null_error).unwrap()).unwrap();
    assert!(frame.error.is_none());
    let encoded = envelope.encode(&frame).unwrap();
    let mut normalized = null_error;
    if error_field.omit_when_none {
        normalized.remove(error_field.name);
    }
    assert_eq!(encoded, serde_json::to_vec(&normalized).unwrap());

    let mut null_data = base_object();
    add_frame_identity(&mut null_data, response, "request-1");
    add_required_response_nulls(&mut null_data, response);
    null_data.insert(
        error_field.name.to_owned(),
        serde_json::json!({ERROR.code.name: 777, ERROR.message.name: "extension", ERROR.data.name: null}),
    );
    let wire = serde_json::to_vec(&null_data).unwrap();
    let frame = envelope.decode(&wire).unwrap();
    assert_eq!(frame.error.as_ref().unwrap().data, Some(serde_json::Value::Null));
    assert_eq!(envelope.encode(&frame).unwrap(), wire);

    let mut both = base_object();
    add_frame_identity(&mut both, response, "request-1");
    both.insert(response.payload.name.to_owned(), serde_json::json!({}));
    both.insert(
        error_field.name.to_owned(),
        serde_json::json!({ERROR.code.name: 500, ERROR.message.name: "failed"}),
    );
    let wire = serde_json::to_vec(&both).unwrap();
    let frame = envelope.decode(&wire).unwrap();
    assert_eq!(frame.payload, Some(serde_json::json!({})));
    assert_eq!(frame.error.as_ref().unwrap().code, 500);
    assert_eq!(envelope.encode(&frame).unwrap(), wire);

    let invalid_errors = [
        serde_json::json!([]),
        serde_json::json!({}),
        serde_json::json!({ERROR.code.name: 0, ERROR.message.name: "unset"}),
        serde_json::json!({ERROR.code.name: "bad", ERROR.message.name: "failed"}),
        serde_json::json!({ERROR.code.name: 500}),
        serde_json::json!({ERROR.code.name: 500, ERROR.message.name: ""}),
        serde_json::json!({ERROR.code.name: 500, ERROR.message.name: 7}),
    ];
    for invalid in invalid_errors {
        let mut object = base_object();
        add_frame_identity(&mut object, response, "request-1");
        add_required_response_nulls(&mut object, response);
        object.insert(error_field.name.to_owned(), invalid);
        assert!(envelope.decode(&serde_json::to_vec(&object).unwrap()).is_err());
    }
}

#[test]
fn generated_encode_validation_and_receive_timestamp_contract() {
    let envelope = Envelope;
    for frame in [
        crate::ControlFrame::request("", "ping", None),
        crate::ControlFrame::request("request-1", "", None),
        crate::ControlFrame::event("", "ready", None),
        crate::ControlFrame::event("event-1", "", None),
        crate::ControlFrame::response("", None, None),
        crate::ControlFrame::response(
            "request-1",
            None,
            Some(crate::WireError { code: 0, message: "unset".to_owned(), data: None }),
        ),
        crate::ControlFrame::response(
            "request-1",
            None,
            Some(crate::WireError { code: 500, message: String::new(), data: None }),
        ),
    ] {
        assert!(envelope.encode(&frame).is_err(), "invalid frame encoded: {frame:?}");
    }

    if let Some(fixture) = [
        crate::ControlFrame::event("event-1", "ready", None),
        crate::ControlFrame::request("request-1", "ping", None),
        crate::ControlFrame::response("request-1", None, None),
    ]
    .into_iter()
    .find(|frame| envelope.encode(frame).is_ok())
    {
        let baseline = envelope.encode(&fixture).unwrap();
        let mut timestamped = fixture;
        timestamped.received_at = Some(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(123));
        assert_eq!(envelope.encode(&timestamped).unwrap(), baseline);
    }
}
"#;

fn render_frame(frame: &ControlFrame) -> Result<String, RustEmitError> {
    Ok(match frame {
        ControlFrame::Request { id, method, params } => format!(
            "crate::ControlFrame::request({id:?}, {method:?}, {})",
            render_value(params.as_ref())?
        ),
        ControlFrame::Event { id, event, data } => format!(
            "crate::ControlFrame::event({id:?}, {event:?}, {})",
            render_value(data.as_ref())?
        ),
        ControlFrame::Response {
            correlation_id,
            result,
            error,
        } => format!(
            "crate::ControlFrame::response({correlation_id:?}, {}, {})",
            render_value(result.as_ref())?,
            match error {
                Some(error) => format!(
                    "Some(crate::WireError {{ code: {}, message: {:?}.to_owned(), data: {} }})",
                    error.code,
                    error.message,
                    render_value(error.data.as_ref())?
                ),
                None => "None".to_owned(),
            }
        ),
    })
}

fn render_value(value: Option<&serde_json::Value>) -> Result<String, RustEmitError> {
    match value {
        None => Ok("None".to_owned()),
        Some(value) => Ok(format!(
            "Some(serde_json::from_str::<serde_json::Value>({:?}).unwrap())",
            serde_json::to_string(value)?
        )),
    }
}

fn field(field: &rtvbp_spec_model::FieldSpec) -> String {
    format!(
        "FieldSpec {{ name: {:?}, omit_when_none: {} }}",
        field.name, field.omit_when_none
    )
}

fn rust_kind(kind: FrameKind) -> &'static str {
    match kind {
        FrameKind::Request => "crate::FrameKind::Request",
        FrameKind::Response => "crate::FrameKind::Response",
        FrameKind::Event => "crate::FrameKind::Event",
    }
}

fn module_name(id: &str) -> Result<String, RustEmitError> {
    let (name, major) = id
        .rsplit_once(".v")
        .ok_or_else(|| RustEmitError::Envelope {
            envelope: id.to_owned(),
            message: "id must end in .v<major>".to_owned(),
        })?;
    if name.is_empty() || major.is_empty() || !major.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RustEmitError::Envelope {
            envelope: id.to_owned(),
            message: "id must end in .v<major>".to_owned(),
        });
    }
    let name = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if name.is_empty() {
        return Err(RustEmitError::Envelope {
            envelope: id.to_owned(),
            message: "id has no module-name characters".to_owned(),
        });
    }
    Ok(format!("v{major}{name}"))
}
