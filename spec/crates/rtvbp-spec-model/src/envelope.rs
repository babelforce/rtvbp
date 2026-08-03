use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// One semantic control frame, independent of any envelope encoding.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

/// The semantic frame kind represented by an envelope shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    fn required(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            omit_when_none: false,
        }
    }

    fn optional(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            omit_when_none: true,
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

/// A declarative envelope description and executable Rust reference codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopeSpec {
    pub id: String,
    pub constants: Vec<ConstantField>,
    /// Frame order is also structural discrimination precedence.
    pub frames: Vec<FrameSpec>,
    pub error: ErrorSpec,
}

impl EnvelopeSpec {
    /// Encode a semantic control frame in this envelope.
    pub fn encode(&self, frame: &ControlFrame) -> Result<Vec<u8>, CodecError> {
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
                let id_field = shape.id.as_ref().ok_or(CodecError::InvalidSpec(
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
                if let Some(error) = error {
                    let error_field = shape.error.as_ref().ok_or(CodecError::InvalidSpec(
                        "response frame must define an error field",
                    ))?;
                    map.serialize_entry(
                        &error_field.name,
                        &EncodedError {
                            value: error,
                            spec: &self.error,
                        },
                    )?;
                }
            }
            ControlFrame::Event { id, event, data } => {
                require_non_empty("event id", id)?;
                require_non_empty("event name", event)?;
                let shape = self.frame(FrameKind::Event)?;
                let id_field = shape.id.as_ref().ok_or(CodecError::InvalidSpec(
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
}

/// The frozen legacy flat JSON envelope.
#[must_use]
pub fn classic_v1() -> EnvelopeSpec {
    EnvelopeSpec {
        id: "classic.v1".to_owned(),
        constants: vec![ConstantField {
            name: "version".to_owned(),
            value: "1".to_owned(),
        }],
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
            data: FieldSpec::optional("any"),
        },
    }
}

/// A reference codec failure.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("invalid envelope specification: {0}")]
    InvalidSpec(&'static str),
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
    let field = shape.id.as_ref().ok_or(CodecError::InvalidSpec(
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
