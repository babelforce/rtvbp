use rtvbp_spec_model::{
    Catalog, CatalogFixture, Event, Nullable, Operation, OperationRejection, Role, TypeRef,
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[allow(dead_code)]
#[derive(Deserialize, JsonSchema, Serialize)]
struct DemoRequest {
    input: String,
}

#[allow(dead_code)]
#[derive(Deserialize, JsonSchema, Serialize)]
struct DemoResponse {
    output: String,
}

#[allow(dead_code)]
#[derive(Deserialize, JsonSchema, Serialize)]
struct DemoEvent {
    state: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct PresenceExample {
    nullable: Nullable<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    optional: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[schemars(extend(
    "x-rtvbp-field-order" = [{"lower": "started_at", "upper": "finished_at"}]
))]
struct ValidatedRequest {
    #[schemars(length(min = 1))]
    name: String,
    #[schemars(range(min = 0), extend("x-rtvbp-nonzero" = true))]
    started_at: i64,
    #[schemars(range(min = 0))]
    finished_at: i64,
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
    assert_eq!(catalog.operations[0].handled_by, Some(Role::Application));
    assert_eq!(catalog.operations[0].request.name, "DemoRequest");
    assert_eq!(catalog.operations[0].response.name, "DemoResponse");
    assert!(catalog.operations[0].terminal);
    assert_eq!(catalog.operations[0].examples[0].name, "canonical");

    assert_eq!(catalog.events.len(), 1);
    assert_eq!(catalog.events[0].name, "demo.updated");
    assert_eq!(catalog.events[0].emitted_by, Some(Role::Voice));
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

#[test]
fn catalog_validation_rejects_duplicates_missing_docs_and_invalid_typed_examples() {
    let valid = Operation::new::<DemoRequest, DemoResponse>("demo.run", Role::Application)
        .docs("Run the demo operation.")
        .example(
            "canonical",
            json!({"input": "hello"}),
            json!({"output": "world"}),
        );
    let duplicate = Catalog::new("demo", 1)
        .operation(valid.clone())
        .operation(valid.clone());
    assert!(
        duplicate
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate operation")
    );

    let valid_event = Event::new::<DemoEvent>("demo.updated", Role::Voice)
        .docs("Report demo state.")
        .example("canonical", json!({"state": "ready"}));
    let duplicate_event = Catalog::new("demo", 1)
        .event(valid_event.clone())
        .event(valid_event);
    assert!(
        duplicate_event
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate event")
    );

    let missing_docs = Catalog::new("demo", 1).operation(
        Operation::new::<DemoRequest, DemoResponse>("demo.run", Role::Application).example(
            "canonical",
            json!({"input": "hello"}),
            json!({"output": "world"}),
        ),
    );
    assert!(
        missing_docs
            .validate()
            .unwrap_err()
            .to_string()
            .contains("documentation")
    );

    let invalid_example = Catalog::new("demo", 1).operation(
        Operation::new::<DemoRequest, DemoResponse>("demo.run", Role::Application)
            .docs("Run the demo operation.")
            .example(
                "canonical",
                json!({"input": 42}),
                json!({"output": "world"}),
            ),
    );
    assert!(
        invalid_example
            .validate()
            .unwrap_err()
            .to_string()
            .contains("canonical")
    );
}

#[test]
fn catalog_validation_rejects_operations_and_events_without_roles() {
    let mut operation = Operation::new::<DemoRequest, DemoResponse>("demo.run", Role::Application)
        .docs("Run the demo operation.")
        .example(
            "canonical",
            json!({"input": "hello"}),
            json!({"output": "world"}),
        );
    operation.handled_by = None;

    let mut event = Event::new::<DemoEvent>("demo.updated", Role::Voice)
        .docs("Report demo state.")
        .example("canonical", json!({"state": "ready"}));
    event.emitted_by = None;

    let error = Catalog::new("demo", 1)
        .operation(operation)
        .event(event)
        .validate()
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("operation \"demo.run\" has no role"),
        "{error}"
    );
    assert!(
        error.contains("event \"demo.updated\" has no role"),
        "{error}"
    );
}

#[test]
fn catalog_validation_accepts_structured_schema_rules_and_per_role_rejections() {
    let operation = Operation::new::<ValidatedRequest, DemoResponse>("demo.run", Role::Application)
        .docs("Run a validated operation.")
        .reject(OperationRejection::new(
            Role::Voice,
            501,
            "demo.run is not offered by voice",
        ))
        .example(
            "canonical",
            json!({"name": "hello", "started_at": 1, "finished_at": 2}),
            json!({"output": "world"}),
        );

    Catalog::new("demo", 1)
        .operation(operation)
        .validate()
        .unwrap();
}

#[test]
fn catalog_validation_rejects_malformed_schema_rule_metadata() {
    let cases = [
        (
            "nonzero marker",
            json!({
                "type": "object",
                "properties": {"value": {"type": "integer", "x-rtvbp-nonzero": "yes"}},
                "required": ["value"]
            }),
            "x-rtvbp-nonzero must be true",
        ),
        (
            "nonzero type",
            json!({
                "type": "object",
                "properties": {"value": {"type": "string", "x-rtvbp-nonzero": true}},
                "required": ["value"]
            }),
            "x-rtvbp-nonzero requires an integer field",
        ),
        (
            "minimum type",
            json!({
                "type": "object",
                "properties": {"value": {"type": "string", "minimum": 0}},
                "required": ["value"]
            }),
            "minimum requires a numeric field",
        ),
        (
            "min length type",
            json!({
                "type": "object",
                "properties": {"value": {"type": "integer", "minLength": 1}},
                "required": ["value"]
            }),
            "minLength requires a string field",
        ),
        (
            "field order shape",
            json!({
                "type": "object",
                "properties": {"first": {"type": "integer"}, "last": {"type": "integer"}},
                "required": ["first", "last"],
                "x-rtvbp-field-order": [{"lower": "first"}]
            }),
            "x-rtvbp-field-order entries require exactly lower and upper",
        ),
        (
            "unknown ordered field",
            json!({
                "type": "object",
                "properties": {"first": {"type": "integer"}, "last": {"type": "integer"}},
                "required": ["first", "last"],
                "x-rtvbp-field-order": [{"lower": "missing", "upper": "last"}]
            }),
            "references unknown lower field \"missing\"",
        ),
        (
            "ordered field type",
            json!({
                "type": "object",
                "properties": {"first": {"type": "string"}, "last": {"type": "integer"}},
                "required": ["first", "last"],
                "x-rtvbp-field-order": [{"lower": "first", "upper": "last"}]
            }),
            "lower field \"first\" must be an integer",
        ),
    ];

    for (name, schema, expected) in cases {
        let mut operation = operation_for_fixture();
        operation.request.schema = schema.try_into().unwrap();
        let error = Catalog::new("demo", 1)
            .operation(operation)
            .validate()
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{name}: {error}");
    }
}

#[test]
fn catalog_validation_rejects_invalid_per_role_operation_rejections() {
    let cases = [
        (
            OperationRejection::new(Role::Application, 501, "already handled"),
            "is already handled by application",
        ),
        (
            OperationRejection::new(Role::Both, 501, "ambiguous role"),
            "must name voice or application, not both",
        ),
        (
            OperationRejection::new(Role::Voice, 0, "unset"),
            "error code must be non-zero",
        ),
        (
            OperationRejection::new(Role::Voice, 501, "   "),
            "error message must be non-empty",
        ),
    ];

    for (rejection, expected) in cases {
        let operation = operation_for_fixture().reject(rejection);
        let error = Catalog::new("demo", 1)
            .operation(operation)
            .validate()
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }

    let duplicate = operation_for_fixture()
        .reject(OperationRejection::new(Role::Voice, 501, "first"))
        .reject(OperationRejection::new(Role::Voice, 500, "second"));
    let error = Catalog::new("demo", 1)
        .operation(duplicate)
        .validate()
        .unwrap_err()
        .to_string();
    assert!(error.contains("duplicate rejection for voice"), "{error}");
}

#[test]
fn catalog_validation_reserves_transport_methods_but_not_event_names() {
    let catalog = Catalog::new("demo", 1).operation(
        Operation::new::<DemoRequest, DemoResponse>("transport.offer", Role::Both)
            .docs("Attempt to claim the framework-reserved signaling namespace.")
            .example(
                "canonical",
                json!({"input": "sdp"}),
                json!({"output": "answer"}),
            ),
    );

    let error = catalog.validate().unwrap_err().to_string();
    assert!(error.contains("reserved transport.* namespace"), "{error}");

    Catalog::new("demo", 1)
        .event(
            Event::new::<DemoEvent>("transport.state", Role::Voice)
                .docs("Report transport state without claiming a control method.")
                .example("canonical", json!({"state": "connected"})),
        )
        .validate()
        .expect("the reserved namespace applies only to operation methods");
}

#[test]
fn catalog_validation_rejects_invalid_fixture_metadata_and_bytes() {
    let valid = operation_for_fixture();
    let duplicate = Catalog::new("demo", 1).operation(valid.clone()).fixtures([
        CatalogFixture::operation_request(
            "demo.run",
            "payloads/demo.json",
            br#"{"input":"hello"}"#.as_slice(),
        ),
        CatalogFixture::operation_request(
            "demo.run",
            "payloads/demo.json",
            br#"{"input":"hello"}"#.as_slice(),
        ),
    ]);
    assert!(
        duplicate
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate conformance fixture path")
    );

    let invalid = Catalog::new("demo", 1).operation(valid).fixtures([
        CatalogFixture::operation_request(
            "missing.run",
            "../outside.json",
            br#"{"input":"hello"}"#.as_slice(),
        ),
        CatalogFixture::operation_request(
            "demo.run",
            "payloads/changed.json",
            b"{\"input\":\"hello\" }".as_slice(),
        ),
        CatalogFixture::operation_request(
            "demo.run",
            "payloads/invalid.json",
            br#"{"input":42}"#.as_slice(),
        ),
    ]);
    let error = invalid.validate().unwrap_err().to_string();
    assert!(error.contains("relative and confined"), "{error}");
    assert!(
        error.contains("unknown operation \"missing.run\" request"),
        "{error}"
    );
    assert!(error.contains("changes after typed round-trip"), "{error}");
    assert!(error.contains("does not match DemoRequest"), "{error}");
}

fn operation_for_fixture() -> Operation {
    Operation::new::<DemoRequest, DemoResponse>("demo.run", Role::Application)
        .docs("Run the demo operation.")
        .example(
            "canonical",
            json!({"input": "hello"}),
            json!({"output": "world"}),
        )
}
