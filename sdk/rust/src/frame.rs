use std::time::SystemTime;

use serde_json::Value;

/// The semantic control-frame kind above an envelope codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameKind {
    Request,
    Response,
    Event,
}

/// One envelope-independent control frame.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlFrame {
    pub kind: FrameKind,
    pub id: String,
    pub correlation_id: String,
    pub method: String,
    pub payload: Option<Value>,
    pub error: Option<WireError>,
    pub received_at: Option<SystemTime>,
}

impl ControlFrame {
    pub fn request(
        id: impl Into<String>,
        method: impl Into<String>,
        payload: Option<Value>,
    ) -> Self {
        Self {
            kind: FrameKind::Request,
            id: id.into(),
            correlation_id: String::new(),
            method: method.into(),
            payload,
            error: None,
            received_at: None,
        }
    }

    pub fn response(
        correlation_id: impl Into<String>,
        payload: Option<Value>,
        error: Option<WireError>,
    ) -> Self {
        Self {
            kind: FrameKind::Response,
            id: String::new(),
            correlation_id: correlation_id.into(),
            method: String::new(),
            payload,
            error,
            received_at: None,
        }
    }

    pub fn event(id: impl Into<String>, event: impl Into<String>, payload: Option<Value>) -> Self {
        Self {
            kind: FrameKind::Event,
            id: id.into(),
            correlation_id: String::new(),
            method: event.into(),
            payload,
            error: None,
            received_at: None,
        }
    }
}

/// An envelope-independent response error.
#[derive(Clone, Debug, PartialEq)]
pub struct WireError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for WireError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

/// A stateless generated envelope codec.
pub trait Envelope: Send + Sync {
    fn name(&self) -> &'static str;

    /// Encode one semantic control frame.
    ///
    /// # Errors
    ///
    /// Returns an envelope error when the frame cannot be represented.
    fn encode(&self, frame: &ControlFrame) -> Result<Vec<u8>, crate::Error>;

    /// Decode one complete envelope message.
    ///
    /// # Errors
    ///
    /// Returns an envelope error when the bytes are malformed or incomplete.
    fn decode(&self, bytes: &[u8]) -> Result<ControlFrame, crate::Error>;
}
