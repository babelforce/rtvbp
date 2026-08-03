use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use rtvbp_spec_gen::catalogs;
use rtvbp_spec_gen::emit::{GeneratedFile, Target};
use rtvbp_spec_gen::resolve::resolve;
use rtvbp_spec_gen::write::{check_files, check_owned_files, synchronize_files, write_files};
use rtvbp_spec_gen::{ResolveError, emit_docs, emit_go, emit_go_envelope, generate};
use rtvbp_spec_model::{
    Catalog, ConstantField, EnvelopeSpec, ErrorSpec, Event, FieldSpec, FrameKind, FrameSpec,
    Operation, Role,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
struct Request {
    input: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
struct Response {
    output: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
struct EventData {
    state: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
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

mod first {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, JsonSchema, Serialize)]
    pub struct Collision {
        pub text: String,
    }
}

mod second {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, JsonSchema, Serialize)]
    pub struct Collision {
        pub count: u32,
    }
}

#[derive(Deserialize, JsonSchema, Serialize)]
struct Wrapper {
    shared: Shared,
}

#[derive(Deserialize, JsonSchema, Serialize)]
struct Shared {
    value: String,
}

fn operation(method: &str) -> Operation {
    Operation::new::<Request, Response>(method, Role::Application)
        .docs("Run an operation.")
        .example(
            "canonical",
            json!({"input": "hello"}),
            json!({"output": "world"}),
        )
}

fn event(name: &str) -> Event {
    Event::new::<EventData>(name, Role::Voice)
        .docs("Report an event.")
        .example("canonical", json!({"state": "ready"}))
}

#[test]
fn validate_stage_rejects_a_duplicate_method() {
    let catalog = Catalog::new("demo", 1)
        .operation(operation("demo.run"))
        .operation(operation("demo.run"));

    let error = catalogs::validate(&[catalog]).unwrap_err().to_string();
    assert!(error.contains("duplicate operation method"), "{error}");
}

#[test]
fn validate_stage_rejects_a_duplicate_event() {
    let catalog = Catalog::new("demo", 1)
        .event(event("demo.updated"))
        .event(event("demo.updated"));

    let error = catalogs::validate(&[catalog]).unwrap_err().to_string();
    assert!(error.contains("duplicate event name"), "{error}");
}

#[test]
fn validate_stage_rejects_an_operation_without_a_role() {
    let mut item = operation("demo.run");
    item.handled_by = None;

    let error = catalogs::validate(&[Catalog::new("demo", 1).operation(item)])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("operation \"demo.run\" has no role"),
        "{error}"
    );
}

#[test]
fn validate_stage_rejects_an_event_without_a_role() {
    let mut item = event("demo.updated");
    item.emitted_by = None;

    let error = catalogs::validate(&[Catalog::new("demo", 1).event(item)])
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("event \"demo.updated\" has no role"),
        "{error}"
    );
}

#[test]
fn validate_stage_rejects_an_example_that_does_not_round_trip() {
    let invalid = Operation::new::<Request, Response>("demo.run", Role::Application)
        .docs("Run an operation.")
        .example(
            "canonical",
            json!({"input": 42}),
            json!({"output": "world"}),
        );

    let error = catalogs::validate(&[Catalog::new("demo", 1).operation(invalid)])
        .unwrap_err()
        .to_string();
    assert!(error.contains("does not match Request"), "{error}");
}

#[test]
fn resolve_requires_roles_and_produces_stably_sorted_complete_schema_registry() {
    let mut missing_role = operation("demo.run");
    missing_role.handled_by = None;
    assert!(matches!(
        resolve(Catalog::new("demo", 1).operation(missing_role)),
        Err(ResolveError::MissingOperationRole { .. })
    ));

    let catalog = Catalog::new("demo", 1)
        .operation(operation("z.last"))
        .operation(operation("a.first"))
        .event(event("z.last"))
        .event(event("a.first"));
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();
    let resolved = resolve(catalog).unwrap();

    assert_eq!(
        resolved
            .operations
            .iter()
            .map(|operation| operation.method.as_str())
            .collect::<Vec<_>>(),
        ["a.first", "z.last"]
    );
    assert_eq!(
        resolved
            .events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        ["a.first", "z.last"]
    );
    assert!(resolved.schemas.contains_key("Request"));
    assert!(resolved.schemas.contains_key("Response"));
    assert!(resolved.schemas.contains_key("EventData"));
}

#[test]
fn resolve_rejects_conflicting_schemas_with_the_same_name() {
    let first = Operation::new::<first::Collision, Response>("demo.first", Role::Application)
        .docs("Run the first operation.")
        .example(
            "canonical",
            json!({"text": "hello"}),
            json!({"output": "world"}),
        );
    let second = Operation::new::<second::Collision, Response>("demo.second", Role::Application)
        .docs("Run the second operation.")
        .example("canonical", json!({"count": 2}), json!({"output": "world"}));
    let catalog = Catalog::new("demo", 1).operation(first).operation(second);
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();

    assert!(matches!(
        resolve(catalog),
        Err(ResolveError::ConflictingSchema { name }) if name == "Collision"
    ));
}

#[test]
fn resolve_deduplicates_a_type_used_as_both_a_root_and_a_definition() {
    let nested = Operation::new::<Wrapper, Response>("demo.nested", Role::Application)
        .docs("Use the shared type as a nested definition.")
        .example(
            "canonical",
            json!({"shared": {"value": "hello"}}),
            json!({"output": "world"}),
        );
    let root = Operation::new::<Shared, Response>("demo.root", Role::Application)
        .docs("Use the shared type as a root schema.")
        .example(
            "canonical",
            json!({"value": "hello"}),
            json!({"output": "world"}),
        );
    let catalog = Catalog::new("demo", 1).operation(nested).operation(root);
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();

    let resolved = resolve(catalog).unwrap();
    assert_eq!(
        resolved
            .schemas
            .keys()
            .filter(|name| name.as_str() == "Shared")
            .count(),
        1
    );
}

#[test]
fn manifest_contains_the_complete_catalog_roles_terminality_and_embedded_schemas() {
    let files = generate(Target::Manifest).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, Path::new("babelforce.v1.catalog.json"));
    assert_eq!(files[1].path, Path::new("demo.v1.catalog.json"));
    let manifest: Value = serde_json::from_slice(&files[0].bytes).unwrap();

    assert_eq!(manifest["catalog"]["id"], "babelforce.v1");
    assert_eq!(manifest["operations"].as_array().unwrap().len(), 10);
    assert_eq!(manifest["events"].as_array().unwrap().len(), 9);
    assert!(
        manifest["operations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|operation| operation["handledBy"].is_string()
                && operation["terminal"].is_boolean())
    );
    assert!(
        manifest["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|event| event["emittedBy"].is_string() && event.get("terminal").is_none())
    );

    let schemas = manifest["schemas"].as_object().unwrap();
    for item in manifest["operations"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|operation| [&operation["request"], &operation["response"]])
        .chain(
            manifest["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|event| &event["data"]),
        )
    {
        let name = item["$ref"]
            .as_str()
            .unwrap()
            .strip_prefix("#/schemas/")
            .unwrap();
        assert!(schemas.contains_key(name), "missing schema {name}");
    }
    assert_schema_refs_resolve(&manifest["schemas"], schemas);
    assert!(!files[0].bytes.windows(8).any(|bytes| bytes == b"#/$defs/"));
}

#[test]
fn manifest_emitter_is_deterministic_and_ends_with_one_newline() {
    let first = generate(Target::Manifest).unwrap();
    let second = generate(Target::Manifest).unwrap();

    assert_eq!(first, second);
    assert!(first.iter().all(|file| file.bytes.ends_with(b"\n")));
    assert!(first.iter().all(|file| !file.bytes.ends_with(b"\n\n")));
}

#[test]
fn go_emitter_pins_names_presence_order_docs_and_all_golden_cases() {
    assert_eq!(Target::from_str("go").unwrap(), Target::Go);
    assert_eq!(Target::Go.canonical_out_dir(), "sdk/go");
    assert!(Target::Go.owns_output_path(Path::new("catalog/babelforcev1/zz_generated.types.go")));
    assert!(Target::Go.owns_output_path(Path::new("envelope/v1classic/zz_generated.codec.go")));
    assert!(!Target::Go.owns_output_path(Path::new("catalog/babelforcev1/handwritten.go")));
    assert!(!Target::Go.owns_output_path(Path::new("zz_generated.runtime.go")));

    let first = generate(Target::Go).unwrap();
    let second = generate(Target::Go).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 10);
    assert_eq!(
        first
            .iter()
            .map(|file| file.path.as_path())
            .collect::<Vec<_>>(),
        [
            Path::new("catalog/babelforcev1/zz_generated.golden_test.go"),
            Path::new("catalog/babelforcev1/zz_generated.roles.go"),
            Path::new("catalog/babelforcev1/zz_generated.roles_test.go"),
            Path::new("catalog/babelforcev1/zz_generated.types.go"),
            Path::new("catalog/demov1/zz_generated.golden_test.go"),
            Path::new("catalog/demov1/zz_generated.roles.go"),
            Path::new("catalog/demov1/zz_generated.roles_test.go"),
            Path::new("catalog/demov1/zz_generated.types.go"),
            Path::new("envelope/v1classic/zz_generated.codec.go"),
            Path::new("envelope/v1classic/zz_generated.golden_test.go"),
        ]
    );
    for file in &first {
        assert!(file.bytes.starts_with(b"// Code generated"));
        assert!(file.bytes.ends_with(b"\n"));
        assert!(!file.bytes.ends_with(b"\n\n"));
    }

    let types = String::from_utf8(
        first
            .iter()
            .find(|file| file.path.ends_with("zz_generated.types.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert!(types.contains("type DtmfEvent struct"));
    assert!(!types.contains("DTMFEvent"));
    assert!(types.contains("Application AppInfo `json:\"application\"`"));
    assert!(types.contains("Metadata *map[string]any `json:\"metadata\"`"));
    assert!(!types.contains("metadata,omitempty"));
    assert!(types.contains("Reason string `json:\"reason,omitempty\"`"));
    assert!(types.contains("Text *string `json:\"text,omitempty\"`"));
    assert!(types.contains("type SessionGetResponse map[string]any"));
    assert!(types.contains("const MethodSessionInitialize = \"session.initialize\""));
    assert!(types.contains("func (*SessionUpdatedEvent) EventName() string"));
    assert!(types.contains("func (value *CallHangupRequest) Validate() error"));
    assert!(types.contains("if len(value.Reason) < 1"));
    assert!(types.contains("if value.Seq < 0"));
    assert!(types.contains("if value.T0 == 0"));
    assert!(types.contains("if value.PressedAt > value.ReleasedAt"));
    assert!(types.contains("Application that owns the call flow."));
    assert!(
        types.find("Application AppInfo").unwrap() < types.find("Call CallInfo").unwrap()
            && types.find("Call CallInfo").unwrap()
                < types.find("AudioCodecOfferings []AudioCodec").unwrap()
    );

    let roles = String::from_utf8(
        first
            .iter()
            .find(|file| file.path.ends_with("zz_generated.roles.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert!(roles.contains("type ApplicationHandler interface"));
    assert!(roles.contains("type VoiceHandler interface"));
    assert!(roles.contains("func ApplicationHandlers(handler ApplicationHandler) []any"));
    assert!(roles.contains("rtvbp.HandleTerminalRequest(handler.SessionTerminate)"));
    assert!(roles.contains(
        "rtvbp.HandleWithError[*SessionTerminateRequest](rtvbp.WireError{Code: 501, Message: \"session.terminate is not supported. please use application.move or call.hangup instead\"})"
    ));
    assert!(roles.contains("type VoicePeer struct"));
    assert!(roles.contains("func (peer *VoicePeer) CallHangup"));
    assert!(roles.contains("type ApplicationEvents struct"));
    assert!(roles.contains("type VoiceEvents struct"));
    assert!(roles.contains("type ApplicationEventHandler interface"));
    assert!(roles.contains("type VoiceEventHandler interface"));

    let tests = String::from_utf8(
        first
            .iter()
            .find(|file| file.path.ends_with("zz_generated.golden_test.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert_eq!(tests.matches("\t{name: \"").count(), 38);
    assert!(tests.contains("events/output.transcript.done.json"));
    assert!(tests.contains("variants/events/output.transcript.done-text-empty.json"));
    assert!(tests.contains("variants/events/output.transcript.done-text.json"));
    assert!(tests.contains("Text: ptr(\"\")"));
    assert!(tests.contains("Text: ptr(\"Hi there\")"));
    assert!(tests.contains("/round_trip"));
    assert!(tests.contains("/construct"));

    let codec = String::from_utf8(
        first
            .iter()
            .find(|file| file.path.ends_with("zz_generated.codec.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert!(codec.contains("const envelopeName = \"classic.v1\""));
    assert!(codec.contains("var _ rtvbp.Envelope = Envelope{}"));
    assert!(
        codec.find("discriminator: \"event\"").unwrap()
            < codec.find("discriminator: \"method\"").unwrap()
    );

    let envelope_tests = String::from_utf8(
        first
            .iter()
            .find(|file| file.path == Path::new("envelope/v1classic/zz_generated.golden_test.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert_eq!(envelope_tests.matches("\t{name: \"").count(), 10);
    assert!(envelope_tests.contains("response-ok-null-result.json"));
    assert!(envelope_tests.contains("TestStructuralPrecedenceAndMalformedInput"));
}

#[test]
fn docs_emitter_projects_the_catalog_roles_examples_and_envelope() {
    assert_eq!(Target::from_str("docs").unwrap(), Target::Docs);
    assert_eq!(Target::Docs.canonical_out_dir(), "website/docs/reference");
    assert!(
        Target::Docs.owns_output_path(Path::new("babelforce.v1/operations/session.initialize.mdx"))
    );
    assert!(!Target::Docs.owns_output_path(Path::new("handwritten.mdx")));

    let first = generate(Target::Docs).unwrap();
    let second = generate(Target::Docs).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 29);
    for file in &first {
        assert!(file.bytes.ends_with(b"\n"));
        assert!(!file.bytes.ends_with(b"\n\n"));
        if file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("mdx")
        {
            assert!(
                file.bytes
                    .windows("DO NOT EDIT".len())
                    .any(|bytes| bytes == b"DO NOT EDIT"),
                "missing generated banner in {}",
                file.path.display()
            );
        }
    }

    let operation = generated_text(&first, "babelforce.v1/operations/session.initialize.mdx");
    assert!(operation.contains("Code generated by rtvbp-spec-gen"));
    assert!(operation.contains("voice → application"));
    assert!(operation.contains("| `audio_codec_offerings` | `AudioCodec[]` | required |"));
    assert!(operation.contains("| `metadata` | `object` | nullable |"));
    assert!(operation.contains("Application that owns the call flow."));
    assert!(operation.contains("### `AudioCodec`"));
    assert!(operation.contains("| `sample_rate` | `integer` | required |"));
    assert!(operation.contains("\"audio_codec_offerings\""));

    let event = generated_text(&first, "babelforce.v1/events/audio.speech.started.mdx");
    assert!(event.contains("application → voice"));
    assert!(event.contains("Side where speech began"));

    let dtmf = generated_text(&first, "babelforce.v1/events/dtmf.mdx");
    assert!(dtmf.contains("| `digit` | `string` | required | length ≥ 1 |"));
    assert!(dtmf.contains("`pressed_at` ≤ `released_at`"));

    let application = generated_text(&first, "babelforce.v1/roles/application.mdx");
    assert!(application.contains("## Must implement"));
    assert!(application.contains("[`session.initialize`](../operations/session.initialize.mdx)"));
    assert!(application.contains("## Emits"));
    assert!(application.contains("[`audio.speech.started`](../events/audio.speech.started.mdx)"));

    let envelope = generated_text(&first, "babelforce.v1/envelopes/classic-v1.mdx");
    assert!(envelope.contains("any non-zero integer"));
    assert!(envelope.contains("both `result` and `error`"));
    assert!(envelope.contains("neither field"));
    assert!(envelope.contains("| `not_implemented` | `501` |"));
    assert!(envelope.contains("event → request → response"));

    let category = generated_text(&first, "babelforce.v1/_category_.json");
    assert!(category.contains("\"label\": \"babelforce.v1\""));
}

#[test]
fn vector_emitter_projects_payloads_envelope_cases_and_typed_scenarios() {
    assert_eq!(Target::from_str("vectors").unwrap(), Target::Vectors);
    assert_eq!(Target::Vectors.canonical_out_dir(), "conformance");
    assert!(Target::Vectors.owns_output_path(Path::new("babelforce.v1/payloads/call.hangup.json")));
    assert!(!Target::Vectors.owns_output_path(Path::new(
        "babelforce.v1/golden/payloads/call.hangup.request.json"
    )));

    let first = generate(Target::Vectors).unwrap();
    let second = generate(Target::Vectors).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 17);

    let payload: Value = serde_json::from_str(generated_text(
        &first,
        "babelforce.v1/payloads/call.hangup.json",
    ))
    .unwrap();
    assert_eq!(payload["method"], "call.hangup");
    assert_eq!(
        payload["request"]["valid"][0]["json"],
        "{\"reason\":\"caller\"}"
    );
    assert!(
        payload["request"]["invalid"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case| case["error"] == "validation")
    );

    let frames: Value = serde_json::from_str(generated_text(
        &first,
        "babelforce.v1/envelope/classic.v1/frames.json",
    ))
    .unwrap();
    assert_eq!(frames["envelope"], "classic.v1");
    assert_eq!(frames["encode"].as_array().unwrap().len(), 10);
    assert!(frames["decode"].as_array().unwrap().iter().any(|case| {
        case["name"] == "structural_precedence" && case["frame"]["kind"] == "event"
    }));
    assert!(frames["invalid"].as_array().unwrap().len() >= 3);

    let initialize: Value = serde_json::from_str(generated_text(
        &first,
        "babelforce.v1/scenarios/initialize-updated-dtmf.json",
    ))
    .unwrap();
    assert_eq!(initialize["roles"]["voice"], "voice");
    assert_eq!(initialize["roles"]["application"], "application");
    assert_eq!(initialize["cases"][0]["steps"][0]["id"], "$init");
    assert_eq!(initialize["cases"][0]["steps"][1]["response"], "$init");

    let termination: Value = serde_json::from_str(generated_text(
        &first,
        "babelforce.v1/scenarios/termination.json",
    ))
    .unwrap();
    assert_eq!(termination["cases"].as_array().unwrap().len(), 3);
    assert_eq!(termination["cases"][2]["steps"][1]["error"]["code"], 501);
}

#[test]
fn every_target_projects_the_loaded_demo_catalog_through_the_common_pipeline() {
    let expected = [
        (Target::Manifest, "demo.v1.catalog.json"),
        (Target::Go, "catalog/demov1/zz_generated.types.go"),
        (Target::Docs, "demo.v1/operations/demo.echo.mdx"),
        (Target::Vectors, "demo.v1/payloads/demo.echo.json"),
    ];
    for (target, path) in expected {
        let files = generate(target).unwrap();
        assert!(
            files.iter().any(|file| file.path == Path::new(path)),
            "{target:?} did not emit {path}"
        );
    }

    let profiles = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../website/docs/profiles.md"),
    )
    .unwrap();
    for fact in [
        "`rtvbp.v1`",
        "reference/babelforce.v1",
        "`rtvbp.demo.v1`",
        "reference/demo.v1",
    ] {
        assert!(profiles.contains(fact), "profiles page is missing {fact}");
    }
}

#[test]
fn docs_emitter_is_catalog_agnostic_and_escapes_mdx_content() {
    let operation = Operation::new::<Request, Response>("demo.render", Role::Application)
        .docs("Render <unsafe> {content} | safely.")
        .example(
            "canonical",
            json!({"input": "<tag>{value}|"}),
            json!({"output": "done"}),
        );
    let event = Event::new::<EventData>("demo.updated", Role::Both)
        .docs("Notify both <peers>.")
        .example("canonical", json!({"state": "ready"}));
    let catalog = Catalog::new("second", 2).operation(operation).event(event);
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();
    let files = emit_docs(&resolve(catalog).unwrap(), &[]).unwrap();

    assert_eq!(files.len(), 5);
    let operation = generated_text(&files, "second.v2/operations/demo.render.mdx");
    assert!(operation.contains("Render &lt;unsafe&gt; &#123;content&#125; | safely."));
    assert!(operation.contains("\"input\": \"<tag>{value}|\""));
    let event = generated_text(&files, "second.v2/events/demo.updated.mdx");
    assert!(event.contains("application ↔ voice"));
    assert!(generated_text(&files, "second.v2/roles/application.mdx").contains("demo.updated"));
    assert!(generated_text(&files, "second.v2/roles/voice.mdx").contains("demo.updated"));
}

fn generated_text<'a>(files: &'a [GeneratedFile], path: &str) -> &'a str {
    std::str::from_utf8(
        &files
            .iter()
            .find(|file| file.path == Path::new(path))
            .unwrap()
            .bytes,
    )
    .unwrap()
}

#[test]
fn go_roles_are_derived_from_synthetic_roles_terminality_and_event_direction() {
    let catalog = Catalog::new("demo", 2)
        .operation(
            Operation::new::<Request, Response>("demo.application", Role::Application)
                .docs("Handled by applications.")
                .reject(rtvbp_spec_model::OperationRejection::new(
                    Role::Voice,
                    409,
                    "voice rejects application operation",
                ))
                .example(
                    "canonical",
                    json!({"input": "in"}),
                    json!({"output": "out"}),
                ),
        )
        .operation(
            Operation::new::<Request, Response>("demo.voice", Role::Voice)
                .docs("Handled by voice peers.")
                .terminal()
                .example(
                    "canonical",
                    json!({"input": "in"}),
                    json!({"output": "out"}),
                ),
        )
        .operation(
            Operation::new::<Request, Response>("demo.both", Role::Both)
                .docs("Handled by both peers.")
                .example(
                    "canonical",
                    json!({"input": "in"}),
                    json!({"output": "out"}),
                ),
        )
        .event(
            Event::new::<EventData>("event.application", Role::Application)
                .docs("Emitted by applications.")
                .example("canonical", json!({"state": "ready"})),
        )
        .event(
            Event::new::<EventData>("event.voice", Role::Voice)
                .docs("Emitted by voice peers.")
                .example("canonical", json!({"state": "ready"})),
        )
        .event(
            Event::new::<EventData>("event.both", Role::Both)
                .docs("Emitted by both peers.")
                .example("canonical", json!({"state": "ready"})),
        );
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();
    let files = emit_go(&resolve(catalog).unwrap()).unwrap();
    let roles = String::from_utf8(
        files
            .iter()
            .find(|file| file.path.ends_with("zz_generated.roles.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();

    let application_handler = section(&roles, "type ApplicationHandler interface", "}\n\n");
    assert!(application_handler.contains("DemoApplication("));
    assert!(application_handler.contains("DemoBoth("));
    assert!(!application_handler.contains("DemoVoice("));
    let voice_handler = section(&roles, "type VoiceHandler interface", "}\n\n");
    assert!(!voice_handler.contains("DemoApplication("));
    assert!(voice_handler.contains("DemoBoth("));
    assert!(voice_handler.contains("DemoVoice("));
    assert!(roles.contains("rtvbp.HandleTerminalRequest(handler.DemoVoice)"));
    assert!(roles.contains("rtvbp.HandleRequest(handler.DemoBoth)"));
    assert!(roles.contains(
        "rtvbp.HandleWithError[*Request](rtvbp.WireError{Code: 409, Message: \"voice rejects application operation\"})"
    ));

    let application_events = section(
        &roles,
        "type ApplicationEvents struct",
        "type VoiceEvents struct",
    );
    assert!(application_events.contains("EventApplication("));
    assert!(application_events.contains("EventBoth("));
    assert!(!application_events.contains("EventVoice("));
    let application_subscriber = section(
        &roles,
        "type ApplicationEventHandler interface",
        "func ApplicationEventHandlers",
    );
    assert!(!application_subscriber.contains("EventApplication("));
    assert!(application_subscriber.contains("EventBoth("));
    assert!(application_subscriber.contains("EventVoice("));
}

#[test]
fn go_validators_are_derived_from_synthetic_structured_metadata() {
    let operation =
        Operation::new::<ValidatedRequest, Response>("demo.validated", Role::Application)
            .docs("Validate a request.")
            .example(
                "canonical",
                json!({"name": "call", "started_at": 1, "finished_at": 2}),
                json!({"output": "ok"}),
            );
    let catalog = Catalog::new("demo", 1).operation(operation);
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();
    let files = emit_go(&resolve(catalog).unwrap()).unwrap();
    let types = String::from_utf8(
        files
            .iter()
            .find(|file| file.path.ends_with("zz_generated.types.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();

    assert!(types.contains("func (value *ValidatedRequest) Validate() error"));
    assert!(types.contains("if len(value.Name) < 1"));
    assert!(types.contains("if value.StartedAt < 0"));
    assert!(types.contains("if value.StartedAt == 0"));
    assert!(types.contains("if value.StartedAt > value.FinishedAt"));
}

#[test]
fn go_roles_reject_colliding_wire_names_on_the_same_surface() {
    let first = Operation::new::<Request, Response>("demo.same_name", Role::Application)
        .docs("First spelling.")
        .example(
            "canonical",
            json!({"input": "in"}),
            json!({"output": "out"}),
        );
    let second = Operation::new::<Request, Response>("demo.same.name", Role::Both)
        .docs("Second spelling.")
        .example(
            "canonical",
            json!({"input": "in"}),
            json!({"output": "out"}),
        );
    let catalog = Catalog::new("collision", 1)
        .operation(first)
        .operation(second);
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();

    let error = emit_go(&resolve(catalog).unwrap()).unwrap_err().to_string();
    assert!(error.contains("ApplicationHandler"), "{error}");
    assert!(error.contains("DemoSameName"), "{error}");
    assert!(error.contains("demo.same_name"), "{error}");
    assert!(error.contains("demo.same.name"), "{error}");
}

fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let source = &source[source.find(start).unwrap()..];
    &source[..source.find(end).unwrap() + end.len()]
}

#[test]
fn go_envelope_emitter_is_driven_by_a_synthetic_second_spec() {
    let envelope = EnvelopeSpec {
        id: "compact__CONSTANTS__.v2".to_owned(),
        constants: vec![ConstantField {
            name: "protocol".to_owned(),
            value: "two__FRAMES__".to_owned(),
        }],
        frames: vec![
            FrameSpec {
                kind: FrameKind::Request,
                discriminator: FieldSpec::required("call__ERROR_SPEC__"),
                id: Some(FieldSpec::required("token")),
                payload: FieldSpec::optional("input"),
                error: None,
            },
            FrameSpec {
                kind: FrameKind::Event,
                discriminator: FieldSpec::required("notice"),
                id: Some(FieldSpec::required("token")),
                payload: FieldSpec::optional("body"),
                error: None,
            },
            FrameSpec {
                kind: FrameKind::Response,
                discriminator: FieldSpec::required("answer"),
                id: None,
                payload: FieldSpec::optional("output"),
                error: Some(FieldSpec::required("failure")),
            },
        ],
        error: ErrorSpec {
            code: FieldSpec::required("status"),
            message: FieldSpec::required("detail"),
            data: FieldSpec::optional("context"),
        },
        error_codes: vec![],
        fixtures: vec![],
    };

    let files = emit_go_envelope(&envelope).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_path())
            .collect::<Vec<_>>(),
        [
            Path::new("envelope/v2compactconstants/zz_generated.codec.go"),
            Path::new("envelope/v2compactconstants/zz_generated.golden_test.go"),
        ]
    );
    let codec = String::from_utf8(files[0].bytes.clone()).unwrap();
    for value in [
        "compact__CONSTANTS__.v2",
        "protocol",
        "two__FRAMES__",
        "call__ERROR_SPEC__",
        "token",
        "input",
        "notice",
        "body",
        "answer",
        "output",
        "failure",
        "status",
        "detail",
        "context",
    ] {
        assert!(codec.contains(value), "missing synthetic value {value}");
    }
    assert!(!codec.contains("classic.v1"));
    assert!(!codec.contains("discriminator: \"event\""));
    assert!(
        codec.find("discriminator: \"call__ERROR_SPEC__\"").unwrap()
            < codec.find("discriminator: \"notice\"").unwrap()
    );
    assert!(codec.contains("error: \"failure\", omitError: false"));
}

#[test]
fn go_cli_check_detects_stale_owned_output_and_generation_removes_only_owned_files() {
    let temp = TempDir::new();
    let binary = env!("CARGO_BIN_EXE_rtvbp-spec-gen");
    let out = format!("--out={}", temp.path().display());
    assert!(
        Command::new(binary)
            .args(["--emit=go", &out])
            .status()
            .unwrap()
            .success()
    );
    fs::write(
        temp.path().join("catalog/babelforcev1/zz_generated.old.go"),
        b"stale\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("catalog/babelforcev1/handwritten.go"),
        b"package babelforcev1\n",
    )
    .unwrap();
    fs::write(
        temp.path()
            .join("envelope/v1classic/zz_generated.obsolete.go"),
        b"stale\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("envelope/v1classic/handwritten.go"),
        b"package v1classic\n",
    )
    .unwrap();
    let stale = Command::new(binary)
        .args(["--emit=go", &out, "--check"])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("zz_generated.old.go"));
    assert!(String::from_utf8_lossy(&stale.stderr).contains("zz_generated.obsolete.go"));
    assert!(
        Command::new(binary)
            .args(["--emit=go", &out])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        !temp
            .path()
            .join("catalog/babelforcev1/zz_generated.old.go")
            .exists()
    );
    assert!(
        temp.path()
            .join("catalog/babelforcev1/handwritten.go")
            .exists()
    );
    assert!(
        !temp
            .path()
            .join("envelope/v1classic/zz_generated.obsolete.go")
            .exists()
    );
    assert!(
        temp.path()
            .join("envelope/v1classic/handwritten.go")
            .exists()
    );
}

fn assert_schema_refs_resolve(value: &Value, schemas: &serde_json::Map<String, Value>) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && let Some(name) = reference.strip_prefix("#/schemas/")
            {
                let name = name.replace("~1", "/").replace("~0", "~");
                assert!(
                    schemas.contains_key(&name),
                    "dangling schema ref {reference}"
                );
            }
            for child in object.values() {
                assert_schema_refs_resolve(child, schemas);
            }
        }
        Value::Array(values) => {
            for child in values {
                assert_schema_refs_resolve(child, schemas);
            }
        }
        _ => {}
    }
}

#[test]
fn committed_manifest_matches_the_pure_emitter() {
    let files = generate(Target::Manifest).unwrap();
    let committed = include_bytes!("../../../manifests/babelforce.v1.catalog.json");

    assert_eq!(files[0].bytes, committed);
}

#[test]
fn writer_is_the_mutating_boundary_and_check_detects_drift_without_rewriting() {
    let temp = TempDir::new();
    let files = vec![GeneratedFile {
        path: PathBuf::from("nested/output.txt"),
        bytes: b"generated\n".to_vec(),
    }];
    let path = temp.path().join("nested/output.txt");

    assert!(check_files(temp.path(), &files).is_err());
    assert!(!path.exists());
    write_files(temp.path(), &files).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"generated\n");
    check_files(temp.path(), &files).unwrap();

    fs::write(&path, b"stale\n").unwrap();
    assert!(check_files(temp.path(), &files).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"stale\n");
    write_files(temp.path(), &files).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"generated\n");
}

#[test]
fn target_owned_check_and_sync_detect_and_remove_stale_generated_files_only() {
    let temp = TempDir::new();
    let files = vec![GeneratedFile {
        path: PathBuf::from("current.catalog.json"),
        bytes: b"current\n".to_vec(),
    }];
    fs::write(temp.path().join("stale.catalog.json"), b"stale\n").unwrap();
    fs::write(temp.path().join("notes.md"), b"handwritten\n").unwrap();

    assert!(
        check_owned_files(temp.path(), &files, |path| Target::Manifest
            .owns_output_path(path))
        .is_err()
    );
    assert_eq!(
        fs::read(temp.path().join("stale.catalog.json")).unwrap(),
        b"stale\n"
    );

    synchronize_files(temp.path(), &files, |path| {
        Target::Manifest.owns_output_path(path)
    })
    .unwrap();
    assert!(!temp.path().join("stale.catalog.json").exists());
    assert_eq!(
        fs::read(temp.path().join("notes.md")).unwrap(),
        b"handwritten\n"
    );
    check_owned_files(temp.path(), &files, |path| {
        Target::Manifest.owns_output_path(path)
    })
    .unwrap();
}

#[test]
fn docs_target_check_and_sync_remove_only_stale_reference_files() {
    let temp = TempDir::new();
    let files = generate(Target::Docs).unwrap();
    synchronize_files(temp.path(), &files, |path| {
        Target::Docs.owns_output_path(path)
    })
    .unwrap();

    let stale = temp.path().join("babelforce.v1/operations/stale.mdx");
    fs::write(&stale, b"stale generated page\n").unwrap();
    let handwritten = temp.path().join("handwritten.mdx");
    fs::write(&handwritten, b"handwritten\n").unwrap();

    assert!(
        check_owned_files(temp.path(), &files, |path| Target::Docs
            .owns_output_path(path))
        .is_err()
    );
    synchronize_files(temp.path(), &files, |path| {
        Target::Docs.owns_output_path(path)
    })
    .unwrap();
    assert!(!stale.exists());
    assert_eq!(fs::read(handwritten).unwrap(), b"handwritten\n");
    check_owned_files(temp.path(), &files, |path| {
        Target::Docs.owns_output_path(path)
    })
    .unwrap();
}

#[test]
fn cli_runs_the_pipeline_and_check_exits_nonzero_on_a_difference() {
    let temp = TempDir::new();
    let binary = env!("CARGO_BIN_EXE_rtvbp-spec-gen");
    let out = format!("--out={}", temp.path().display());

    assert!(
        Command::new(binary)
            .args(["--emit=manifest", &out])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(binary)
            .args(["--emit=manifest", &out, "--check"])
            .status()
            .unwrap()
            .success()
    );

    let manifest = temp.path().join("babelforce.v1.catalog.json");
    fs::write(&manifest, b"stale\n").unwrap();
    let stale = Command::new(binary)
        .args(["--emit=manifest", &out, "--check"])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("generated output differs"));
    assert_eq!(fs::read(manifest).unwrap(), b"stale\n");
}

#[test]
fn bare_check_uses_the_committed_manifest_destination() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtvbp-spec-gen"))
        .arg("--check")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_help_succeeds_and_prints_usage_once() {
    let output = Command::new(env!("CARGO_BIN_EXE_rtvbp-spec-gen"))
        .arg("--help")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(stdout.matches("usage: rtvbp-spec-gen").count(), 1);
    assert!(output.stderr.is_empty());
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rtvbp-spec-gen-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
