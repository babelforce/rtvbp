use rtvbp_spec_model::{Catalog, Event, Nullable, Operation, Role, TypeRef};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[allow(dead_code)]
#[derive(JsonSchema)]
struct DemoRequest {
    input: String,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct DemoResponse {
    output: String,
}

#[allow(dead_code)]
#[derive(JsonSchema)]
struct DemoEvent {
    state: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct PresenceExample {
    nullable: Nullable<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    optional: Option<String>,
}

#[test]
fn catalog_records_typed_operations_events_and_examples() {
    let catalog = Catalog::new("demo", 1)
        .operation(
            Operation::new::<DemoRequest, DemoResponse>("demo.run", Role::Application)
                .docs("Run the demo operation.")
                .terminal()
                .example(
                    "canonical",
                    json!({"input": "hello"}),
                    json!({"output": "world"}),
                ),
        )
        .event(
            Event::new::<DemoEvent>("demo.updated", Role::Voice)
                .docs("Report demo state.")
                .example("canonical", json!({"state": "ready"})),
        );

    assert_eq!(catalog.id.to_string(), "demo.v1");
    assert_eq!(catalog.operations.len(), 1);
    assert_eq!(catalog.operations[0].method, "demo.run");
    assert_eq!(catalog.operations[0].handled_by, Role::Application);
    assert_eq!(catalog.operations[0].request.name, "DemoRequest");
    assert_eq!(catalog.operations[0].response.name, "DemoResponse");
    assert!(catalog.operations[0].terminal);
    assert_eq!(catalog.operations[0].examples[0].name, "canonical");

    assert_eq!(catalog.events.len(), 1);
    assert_eq!(catalog.events[0].name, "demo.updated");
    assert_eq!(catalog.events[0].emitted_by, Role::Voice);
    assert_eq!(catalog.events[0].data.name, "DemoEvent");
    assert_eq!(
        catalog.events[0].examples[0].data,
        json!({"state": "ready"})
    );

    let string_ref = TypeRef::of::<String>();
    assert_eq!(string_ref.name, "string");
    assert_eq!(string_ref.schema.to_value()["type"], "string");
}

#[test]
fn nullable_serializes_null_and_marks_required_nullable_schema() {
    let value = PresenceExample {
        nullable: Nullable::none(),
        optional: None,
    };

    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"nullable": null})
    );

    let schema: Value = schema_for!(PresenceExample).to_value();
    assert_eq!(schema["required"], json!(["nullable"]));
    assert_eq!(
        schema["properties"]["nullable"]["x-rtvbp-presence"],
        "nullable"
    );
    assert!(
        schema["properties"]["optional"]
            .get("x-rtvbp-presence")
            .is_none()
    );

    let nullable_types = schema["properties"]["nullable"]["type"]
        .as_array()
        .expect("nullable type must be a JSON Schema type union");
    assert!(nullable_types.contains(&json!("string")));
    assert!(nullable_types.contains(&json!("null")));
}
