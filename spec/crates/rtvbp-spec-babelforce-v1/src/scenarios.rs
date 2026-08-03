use rtvbp_spec_model::{Role, Scenario, ScenarioCase, ScenarioStep, WireError};

use crate::examples;

pub(crate) fn scenarios() -> Vec<Scenario> {
    vec![initialize_updated_dtmf(), termination(), ping()]
}

fn base(name: &str) -> Scenario {
    Scenario::new(name)
        .role("voice", Role::Voice)
        .role("application", Role::Application)
}

fn initialize_updated_dtmf() -> Scenario {
    let (request, response) = examples::session_initialize();
    base("initialize-updated-dtmf").case(ScenarioCase::new(
        "canonical",
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

fn termination() -> Scenario {
    let (hangup, hangup_response) = examples::call_hangup();
    let (terminate, terminate_response) = examples::session_terminate();
    base("termination")
        .case(ScenarioCase::new(
            "application-call-hangup",
            [
                ScenarioStep::request("application", "$hangup", "call.hangup", &hangup),
                ScenarioStep::response("voice", "$hangup", &hangup_response),
            ],
        ))
        .case(ScenarioCase::new(
            "voice-session-terminate",
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
    base("ping").case(ScenarioCase::new(
        "both-directions",
        [
            ScenarioStep::request("voice", "$voice_ping", "ping", &request),
            ScenarioStep::response("application", "$voice_ping", &response),
            ScenarioStep::request("application", "$application_ping", "ping", &request),
            ScenarioStep::response("voice", "$application_ping", &response),
        ],
    ))
}
