#![forbid(unsafe_code)]

use rtvbp_spec_model::{Catalog, Event, Operation, Role, Scenario, ScenarioCase, ScenarioStep};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct DemoEchoRequest {
    /// Text to echo through the application role.
    #[schemars(length(min = 1))]
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct DemoEchoResponse {
    /// Text returned unchanged by the application role.
    #[schemars(length(min = 1))]
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
pub struct DemoObservedEvent {
    /// Text observed after a completed echo.
    #[schemars(length(min = 1))]
    pub message: String,
}

#[must_use]
pub fn catalog() -> Catalog {
    let request = DemoEchoRequest {
        message: "hello from demo.v1".to_owned(),
    };
    let response = DemoEchoResponse {
        message: request.message.clone(),
    };
    let observed = DemoObservedEvent {
        message: request.message.clone(),
    };
    Catalog::new("demo", 1)
        .operation(
            Operation::new::<DemoEchoRequest, DemoEchoResponse>("demo.echo", Role::Application)
                .docs("Echo one message through the application peer.")
                .example(
                    "canonical",
                    serde_json::to_value(&request).unwrap(),
                    serde_json::to_value(&response).unwrap(),
                ),
        )
        .event(
            Event::new::<DemoObservedEvent>("demo.observed", Role::Application)
                .docs("Report that the application observed an echoed message.")
                .example("canonical", serde_json::to_value(&observed).unwrap()),
        )
        .scenarios([Scenario::new(
            "echo-observed",
            "Echo one message and publish the observation from the application peer.",
        )
        .role("voice", Role::Voice)
        .role("application", Role::Application)
        .case(ScenarioCase::new(
            "canonical",
            "The voice peer calls demo.echo and receives a matching observation event.",
            [
                ScenarioStep::request("voice", "$echo", "demo.echo", &request),
                ScenarioStep::response("application", "$echo", &response),
                ScenarioStep::event("application", "$observed", "demo.observed", &observed),
            ],
        ))])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn minimal_catalog_represents_both_roles_and_validates() {
        let catalog = catalog();
        catalog.validate().unwrap();
        assert_eq!(catalog.id.to_string(), "demo.v1");
        assert_eq!(catalog.operations.len(), 1);
        assert_eq!(catalog.operations[0].method, "demo.echo");
        assert_eq!(catalog.events.len(), 1);
        assert_eq!(catalog.events[0].name, "demo.observed");
        assert_eq!(catalog.scenarios.len(), 1);
        assert_eq!(
            catalog.operations[0].examples[0].request,
            json!({"message": "hello from demo.v1"})
        );
    }
}
