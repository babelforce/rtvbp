use crate::v1::event::EventExt;
use crate::v1::metadata::Metadata;
use crate::v1::op::audio::AudioCapabilities;
use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use indexmap::IndexSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct SessionCapabilities {
    /// List of events which will be dispatched the lifetime of the session
    events: IndexSet<String>,

    /// List of requests which are allowed to be sent to the session
    requests: IndexSet<String>,

    /// Audio capabilities - set when audio is enabled on the session
    audio: Option<AudioCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionCreateRequest {
    pub session_id: String,

    /// Session capabilities of the session creator
    pub capabilities: SessionCapabilities,

    /// Additional Metadata provided by the session owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
}

impl RequestExt for SessionCreateRequest {
    type Response = SessionCreateResponse;
    fn request_method_name() -> &'static str {
        "session_create"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionCreateResponse {
    /// Session capabilities of the other end
    capabilities: SessionCapabilities,
}

impl ResponseExt for SessionCreateResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionUpdatedEvent {
    /// Unique ID of the session
    session_id: String,

    capabilities: Option<SessionCapabilities>,

    /// Additional Metadata provided by the session owner
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}

impl EventExt for SessionUpdatedEvent {
    fn event_name() -> &'static str {
        "session_updated"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionTerminatedEvent {
    session_id: String,
    reason: SessionCloseReason,
}

impl EventExt for SessionTerminatedEvent {
    fn event_name() -> &'static str {
        "session_terminated"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionCloseReason {
    Normal,
    Error,
    Timeout,
}

mod docs {
    use crate::v1::op::audio::{AudioCapabilities, AudioCodec, AudioDirection};

    use crate::v1::docs::{EXAMPLE_RESOURCE_ID, Example};
    use crate::v1::op::session::{
        SessionCapabilities, SessionCloseReason, SessionCreateRequest, SessionCreateResponse,
        SessionTerminatedEvent, SessionUpdatedEvent,
    };
    use indexmap::IndexMap;

    impl Example for SessionCreateRequest {
        fn example() -> Self {
            Self {
                session_id: EXAMPLE_RESOURCE_ID.to_string(),
                capabilities: SessionCapabilities::example().into(),
                metadata: None,
            }
        }
    }

    impl Example for SessionCreateResponse {
        fn example() -> Self {
            Self {
                capabilities: SessionCapabilities::example(),
            }
        }
    }

    impl Example for SessionCapabilities {
        fn example() -> Self {
            SessionCapabilities {
                events: vec![
                    "session_updated".to_string(),
                    "session_terminated".to_string(),
                ]
                .into_iter()
                .collect(),
                requests: vec![
                    // TODO: use enum list here
                    "call_hangup".to_string(),
                    "recording_start".to_string(),
                    "recording_stop".to_string(),
                ]
                .into_iter()
                .collect(),
                audio: AudioCapabilities {
                    codecs: vec![AudioCodec::ALAW].into_iter().collect(),
                    direction: AudioDirection::Both,
                }
                .into(),
            }
        }
    }

    impl Example for SessionUpdatedEvent {
        fn example() -> Self {
            Self {
                session_id: EXAMPLE_RESOURCE_ID.to_string(),
                capabilities: SessionCapabilities::example().into(),
                metadata: IndexMap::from([
                    ("call.id".into(), "1234".into()),
                    ("call.from".into(), "+493010001000".into()),
                ])
                .into(),
            }
        }
    }

    impl Example for SessionTerminatedEvent {
        fn example() -> Self {
            SessionTerminatedEvent {
                session_id: EXAMPLE_RESOURCE_ID.to_string(),
                reason: SessionCloseReason::Normal,
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::v1::docs::provider::EventSpecExt;

    #[test]
    fn test_example() {
        assert_eq!(SessionUpdatedEvent::event_name(), "session_updated");
        assert_eq!(
            SessionUpdatedEvent::spec_event_name(),
            "SessionUpdatedEvent"
        );
        assert_eq!(
            SessionUpdatedEvent::spec_operation_name(),
            "SessionUpdatedEvent"
        );
        assert_eq!(
            SessionUpdatedEvent::spec_event_payload_name(),
            "SessionUpdatedEventPayload"
        );

        println!("{:?}", SessionUpdatedEvent::operation());
    }
}
