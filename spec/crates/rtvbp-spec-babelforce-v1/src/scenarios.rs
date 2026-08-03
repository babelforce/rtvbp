use rtvbp_spec_model::{Role, Scenario, ScenarioCase, ScenarioStep, WireError};

use crate::examples;

pub(crate) fn scenarios() -> Vec<Scenario> {
    vec![initialize_updated_dtmf(), barge_in(), termination(), ping()]
}

fn base(name: &str, description: &str) -> Scenario {
    Scenario::new(name, description)
        .role("voice", Role::Voice)
        .role("application", Role::Application)
}

fn initialize_updated_dtmf() -> Scenario {
    let (request, response) = examples::session_initialize();
    base(
        "initialize-updated-dtmf",
        "Initialize a voice session, publish the negotiated state, and deliver caller DTMF.",
    )
    .case(ScenarioCase::new(
        "canonical",
        "The voice peer offers audio, the application selects it, then receives session state and DTMF.",
        [
            ScenarioStep::request("voice", "$init", "session.initialize", &request),
            ScenarioStep::response("application", "$init", &response),
            ScenarioStep::event(
                "voice",
                "$updated",
                "session.updated",
                &examples::session_updated(),
            ),
            ScenarioStep::event("voice", "$dtmf", "dtmf", &examples::dtmf()),
        ],
    ))
}

fn barge_in() -> Scenario {
    let (clear, cleared) = examples::audio_buffer_clear();
    base(
        "barge-in",
        "Stop queued application audio when new speech begins.",
    )
    .case(ScenarioCase::new(
        "speech-started-clear-buffer",
        "The application signals speech detection and clears audio queued by the voice peer.",
        [
            ScenarioStep::event(
                "application",
                "$speech",
                "audio.speech.started",
                &examples::audio_speech_started(),
            ),
            ScenarioStep::request("application", "$clear", "audio.buffer.clear", &clear),
            ScenarioStep::response("voice", "$clear", &cleared),
        ],
    ))
}

fn termination() -> Scenario {
    let (hangup, hangup_response) = examples::call_hangup();
    let (terminate, terminate_response) = examples::session_terminate();
    base(
        "termination",
        "Close a session through the supported terminal operations and preserve the frozen reverse rejection.",
    )
        .case(ScenarioCase::new(
            "application-call-hangup",
            "The application asks the voice peer to hang up the telephony call.",
            [
                ScenarioStep::request("application", "$hangup", "call.hangup", &hangup),
                ScenarioStep::response("voice", "$hangup", &hangup_response),
            ],
        ))
        .case(ScenarioCase::new(
            "voice-session-terminate",
            "The voice peer asks the application to terminate the RTVBP session.",
            [
                ScenarioStep::request(
                    "voice",
                    "$terminate",
                    "session.terminate",
                    &terminate,
                ),
                ScenarioStep::response("application", "$terminate", &terminate_response),
            ],
        ))
        .case(ScenarioCase::new(
            "reverse-session-terminate-rejection",
            "The voice role rejects the frozen unsupported reverse session.terminate direction.",
            [
                ScenarioStep::request(
                    "application",
                    "$reverse",
                    "session.terminate",
                    &terminate,
                ),
                ScenarioStep::error(
                    "voice",
                    "$reverse",
                    WireError {
                        code: 501,
                        message: "session.terminate is not supported. please use application.move or call.hangup instead".to_owned(),
                        data: None,
                    },
                ),
            ],
        ))
}

fn ping() -> Scenario {
    let (request, response) = examples::ping();
    base(
        "ping",
        "Measure application-layer timing independently of transport keepalive.",
    )
    .case(ScenarioCase::new(
        "both-directions",
        "Each peer calls the bidirectional ping operation once.",
        [
            ScenarioStep::request("voice", "$voice_ping", "ping", &request),
            ScenarioStep::response("application", "$voice_ping", &response),
            ScenarioStep::request("application", "$application_ping", "ping", &request),
            ScenarioStep::response("voice", "$application_ping", &response),
        ],
    ))
}
