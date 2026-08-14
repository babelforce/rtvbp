#![forbid(unsafe_code)]

//! Rust SDK for the Real-Time Voice Bridge Protocol.

mod error;
mod frame;
mod protocol;

pub mod audio;
pub mod bridge;
pub mod catalog;
pub mod envelope;
pub mod profile;
pub mod session;
pub mod transport;

pub use audio::AudioObserver;
pub use error::{Error, ValidationError};
pub use frame::{ControlFrame, Envelope, FrameKind, WireError};
pub use protocol::{
    EventRegistration, HandlerReply, NamedEvent, NamedRequest, Notifier, RequestRegistration,
    Requester, Validate, notify_event, request_peer,
};
pub use session::{
    DeferredResponse, Handler, HandlerContext, InboundEvent, InboundRequest, Session,
    SessionConfig, SessionState,
};
pub use transport::{
    ControlChannel, KeepalivePolicy, MediaChannel, MediaFormat, MediaFrame, Received, Transport,
    TransportFactory,
};
