use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use rtvbp_spec_gen::catalogs;
use rtvbp_spec_gen::emit::{GeneratedFile, Target};
use rtvbp_spec_gen::resolve::resolve;
use rtvbp_spec_gen::write::{check_files, check_owned_files, synchronize_files, write_files};
use rtvbp_spec_gen::{ResolveError, generate};
use rtvbp_spec_model::{Catalog, Event, Operation, Role};
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
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, Path::new("babelforce.v1.catalog.json"));
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
    assert!(first[0].bytes.ends_with(b"\n"));
    assert!(!first[0].bytes.ends_with(b"\n\n"));
}

#[test]
fn go_emitter_pins_names_presence_order_docs_and_all_golden_cases() {
    assert_eq!(Target::from_str("go").unwrap(), Target::Go);
    assert_eq!(Target::Go.canonical_out_dir(), "sdk/go/catalog");
    assert!(Target::Go.owns_output_path(Path::new("babelforcev1/zz_generated.types.go")));
    assert!(!Target::Go.owns_output_path(Path::new("babelforcev1/handwritten.go")));

    let first = generate(Target::Go).unwrap();
    let second = generate(Target::Go).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
    assert_eq!(
        first
            .iter()
            .map(|file| file.path.as_path())
            .collect::<Vec<_>>(),
        [
            Path::new("babelforcev1/zz_generated.golden_test.go"),
            Path::new("babelforcev1/zz_generated.types.go"),
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
    assert!(types.contains("type SessionGetResponse map[string]any"));
    assert!(types.contains("const MethodSessionInitialize = \"session.initialize\""));
    assert!(types.contains("func (*SessionUpdatedEvent) EventName() string"));
    assert!(types.contains("Application that owns the call flow."));
    assert!(
        types.find("Application AppInfo").unwrap() < types.find("Call CallInfo").unwrap()
            && types.find("Call CallInfo").unwrap()
                < types.find("AudioCodecOfferings []AudioCodec").unwrap()
    );

    let tests = String::from_utf8(
        first
            .iter()
            .find(|file| file.path.ends_with("zz_generated.golden_test.go"))
            .unwrap()
            .bytes
            .clone(),
    )
    .unwrap();
    assert_eq!(tests.matches("\t{name: \"").count(), 36);
    assert!(tests.contains("/round_trip"));
    assert!(tests.contains("/construct"));
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
        temp.path().join("babelforcev1/zz_generated.old.go"),
        b"stale\n",
    )
    .unwrap();
    fs::write(
        temp.path().join("babelforcev1/handwritten.go"),
        b"package babelforcev1\n",
    )
    .unwrap();
    let stale = Command::new(binary)
        .args(["--emit=go", &out, "--check"])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("zz_generated.old.go"));
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
            .join("babelforcev1/zz_generated.old.go")
            .exists()
    );
    assert!(temp.path().join("babelforcev1/handwritten.go").exists());
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
