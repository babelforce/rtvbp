use std::path::Path;

use rtvbp_spec_gen::emit::{Target, emit_manifest};
use rtvbp_spec_gen::resolve::resolve;
use rtvbp_spec_gen::{catalogs, generate};
use rtvbp_spec_model::{Catalog, Operation, OperationRejection, Role};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct Request {
    value: String,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct Response {}

fn operation() -> Operation {
    Operation::new::<Request, Response>("demo.run", Role::Application)
        .docs("Run the demo operation.")
        .reject(OperationRejection::new(
            Role::Voice,
            501,
            "demo.run is not supported by voice",
        ))
        .example("canonical", json!({"value": "hello"}), json!({}))
}

#[test]
fn resolve_preserves_target_neutral_per_role_rejections() {
    let catalog = Catalog::new("demo", 1).operation(operation());
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();

    let resolved = resolve(catalog).unwrap();
    assert_eq!(
        resolved.operations[0].rejections,
        [OperationRejection::new(
            Role::Voice,
            501,
            "demo.run is not supported by voice"
        )]
    );
}

#[test]
fn manifest_emits_rejections_and_structured_validation_metadata() {
    let files = generate(Target::Manifest).unwrap();
    assert_eq!(files[0].path, Path::new("babelforce.v1.catalog.json"));
    let manifest: Value = serde_json::from_slice(&files[0].bytes).unwrap();

    let terminate = manifest["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["method"] == "session.terminate")
        .unwrap();
    assert_eq!(
        terminate["rejections"],
        json!([{
            "role": "voice",
            "code": 501,
            "message": "session.terminate is not supported. please use application.move or call.hangup instead"
        }])
    );

    let schemas = &manifest["schemas"];
    assert_eq!(
        schemas["SessionTerminateRequest"]["properties"]["reason"]["minLength"],
        1
    );
    assert_eq!(
        schemas["PingRequest"]["properties"]["t0"]["x-rtvbp-nonzero"],
        true
    );
    assert_eq!(schemas["DtmfEvent"]["properties"]["seq"]["minimum"], 0);
    assert_eq!(
        schemas["DtmfEvent"]["x-rtvbp-field-order"],
        json!([{"lower": "pressed_at", "upper": "released_at"}])
    );
}

#[test]
fn standalone_manifest_emitter_includes_resolved_rejections() {
    let catalog = Catalog::new("demo", 1).operation(operation());
    catalogs::validate(std::slice::from_ref(&catalog)).unwrap();
    let files = emit_manifest(&resolve(catalog).unwrap()).unwrap();
    let manifest: Value = serde_json::from_slice(&files[0].bytes).unwrap();

    assert_eq!(manifest["operations"][0]["rejections"][0]["role"], "voice");
}
