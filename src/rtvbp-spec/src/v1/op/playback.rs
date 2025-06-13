use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybackStartRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupt: Option<bool>,

    #[serde(alias = "async", skip_serializing_if = "Option::is_none")]
    pub play_async: Option<bool>,

    pub content: PlaybackContent,
}

impl RequestExt for PlaybackStartRequest {
    type Response = PlaybackStartResponse;

    fn request_method_name() -> &'static str {
        "playback_start"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybackStartResponse {
    /// ID of the started playback
    id: String,
}

impl ResponseExt for PlaybackStartResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybackStopRequest {
    id: String,
}

impl RequestExt for PlaybackStopRequest {
    type Response = PlaybackStopResponse;

    fn request_method_name() -> &'static str {
        "playback_stop"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybackStopResponse {}

impl ResponseExt for PlaybackStopResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackContent {
    URL {
        url: String,
    },
    TTS {
        language: String,
        voice: Option<String>,
        text: String,
        provider: Option<String>,
    },
    Prompt {
        id: String,
    },
    Multiple(Vec<PlaybackContent>),
}

mod docs {
    use crate::v1::docs::{EXAMPLE_RESOURCE_ID, Example};
    use crate::v1::op::playback::{PlaybackContent, PlaybackStartRequest, PlaybackStartResponse};

    impl Example for PlaybackStartRequest {
        fn example() -> Self {
            Self {
                content: PlaybackContent::TTS {
                    text: "hello world".to_string(),
                    language: "en-US".to_string(),
                    voice: "my-cool-voice".to_string().into(),
                    provider: "some-provider".to_string().into(),
                },
                interrupt: Some(true),
                play_async: Some(true),
            }
        }
    }

    impl Example for PlaybackStartResponse {
        fn example() -> Self {
            Self {
                id: EXAMPLE_RESOURCE_ID.to_string(),
            }
        }
    }
}
