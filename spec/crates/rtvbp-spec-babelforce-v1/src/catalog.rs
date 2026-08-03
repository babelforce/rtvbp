use rtvbp_spec_model::{Catalog, Event, Operation, OperationRejection, Role};
use serde::Serialize;
use serde_json::Value;

use crate::examples;
use crate::*;

/// Build the complete frozen `babelforce.v1` payload catalog.
#[must_use]
pub fn catalog() -> Catalog {
    Catalog::new("babelforce", 1)
        .operation(operation(
            Operation::new::<SessionInitializeRequest, SessionInitializeResponse>(
                "session.initialize",
                Role::Application,
            )
            .docs("Negotiate the audio codec and initialize a real-time voice session."),
            examples::session_initialize(),
        ))
        .operation(operation(
            Operation::new::<SessionTerminateRequest, EmptyResponse>(
                "session.terminate",
                Role::Application,
            )
            .docs("Terminate the real-time voice session after replying.")
            .reject(OperationRejection::new(
                Role::Voice,
                501,
                "session.terminate is not supported. please use application.move or call.hangup instead",
            ))
            .terminal(),
            examples::session_terminate(),
        ))
        .operation(operation(
            Operation::new::<SessionSetRequest, EmptyResponse>("session.set", Role::Voice)
                .docs("Set free-form variables on the voice session."),
            examples::session_set(),
        ))
        .operation(operation(
            Operation::new::<SessionGetRequest, SessionGetResponse>("session.get", Role::Voice)
                .docs("Get selected session variables, or all variables when keys is empty."),
            examples::session_get(),
        ))
        .operation(operation(
            Operation::new::<ApplicationMoveRequest, ApplicationMoveResponse>(
                "application.move",
                Role::Voice,
            )
            .docs("Advance the IVR flow to another application node after replying.")
            .terminal(),
            examples::application_move(),
        ))
        .operation(operation(
            Operation::new::<CallHangupRequest, EmptyResponse>("call.hangup", Role::Voice)
                .docs("Hang up the telephony call after replying.")
                .terminal(),
            examples::call_hangup(),
        ))
        .operation(operation(
            Operation::new::<AudioBufferClearRequest, AudioBufferClearResponse>(
                "audio.buffer.clear",
                Role::Voice,
            )
            .docs("Discard buffered audio that has not yet been played."),
            examples::audio_buffer_clear(),
        ))
        .operation(operation(
            Operation::new::<RecordingStartRequest, RecordingStartResponse>(
                "recording.start",
                Role::Voice,
            )
            .docs("Start recording the telephony call."),
            examples::recording_start(),
        ))
        .operation(operation(
            Operation::new::<RecordingStopRequest, EmptyResponse>("recording.stop", Role::Voice)
                .docs("Stop a recording by identifier."),
            examples::recording_stop(),
        ))
        .operation(operation(
            Operation::new::<PingRequest, PingResponse>("ping", Role::Both)
                .docs("Measure application-layer latency and optionally echo arbitrary data."),
            examples::ping(),
        ))
        .event(event(
            Event::new::<SessionUpdatedEvent>("session.updated", Role::Voice)
                .docs("Report a change to negotiated session state such as the selected codec."),
            examples::session_updated(),
        ))
        .event(event(
            Event::new::<DtmfEvent>("dtmf", Role::Voice)
                .docs("Report a DTMF key after the remote caller releases it."),
            examples::dtmf(),
        ))
        .event(event(
            Event::new::<CallHangupEvent>("call.hangup", Role::Voice)
                .docs("Report that the telephony call has ended."),
            examples::call_hangup_event(),
        ))
        .event(event(
            Event::new::<AudioInfoEvent>("audio.info", Role::Voice)
                .docs("Report periodic audio read and write throughput counters."),
            examples::audio_info(),
        ))
        .event(event(
            Event::new::<AudioSpeechStartedEvent>("audio.speech.started", Role::Application)
                .docs("Signal that speech started for barge-in handling."),
            examples::audio_speech_started(),
        ))
        .event(event(
            Event::new::<OutputTranscriptDeltaEvent>("output.transcript.delta", Role::Application)
                .docs("Stream an incremental piece of the agent's spoken-output transcript."),
            examples::output_transcript_delta(),
        ))
        .event(event(
            Event::new::<OutputTranscriptDoneEvent>("output.transcript.done", Role::Application)
                .docs("Finalize the current agent spoken-output transcript."),
            examples::output_transcript_done(),
        ))
        .event(event(
            Event::new::<InputTranscriptEvent>("input.transcript", Role::Application)
                .docs("Publish one finalized caller speech transcript."),
            examples::input_transcript(),
        ))
        .event(event(
            Event::new::<AgentToolCallEvent>("agent.tool.call", Role::Application)
                .docs("Publish the redaction-safe name of a tool invoked by the agent."),
            examples::agent_tool_call(),
        ))
        .fixtures(crate::fixtures::fixtures())
        .scenarios(crate::scenarios::scenarios())
}

fn operation<Request: Serialize, Response: Serialize>(
    operation: Operation,
    (request, response): (Request, Response),
) -> Operation {
    operation.example("canonical", value(request), value(response))
}

fn event<Data: Serialize>(event: Event, data: Data) -> Event {
    event.example("canonical", value(data))
}

fn value(value: impl Serialize) -> Value {
    serde_json::to_value(value).expect("canonical example must serialize")
}
