use crate::v1::event::EventExt;
use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use indexmap::IndexSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    SLIN,
    ALAW,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioDirection {
    /// Audio flows into both directions
    #[default]
    Both,

    /// Audio can flow into the session owning side
    In,

    /// Audio can flow out of the session owning side
    Out,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioCapabilities {
    ///
    pub direction: AudioDirection,

    /// Set of supported codecs
    pub codecs: IndexSet<AudioCodec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioStreamStartRequest {
    pub codec: Option<AudioCodec>,
}

impl RequestExt for AudioStreamStartRequest {
    type Response = AudioStreamStartResponse;

    fn request_method_name() -> &'static str {
        "audio_stream_start"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioStreamStartResponse;

impl ResponseExt for AudioStreamStartResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioStreamStopRequest;

impl RequestExt for AudioStreamStopRequest {
    type Response = AudioStreamStopResponse;

    fn request_method_name() -> &'static str {
        "audio_stream_stop"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AudioStreamStopResponse;

impl ResponseExt for AudioStreamStopResponse {}

mod docs {
    use crate::v1::docs::Example;
    use crate::v1::op::audio::{
        AudioCodec, AudioStreamStartRequest,
        AudioStreamStartResponse, AudioStreamStopRequest, AudioStreamStopResponse,
    };

    impl Example for AudioStreamStartRequest {
        fn example() -> Self {
            Self {
                codec: AudioCodec::ALAW.into(),
            }
        }
    }

    impl Example for AudioStreamStartResponse {
        fn example() -> Self {
            Self {}
        }
    }

    impl Example for AudioStreamStopRequest {
        fn example() -> Self {
            Self {}
        }
    }

    impl Example for AudioStreamStopResponse {
        fn example() -> Self {
            Self {}
        }
    }
}
