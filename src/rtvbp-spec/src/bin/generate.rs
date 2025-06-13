use rtvbp_spec::v1::docs::generate::{async_api_schema, json_schema};
use serde_json::json;
use std::fs;

pub fn main() {
    let out_dir = format!("{}/../../public/rtvbp-spec", env!("CARGO_MANIFEST_DIR"));

    // jsonschema
    fs::write(
        format!("{}/schema.json", out_dir),
        serde_json::to_string_pretty(&json!(&json_schema())).expect("failed"),
    )
    .expect("failed to write schema");

    // asyncapi.yaml
    fs::write(
        format!("{}/asyncapi.yaml", out_dir),
        serde_yaml::to_string(&json!(&async_api_schema())).expect("failed"),
    )
    .expect("failed to write schema");
}
