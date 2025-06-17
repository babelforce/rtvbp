use crate::v1::event::EventExt;
use crate::v1::metadata::Metadata;
use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionTerminateRequest {}

impl RequestExt for SessionTerminateRequest {
    type Response = SessionTerminateResponse;

    fn request_method_name() -> &'static str {
        "session.terminate"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionTerminateResponse {}

impl ResponseExt for SessionTerminateResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionUpdatedEvent {
    /// Additional Metadata provided by the session owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
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
    use crate::v1::docs::{EXAMPLE_RESOURCE_ID, Example};
    use crate::v1::op::session::{SessionCloseReason, SessionTerminatedEvent, SessionUpdatedEvent};
    use indexmap::IndexMap;

    impl Example for SessionUpdatedEvent {
        fn example() -> Self {
            Self {
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
