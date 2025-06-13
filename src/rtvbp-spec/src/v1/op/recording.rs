use crate::v1::event::EventExt;
use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use indexmap::IndexSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordingStartRequest {
    /// Tags to store alongside the Recording
    pub tags: Option<IndexSet<String>>,
}

impl RequestExt for RecordingStartRequest {
    type Response = RecordingStartResponse;

    fn request_method_name() -> &'static str {
        "recording_start"
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordingStartResponse {
    pub recording_id: String,
}

impl ResponseExt for RecordingStartResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordingStopRequest;

impl RequestExt for RecordingStopRequest {
    type Response = RecordingStopResponse;

    fn request_method_name() -> &'static str {
        "recording_stop"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecordingStopResponse;

impl ResponseExt for RecordingStopResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecordingFinishedEvent {
    pub recording_id: String,
}

impl EventExt for RecordingFinishedEvent {
    fn event_name() -> &'static str {
        "recording_finished"
    }
}

mod docs {
    use crate::v1::docs::EXAMPLE_RESOURCE_ID;
    use crate::v1::docs::Example;
    use crate::v1::op::recording::{
        RecordingFinishedEvent, RecordingStartRequest, RecordingStartResponse,
        RecordingStopRequest, RecordingStopResponse,
    };

    impl Example for RecordingFinishedEvent {
        fn example() -> Self {
            Self {
                recording_id: EXAMPLE_RESOURCE_ID.to_string(),
            }
        }
    }

    impl Example for RecordingStopResponse {
        fn example() -> Self {
            Self {}
        }
    }

    impl Example for RecordingStopRequest {
        fn example() -> Self {
            Self {}
        }
    }

    impl Example for RecordingStartResponse {
        fn example() -> Self {
            Self {
                recording_id: EXAMPLE_RESOURCE_ID.to_string(),
            }
        }
    }

    impl Example for RecordingStartRequest {
        fn example() -> Self {
            Self {
                tags: Some(
                    vec!["tag1".to_string(), "tag2".to_string()]
                        .into_iter()
                        .collect(),
                ),
            }
        }
    }
}
