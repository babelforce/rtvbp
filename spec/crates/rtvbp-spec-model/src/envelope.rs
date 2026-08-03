use std::collections::HashSet;
use std::fmt;
use std::path::{Component, Path};

use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// One semantic control frame, independent of any envelope encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlFrame {
    Request {
        id: String,
        method: String,
        params: Option<Value>,
    },
    Response {
        correlation_id: String,
        result: Option<Value>,
        error: Option<WireError>,
    },
    Event {
        id: String,
        event: String,
        data: Option<Value>,
    },
}

/// An envelope-independent response error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// The semantic frame kind represented by an envelope shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameKind {
    Request,
    Response,
    Event,
}

/// A constant field emitted on every envelope frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstantField {
    pub name: String,
    pub value: String,
}

/// A named envelope field and its empty-value behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    pub name: String,
    pub omit_when_none: bool,
}

impl FieldSpec {
    #[must_use]
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            omit_when_none: false,
        }
    }

    #[must_use]
    pub fn optional(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            omit_when_none: true,
        }
    }
}

/// One frozen envelope fixture and its target-neutral semantic meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopeFixture {
    pub path: String,
    pub bytes: Vec<u8>,
    pub frame: ControlFrame,
}

impl EnvelopeFixture {
    #[must_use]
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>, frame: ControlFrame) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
            frame,
        }
    }
}

/// The field mapping for one semantic frame kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSpec {
    pub kind: FrameKind,
    pub discriminator: FieldSpec,
    pub id: Option<FieldSpec>,
    pub payload: FieldSpec,
    pub error: Option<FieldSpec>,
}

/// The field mapping inside a wire error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorSpec {
    pub code: FieldSpec,
    pub message: FieldSpec,
    pub data: FieldSpec,
}

/// A conventional error code minted by the deployed implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorCodeSpec {
    pub name: String,
    pub code: i64,
    pub description: String,
}

/// A declarative envelope description and executable Rust reference codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopeSpec {
    pub id: String,
    pub constants: Vec<ConstantField>,
    /// Frame order is also structural discrimination precedence.
    pub frames: Vec<FrameSpec>,
    pub error: ErrorSpec,
    /// Known conventions; decoders still accept any non-zero integer code.
    pub error_codes: Vec<ErrorCodeSpec>,
    /// Frozen wire witnesses used by generators and conformance harnesses.
    pub fixtures: Vec<EnvelopeFixture>,
}

impl EnvelopeSpec {
    /// Validate the complete envelope declaration and every frozen fixture.
    pub fn validate(&self) -> Result<(), EnvelopeValidationErrors> {
        let mut issues = self.structure_issues();
        if issues.is_empty() {
            let mut fixture_paths = HashSet::new();
            for fixture in &self.fixtures {
                if fixture.path.is_empty()
                    || !Path::new(&fixture.path)
                        .components()
                        .all(|component| matches!(component, Component::Normal(_)))
                {
                    issues.push(EnvelopeValidationError::InvalidFixturePath {
                        path: fixture.path.clone(),
                    });
                }
                if !fixture_paths.insert(fixture.path.as_str()) {
                    issues.push(EnvelopeValidationError::DuplicateFixturePath {
                        path: fixture.path.clone(),
                    });
                }
                match self.encode_unvalidated(&fixture.frame) {
                    Ok(bytes) if bytes != fixture.bytes => {
                        issues.push(EnvelopeValidationError::FixtureEncodingChanged {
                            path: fixture.path.clone(),
                            expected: fixture.bytes.clone(),
                            actual: bytes,
                        });
                    }
                    Ok(_) => {}
                    Err(error) => issues.push(EnvelopeValidationError::InvalidFixture {
                        path: fixture.path.clone(),
                        message: error.to_string(),
                    }),
                }
                match self.decode_unvalidated(&fixture.bytes) {
                    Ok(frame) if frame != fixture.frame => {
                        issues.push(EnvelopeValidationError::FixtureSemanticChanged {
                            path: fixture.path.clone(),
                            expected: Box::new(fixture.frame.clone()),
                            actual: Box::new(frame),
                        });
                    }
                    Ok(_) => {}
                    Err(error) => issues.push(EnvelopeValidationError::InvalidFixture {
                        path: fixture.path.clone(),
                        message: error.to_string(),
                    }),
                }
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(EnvelopeValidationErrors { issues })
        }
    }

    /// Encode a semantic control frame in this envelope.
    pub fn encode(&self, frame: &ControlFrame) -> Result<Vec<u8>, CodecError> {
        self.validate_structure()?;
        self.encode_unvalidated(frame)
    }

    fn encode_unvalidated(&self, frame: &ControlFrame) -> Result<Vec<u8>, CodecError> {
        let mut bytes = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut bytes);
        let mut map = serializer.serialize_map(None)?;

        for constant in &self.constants {
            map.serialize_entry(&constant.name, &constant.value)?;
        }

        match frame {
            ControlFrame::Request { id, method, params } => {
                require_non_empty("request id", id)?;
                require_non_empty("request method", method)?;
                let shape = self.frame(FrameKind::Request)?;
                let id_field = shape.id.as_ref().ok_or(CodecError::InvalidMapping(
                    "request frame must define an id field",
                ))?;
                map.serialize_entry(&id_field.name, id)?;
                map.serialize_entry(&shape.discriminator.name, method)?;
                serialize_optional(&mut map, &shape.payload, params.as_ref())?;
            }
            ControlFrame::Response {
                correlation_id,
                result,
                error,
            } => {
                require_non_empty("response correlation id", correlation_id)?;
                let shape = self.frame(FrameKind::Response)?;
                map.serialize_entry(&shape.discriminator.name, correlation_id)?;
                serialize_optional(&mut map, &shape.payload, result.as_ref())?;
                let error_field = shape.error.as_ref().ok_or(CodecError::InvalidMapping(
                    "response frame must define an error field",
                ))?;
                if let Some(error) = error {
                    validate_wire_error(error)?;
                    map.serialize_entry(
                        &error_field.name,
                        &EncodedError {
                            value: error,
                            spec: &self.error,
                        },
                    )?;
                } else if !error_field.omit_when_none {
                    map.serialize_entry(&error_field.name, &Value::Null)?;
                }
            }
            ControlFrame::Event { id, event, data } => {
                require_non_empty("event id", id)?;
                require_non_empty("event name", event)?;
                let shape = self.frame(FrameKind::Event)?;
                let id_field = shape.id.as_ref().ok_or(CodecError::InvalidMapping(
                    "event frame must define an id field",
                ))?;
                map.serialize_entry(&id_field.name, id)?;
                map.serialize_entry(&shape.discriminator.name, event)?;
                serialize_optional(&mut map, &shape.payload, data.as_ref())?;
            }
        }

        map.end()?;
        Ok(bytes)
    }

    /// Decode and validate one envelope frame.
    pub fn decode(&self, bytes: &[u8]) -> Result<ControlFrame, CodecError> {
        self.validate_structure()?;
        self.decode_unvalidated(bytes)
    }

    fn decode_unvalidated(&self, bytes: &[u8]) -> Result<ControlFrame, CodecError> {
        let value: Value = serde_json::from_slice(bytes)?;
        let object = value
            .as_object()
            .ok_or_else(|| CodecError::InvalidFrame("envelope must be an object".to_owned()))?;

        for constant in &self.constants {
            match object.get(&constant.name) {
                Some(Value::String(value)) if value == &constant.value => {}
                _ => {
                    return Err(CodecError::InvalidFrame(format!(
                        "{} must equal {:?}",
                        constant.name, constant.value
                    )));
                }
            }
        }

        for shape in &self.frames {
            let Some(Value::String(discriminator)) = object.get(&shape.discriminator.name) else {
                continue;
            };
            if discriminator.is_empty() {
                continue;
            }

            return match shape.kind {
                FrameKind::Request => Ok(ControlFrame::Request {
                    id: required_frame_id(object, shape, "request")?,
                    method: discriminator.clone(),
                    params: optional_value(object, &shape.payload),
                }),
                FrameKind::Event => Ok(ControlFrame::Event {
                    id: required_frame_id(object, shape, "event")?,
                    event: discriminator.clone(),
                    data: optional_value(object, &shape.payload),
                }),
                FrameKind::Response => Ok(ControlFrame::Response {
                    correlation_id: discriminator.clone(),
                    result: optional_value(object, &shape.payload),
                    error: match shape
                        .error
                        .as_ref()
                        .and_then(|field| object.get(&field.name))
                    {
                        None | Some(Value::Null) => None,
                        Some(value) => Some(self.decode_error(value)?),
                    },
                }),
            };
        }

        Err(CodecError::InvalidFrame(
            "envelope has no recognized frame discriminator".to_owned(),
        ))
    }

    fn frame(&self, kind: FrameKind) -> Result<&FrameSpec, CodecError> {
        self.frames
            .iter()
            .find(|frame| frame.kind == kind)
            .ok_or(CodecError::MissingFrameSpec(kind))
    }

    fn decode_error(&self, value: &Value) -> Result<WireError, CodecError> {
        let object = value
            .as_object()
            .ok_or_else(|| CodecError::InvalidFrame("error must be an object".to_owned()))?;
        let code = object
            .get(&self.error.code.name)
            .and_then(Value::as_i64)
            .ok_or_else(|| CodecError::InvalidFrame("error code must be an integer".to_owned()))?;
        let message = object
            .get(&self.error.message.name)
            .and_then(Value::as_str)
            .filter(|message| !message.is_empty())
            .ok_or_else(|| CodecError::InvalidFrame("error message is required".to_owned()))?;

        if code == 0 {
            return Err(CodecError::InvalidFrame(
                "error code must be non-zero".to_owned(),
            ));
        }

        Ok(WireError {
            code,
            message: message.to_owned(),
            data: optional_value(object, &self.error.data),
        })
    }

    fn validate_structure(&self) -> Result<(), CodecError> {
        let issues = self.structure_issues();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CodecError::InvalidSpec(EnvelopeValidationErrors { issues }))
        }
    }

    fn structure_issues(&self) -> Vec<EnvelopeValidationError> {
        let mut issues = Vec::new();
        if self.id.is_empty() {
            issues.push(EnvelopeValidationError::EmptyId);
        }

        let mut constants = HashSet::new();
        for constant in &self.constants {
            validate_field_name(&mut issues, "constant", &constant.name);
            if !constants.insert(constant.name.as_str()) {
                issues.push(EnvelopeValidationError::DuplicateConstant {
                    name: constant.name.clone(),
                });
            }
        }

        let mut kinds = HashSet::new();
        let mut discriminators = HashSet::new();
        for frame in &self.frames {
            if !kinds.insert(frame.kind) {
                issues.push(EnvelopeValidationError::DuplicateFrame { kind: frame.kind });
            }
            validate_field_name(
                &mut issues,
                "frame discriminator",
                &frame.discriminator.name,
            );
            if frame.discriminator.omit_when_none {
                issues.push(EnvelopeValidationError::InvalidFrameShape {
                    kind: frame.kind,
                    message: "the discriminator must be required",
                });
            }
            if !discriminators.insert(frame.discriminator.name.as_str()) {
                issues.push(EnvelopeValidationError::DuplicateDiscriminator {
                    name: frame.discriminator.name.clone(),
                });
            }
            validate_field_name(&mut issues, "frame payload", &frame.payload.name);
            if let Some(id) = &frame.id {
                validate_field_name(&mut issues, "frame id", &id.name);
            }
            if let Some(error) = &frame.error {
                validate_field_name(&mut issues, "frame error", &error.name);
            }

            match frame.kind {
                FrameKind::Request | FrameKind::Event => {
                    if frame.id.is_none() {
                        issues.push(EnvelopeValidationError::InvalidFrameShape {
                            kind: frame.kind,
                            message: "request and event frames must map an id",
                        });
                    } else if frame.id.as_ref().is_some_and(|id| id.omit_when_none) {
                        issues.push(EnvelopeValidationError::InvalidFrameShape {
                            kind: frame.kind,
                            message: "request and event ids must be required",
                        });
                    }
                    if frame.error.is_some() {
                        issues.push(EnvelopeValidationError::InvalidFrameShape {
                            kind: frame.kind,
                            message: "only response frames may map an error",
                        });
                    }
                }
                FrameKind::Response => {
                    if frame.id.is_some() {
                        issues.push(EnvelopeValidationError::InvalidFrameShape {
                            kind: frame.kind,
                            message: "response correlation is carried by its discriminator, not an id",
                        });
                    }
                    if frame.error.is_none() {
                        issues.push(EnvelopeValidationError::InvalidFrameShape {
                            kind: frame.kind,
                            message: "response frames must map an error",
                        });
                    }
                }
            }

            let mut wire_fields = constants.clone();
            for field in [
                Some(&frame.discriminator),
                frame.id.as_ref(),
                Some(&frame.payload),
                frame.error.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if !wire_fields.insert(field.name.as_str()) {
                    issues.push(EnvelopeValidationError::DuplicateWireField {
                        kind: frame.kind,
                        name: field.name.clone(),
                    });
                }
            }
        }
        for kind in [FrameKind::Event, FrameKind::Request, FrameKind::Response] {
            if !kinds.contains(&kind) {
                issues.push(EnvelopeValidationError::MissingFrame { kind });
            }
        }

        for (context, field) in [
            ("error code", &self.error.code),
            ("error message", &self.error.message),
            ("error data", &self.error.data),
        ] {
            validate_field_name(&mut issues, context, &field.name);
        }
        if self.error.code.omit_when_none || self.error.message.omit_when_none {
            issues.push(EnvelopeValidationError::InvalidErrorShape {
                message: "error code and message must be required",
            });
        }
        let mut error_fields = HashSet::new();
        for field in [&self.error.code, &self.error.message, &self.error.data] {
            if !error_fields.insert(field.name.as_str()) {
                issues.push(EnvelopeValidationError::DuplicateErrorField {
                    name: field.name.clone(),
                });
            }
        }

        let mut error_names = HashSet::new();
        let mut error_values = HashSet::new();
        for error in &self.error_codes {
            if error.name.is_empty() || error.description.is_empty() || error.code == 0 {
                issues.push(EnvelopeValidationError::InvalidErrorCode {
                    name: error.name.clone(),
                    code: error.code,
                });
            }
            if !error_names.insert(error.name.as_str()) {
                issues.push(EnvelopeValidationError::DuplicateErrorCodeName {
                    name: error.name.clone(),
                });
            }
            if !error_values.insert(error.code) {
                issues.push(EnvelopeValidationError::DuplicateErrorCodeValue { code: error.code });
            }
        }
        issues
    }
}

/// One invalid part of an envelope declaration.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EnvelopeValidationError {
    #[error("envelope id must not be empty")]
    EmptyId,
    #[error("{context} field name must not be empty")]
    EmptyFieldName { context: &'static str },
    #[error("duplicate constant field {name:?}")]
    DuplicateConstant { name: String },
    #[error("envelope has no {kind:?} frame")]
    MissingFrame { kind: FrameKind },
    #[error("envelope has more than one {kind:?} frame")]
    DuplicateFrame { kind: FrameKind },
    #[error("duplicate structural discriminator {name:?}")]
    DuplicateDiscriminator { name: String },
    #[error("invalid {kind:?} frame: {message}")]
    InvalidFrameShape {
        kind: FrameKind,
        message: &'static str,
    },
    #[error("{kind:?} frame emits wire field {name:?} more than once")]
    DuplicateWireField { kind: FrameKind, name: String },
    #[error("invalid error shape: {message}")]
    InvalidErrorShape { message: &'static str },
    #[error("duplicate error field {name:?}")]
    DuplicateErrorField { name: String },
    #[error("invalid conventional error code {name:?}={code}")]
    InvalidErrorCode { name: String, code: i64 },
    #[error("duplicate conventional error code name {name:?}")]
    DuplicateErrorCodeName { name: String },
    #[error("duplicate conventional error code value {code}")]
    DuplicateErrorCodeValue { code: i64 },
    #[error("invalid envelope fixture path {path:?}")]
    InvalidFixturePath { path: String },
    #[error("duplicate envelope fixture path {path:?}")]
    DuplicateFixturePath { path: String },
    #[error("envelope fixture {path:?} is invalid: {message}")]
    InvalidFixture { path: String, message: String },
    #[error("envelope fixture {path:?} changed bytes")]
    FixtureEncodingChanged {
        path: String,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    #[error("envelope fixture {path:?} changed semantic frame")]
    FixtureSemanticChanged {
        path: String,
        expected: Box<ControlFrame>,
        actual: Box<ControlFrame>,
    },
}

/// All issues found while validating one envelope declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopeValidationErrors {
    pub issues: Vec<EnvelopeValidationError>,
}

impl fmt::Display for EnvelopeValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, issue) in self.issues.iter().enumerate() {
            if index != 0 {
                formatter.write_str("; ")?;
            }
            write!(formatter, "{issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for EnvelopeValidationErrors {}

/// A reference codec failure.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("invalid envelope specification: {0}")]
    InvalidSpec(EnvelopeValidationErrors),
    #[error("invalid envelope mapping: {0}")]
    InvalidMapping(&'static str),
    #[error("envelope specification has no {0:?} frame")]
    MissingFrameSpec(FrameKind),
    #[error("invalid control frame: {0}")]
    InvalidControlFrame(String),
    #[error("invalid envelope frame: {0}")]
    InvalidFrame(String),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

struct EncodedError<'a> {
    value: &'a WireError,
    spec: &'a ErrorSpec,
}

impl Serialize for EncodedError<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(&self.spec.code.name, &self.value.code)?;
        map.serialize_entry(&self.spec.message.name, &self.value.message)?;
        serialize_optional(&mut map, &self.spec.data, self.value.data.as_ref())?;
        map.end()
    }
}

fn validate_field_name(
    issues: &mut Vec<EnvelopeValidationError>,
    context: &'static str,
    name: &str,
) {
    if name.is_empty() {
        issues.push(EnvelopeValidationError::EmptyFieldName { context });
    }
}

fn serialize_optional<M: SerializeMap>(
    map: &mut M,
    field: &FieldSpec,
    value: Option<&Value>,
) -> Result<(), M::Error> {
    if let Some(value) = value {
        map.serialize_entry(&field.name, value)?;
    } else if !field.omit_when_none {
        map.serialize_entry(&field.name, &Value::Null)?;
    }
    Ok(())
}

fn optional_value(object: &serde_json::Map<String, Value>, field: &FieldSpec) -> Option<Value> {
    object.get(&field.name).cloned()
}

fn required_frame_id(
    object: &serde_json::Map<String, Value>,
    shape: &FrameSpec,
    kind: &str,
) -> Result<String, CodecError> {
    let field = shape.id.as_ref().ok_or(CodecError::InvalidMapping(
        "request and event frames must define an id field",
    ))?;
    object
        .get(&field.name)
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| CodecError::InvalidFrame(format!("{kind} id is required")))
}

fn require_non_empty(label: &str, value: &str) -> Result<(), CodecError> {
    if value.is_empty() {
        Err(CodecError::InvalidControlFrame(format!(
            "{label} is required"
        )))
    } else {
        Ok(())
    }
}

fn validate_wire_error(error: &WireError) -> Result<(), CodecError> {
    if error.code == 0 {
        return Err(CodecError::InvalidControlFrame(
            "error code must be non-zero".to_owned(),
        ));
    }
    require_non_empty("error message", &error.message)
}
