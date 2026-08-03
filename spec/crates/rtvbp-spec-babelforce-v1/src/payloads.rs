use rtvbp_spec_model::Nullable;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A negotiated audio codec.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AudioCodec {
    /// Codec identifier such as `L16/8000/1`.
    pub id: String,
    /// Codec name such as `L16`.
    pub name: String,
    /// Sample rate in hertz.
    #[schemars(extend("x-go-type" = "int"))]
    pub sample_rate: i64,
    /// Number of bits per sample.
    #[schemars(extend("x-go-type" = "int"))]
    pub bit_depth: i64,
    /// Number of audio channels.
    #[schemars(extend("x-go-type" = "int"))]
    pub channels: i64,
}

impl AudioCodec {
    /// The legacy default: mono L16 at 8 kHz.
    #[must_use]
    pub fn l16_8khz_mono() -> Self {
        Self {
            id: "L16/8000/1".to_owned(),
            name: "L16".to_owned(),
            sample_rate: 8_000,
            bit_depth: 16,
            channels: 1,
        }
    }
}

/// Telephony call metadata supplied when a session starts.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct CallInfo {
    /// Call identifier.
    pub id: String,
    /// Session identifier owned by the voice peer.
    pub session_id: String,
    /// Caller address or number.
    pub from: String,
    /// Callee address or number.
    pub to: String,
}

/// IVR application metadata supplied when a session starts.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AppInfo {
    /// Application or graph-node identifier.
    pub id: String,
}

/// Parameters for `session.initialize`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct SessionInitializeRequest {
    /// Application that owns the call flow.
    pub application: AppInfo,
    /// Call being bridged into the session.
    pub call: CallInfo,
    /// Audio codecs offered by the voice peer.
    pub audio_codec_offerings: Vec<AudioCodec>,
    /// Free-form session metadata; the field is present and may be `null`.
    pub metadata: Nullable<Map<String, Value>>,
}

/// Result of `session.initialize`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct SessionInitializeResponse {
    /// Selected codec; the field is present and may be `null`.
    pub audio_codec: Nullable<AudioCodec>,
}

/// Data emitted by `session.updated` when negotiated state changes.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct SessionUpdatedEvent {
    /// Current codec; the field is present and may be `null`.
    pub audio_codec: Nullable<AudioCodec>,
}

/// Parameters for terminal operation `session.terminate`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct SessionTerminateRequest {
    /// Reason the session is ending.
    pub reason: String,
}

/// Empty object returned by operations without result fields.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct EmptyResponse {}

/// Parameters for terminal operation `application.move`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct ApplicationMoveRequest {
    /// Optional reason for leaving the current graph node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional target graph-node identifier; absence means advance normally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
}

/// Result of `application.move`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct ApplicationMoveResponse {
    /// Graph node selected after the move, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_application_id: Option<String>,
}

/// Parameters for terminal operation `call.hangup`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct CallHangupRequest {
    /// Reason the call should be hung up.
    pub reason: String,
}

/// Data emitted by `call.hangup` when the voice peer reports call termination.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct CallHangupEvent {
    /// Optional hangup reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Parameters for `audio.buffer.clear`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AudioBufferClearRequest {}

/// Result of `audio.buffer.clear`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AudioBufferClearResponse {
    /// Number of buffered bytes removed.
    #[schemars(extend("x-go-type" = "int"))]
    pub len: i64,
}

/// Data emitted by `audio.speech.started` for barge-in signaling.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AudioSpeechStartedEvent {
    /// Side where speech began: `sender` or `receiver`.
    pub origin: String,
}

/// Throughput counters for one audio direction.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AudioInfoItem {
    /// Bytes transferred during the last reporting interval.
    pub bytes: i64,
    /// Transfer rate during the last reporting interval.
    #[serde(
        deserialize_with = "deserialize_go_float64",
        serialize_with = "serialize_go_float64"
    )]
    #[schemars(with = "f64")]
    pub bytes_per_second: f64,
    /// Total bytes transferred since session start.
    pub bytes_total: i64,
}

/// Data emitted by `audio.info` with read and write throughput counters.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AudioInfoEvent {
    /// Audio read from the telephony stream.
    pub read: AudioInfoItem,
    /// Audio written to the telephony stream.
    pub write: AudioInfoItem,
}

/// Data emitted by `dtmf` when the caller releases a key.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct DtmfEvent {
    /// Monotonic event sequence number within the session.
    #[schemars(extend("x-go-type" = "int"))]
    pub seq: i64,
    /// Epoch timestamp in milliseconds when the key was pressed.
    pub pressed_at: i64,
    /// Epoch timestamp in milliseconds when the key was released.
    pub released_at: i64,
    /// DTMF digit that was pressed.
    pub digit: String,
}

/// Parameters for `session.set`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct SessionSetRequest {
    /// Session variables to set.
    pub data: Map<String, Value>,
}

/// Parameters for `session.get`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct SessionGetRequest {
    /// Keys to fetch; an empty list requests all variables.
    pub keys: Vec<String>,
}

/// Bare open-map result returned by `session.get`.
pub type SessionGetResponse = Map<String, Value>;

/// Parameters for `recording.start`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct RecordingStartRequest {
    /// Optional recording tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Result of `recording.start`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct RecordingStartResponse {
    /// New recording identifier.
    pub id: String,
}

/// Parameters for `recording.stop`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct RecordingStopRequest {
    /// Recording identifier to stop.
    pub id: String,
}

/// Parameters for ordinary catalog operation `ping`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct PingRequest {
    /// Sender timestamp in epoch milliseconds.
    pub t0: i64,
    /// Optional round-trip time measured by a previous ping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtt: Option<i64>,
    /// Optional arbitrary data echoed by the peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Result of ordinary catalog operation `ping`.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct PingResponse {
    /// Echo of the request sender timestamp.
    pub t0: i64,
    /// Transport receive timestamp in epoch milliseconds.
    pub t1: i64,
    /// Application handling timestamp in epoch milliseconds.
    pub t2: i64,
    /// Estimated one-way delay in milliseconds.
    pub owd: i64,
    /// Optional arbitrary data echoed from the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Data emitted by `output.transcript.delta` for incremental agent speech text.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct OutputTranscriptDeltaEvent {
    /// Incremental transcript text.
    pub delta: String,
}

/// Data emitted by `output.transcript.done` when an agent utterance finishes.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct OutputTranscriptDoneEvent {
    /// Optional complete text; absence means retain the accumulated deltas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Data emitted by `input.transcript` for one finalized caller utterance.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct InputTranscriptEvent {
    /// Complete caller transcript; the legacy wire field is named `delta`.
    pub delta: String,
}

/// Data emitted by `agent.tool.call` when the agent invokes a tool.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, Serialize)]
pub struct AgentToolCallEvent {
    /// Tool name; arguments and results are deliberately excluded.
    pub name: String,
}

fn serialize_go_float64<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if value.is_finite()
        && value.fract() == 0.0
        && *value >= i64::MIN as f64
        && *value <= i64::MAX as f64
    {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

fn deserialize_go_float64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    f64::deserialize(deserializer)
}
