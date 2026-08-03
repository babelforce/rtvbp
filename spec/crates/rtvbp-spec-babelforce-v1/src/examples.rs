//! Typed canonical examples used by validation, generators, and conformance.

use rtvbp_spec_model::Nullable;
use serde_json::{Map, Value, json};

use crate::*;

#[must_use]
pub fn session_initialize() -> (SessionInitializeRequest, SessionInitializeResponse) {
    (
        SessionInitializeRequest {
            application: AppInfo {
                id: "app-1".to_owned(),
            },
            call: CallInfo {
                id: "call-1".to_owned(),
                session_id: "session-1".to_owned(),
                from: "+12025550100".to_owned(),
                to: "+12025550101".to_owned(),
            },
            audio_codec_offerings: vec![AudioCodec::l16_8khz_mono()],
            metadata: Nullable::none(),
        },
        SessionInitializeResponse {
            audio_codec: Nullable::none(),
        },
    )
}

#[must_use]
pub fn session_terminate() -> (SessionTerminateRequest, EmptyResponse) {
    (
        SessionTerminateRequest {
            reason: "completed".to_owned(),
        },
        EmptyResponse {},
    )
}

#[must_use]
pub fn session_set() -> (SessionSetRequest, EmptyResponse) {
    (
        SessionSetRequest {
            data: canonical_variables(),
        },
        EmptyResponse {},
    )
}

#[must_use]
pub fn session_get() -> (SessionGetRequest, SessionGetResponse) {
    (
        SessionGetRequest {
            keys: vec!["customer".to_owned(), "attempt".to_owned()],
        },
        canonical_variables(),
    )
}

#[must_use]
pub fn application_move() -> (ApplicationMoveRequest, ApplicationMoveResponse) {
    (
        ApplicationMoveRequest {
            reason: Some("handoff".to_owned()),
            application_id: Some("app-2".to_owned()),
        },
        ApplicationMoveResponse {
            next_application_id: Some("app-2".to_owned()),
        },
    )
}

#[must_use]
pub fn call_hangup() -> (CallHangupRequest, EmptyResponse) {
    (
        CallHangupRequest {
            reason: "caller".to_owned(),
        },
        EmptyResponse {},
    )
}

#[must_use]
pub fn audio_buffer_clear() -> (AudioBufferClearRequest, AudioBufferClearResponse) {
    (
        AudioBufferClearRequest {},
        AudioBufferClearResponse { len: 320 },
    )
}

#[must_use]
pub fn recording_start() -> (RecordingStartRequest, RecordingStartResponse) {
    (
        RecordingStartRequest {
            tags: Some(vec!["support".to_owned(), "canonical".to_owned()]),
        },
        RecordingStartResponse {
            id: "recording-1".to_owned(),
        },
    )
}

#[must_use]
pub fn recording_stop() -> (RecordingStopRequest, EmptyResponse) {
    (
        RecordingStopRequest {
            id: "recording-1".to_owned(),
        },
        EmptyResponse {},
    )
}

#[must_use]
pub fn ping() -> (PingRequest, PingResponse) {
    (
        PingRequest {
            t0: 1_700_000_000_000,
            rtt: Some(42),
            data: Some(json!({"probe": "canonical"})),
        },
        PingResponse {
            t0: 1_700_000_000_000,
            t1: 1_700_000_000_010,
            t2: 1_700_000_000_012,
            owd: 5,
            data: Some(json!({"probe": "canonical"})),
        },
    )
}

#[must_use]
pub fn session_updated() -> SessionUpdatedEvent {
    SessionUpdatedEvent {
        audio_codec: Nullable::some(AudioCodec::l16_8khz_mono()),
    }
}

#[must_use]
pub fn dtmf() -> DtmfEvent {
    DtmfEvent {
        seq: 7,
        pressed_at: 1_700_000_000_000,
        released_at: 1_700_000_000_120,
        digit: "5".to_owned(),
    }
}

#[must_use]
pub fn call_hangup_event() -> CallHangupEvent {
    CallHangupEvent {
        reason: Some("caller".to_owned()),
    }
}

#[must_use]
pub fn audio_info() -> AudioInfoEvent {
    AudioInfoEvent::default()
}

#[must_use]
pub fn audio_speech_started() -> AudioSpeechStartedEvent {
    AudioSpeechStartedEvent {
        origin: "sender".to_owned(),
    }
}

#[must_use]
pub fn output_transcript_delta() -> OutputTranscriptDeltaEvent {
    OutputTranscriptDeltaEvent {
        delta: "Hi ".to_owned(),
    }
}

#[must_use]
pub fn output_transcript_done() -> OutputTranscriptDoneEvent {
    OutputTranscriptDoneEvent::default()
}

#[must_use]
pub fn input_transcript() -> InputTranscriptEvent {
    InputTranscriptEvent {
        delta: "hello there".to_owned(),
    }
}

#[must_use]
pub fn agent_tool_call() -> AgentToolCallEvent {
    AgentToolCallEvent {
        name: "lookup_order".to_owned(),
    }
}

fn canonical_variables() -> Map<String, Value> {
    json!({"attempt": 2, "customer": "Ada"})
        .as_object()
        .expect("canonical variables are an object")
        .clone()
}
