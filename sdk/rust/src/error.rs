use std::fmt::Display;

/// One SDK failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("envelope: {0}")]
    Envelope(String),
    #[error("validation: {0}")]
    Validation(#[from] ValidationError),
    #[error("remote error {0}")]
    Remote(crate::WireError),
    #[error("handler error {0}")]
    Handler(crate::WireError),
    #[error("invalid media format: {0}")]
    InvalidMediaFormat(String),
    #[error("dynamic media is unsupported")]
    MediaUnsupported,
    #[error("media channel is already open")]
    MediaAlreadyOpen,
    #[error("channel is closed")]
    Closed,
    #[error("transport: {0}")]
    Transport(String),
    #[error("unsupported WebSocket subprotocol: {0}")]
    UnsupportedSubprotocol(String),
    #[error("keepalive timed out")]
    KeepaliveTimeout,
    #[error("operation timed out")]
    Timeout,
    #[error("request timed out")]
    RequestTimeout,
    #[error("request failed: {0}")]
    RequestFailed(String),
    #[error("session is closed")]
    SessionClosed,
    #[error("session has already run")]
    SessionAlreadyRun,
    #[error("session failed: {0}")]
    SessionFailed(String),
    #[error("response has already been sent or deferred")]
    ResponseAlreadySent,
    #[error("there is no inbound request context")]
    NoRequestContext,
    #[error("audio is already bound")]
    AudioAlreadyBound,
    #[error("audio format conflicts with the negotiated format")]
    AudioFormatConflict,
    #[error("audio transport is unavailable")]
    AudioUnavailable,
    #[error("configuration: {0}")]
    Configuration(String),
}

impl Error {
    pub fn envelope(error: impl Display) -> Self {
        Self::Envelope(error.to_string())
    }

    pub fn envelope_message(message: impl Into<String>) -> Self {
        Self::Envelope(message.into())
    }
}

/// A generated payload violated a catalog constraint.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ValidationError {
    message: String,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
