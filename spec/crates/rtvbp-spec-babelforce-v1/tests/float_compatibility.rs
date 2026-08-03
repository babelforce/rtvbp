use std::collections::BTreeMap;
use std::path::PathBuf;

use rtvbp_spec_babelforce_v1::AudioInfoItem;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoFloat64Boundary {
    name: String,
    bits: String,
    #[serde(default)]
    json: Option<String>,
    #[serde(default)]
    rejects: bool,
}

fn authority() -> BTreeMap<String, GoFloat64Boundary> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../conformance/babelforce.v1/authority/go-float64-boundaries.json");
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "read pinned Go float64 boundary capture {}: {error}",
            path.display()
        )
    });
    let cases: Vec<GoFloat64Boundary> = serde_json::from_slice(&bytes).unwrap();
    cases
        .into_iter()
        .map(|case| (case.name.clone(), case))
        .collect()
}

fn value(case: &GoFloat64Boundary) -> f64 {
    f64::from_bits(u64::from_str_radix(&case.bits, 16).unwrap())
}

fn rust_json(value: f64) -> String {
    serde_json::to_string(&AudioInfoItem {
        bytes: 0,
        bytes_per_second: value,
        bytes_total: 0,
    })
    .unwrap()
}

fn wrapped(scalar: &str) -> String {
    format!(r#"{{"bytes":0,"bytes_per_second":{scalar},"bytes_total":0}}"#)
}

#[test]
fn deployed_nonnegative_float64_envelope_matches_pinned_go_encoding() {
    let authority = authority();

    // The supported deployed envelope is +0 or 1e-5 <= value <= 2^53. These cases pin both
    // inclusive boundaries and a representative fraction.
    for name in [
        "positive-zero",
        "one-e-minus-five",
        "canonical-fraction",
        "two-to-53",
    ] {
        let case = &authority[name];
        assert!(!case.rejects, "{name} unexpectedly rejected by Go");
        assert_eq!(rust_json(value(case)), wrapped(case.json.as_ref().unwrap()));
    }
}

#[test]
fn values_outside_the_deployed_envelope_have_explicit_go_witnesses() {
    let authority = authority();

    // Some disconnected values happen to share a spelling today, but are intentionally outside
    // the supported deployed-rate envelope.
    for name in ["one-e-minus-seven", "above-two-to-53", "one-e21"] {
        let case = &authority[name];
        assert_eq!(rust_json(value(case)), wrapped(case.json.as_ref().unwrap()));
    }

    // These are the known notation, signed-zero, and integral-cast incompatibilities.
    for name in [
        "negative-zero",
        "one-e-minus-six",
        "below-one-e-minus-five",
        "below-two-to-63",
        "two-to-63",
        "one-e19",
        "below-one-e21",
    ] {
        let case = &authority[name];
        assert_ne!(rust_json(value(case)), wrapped(case.json.as_ref().unwrap()));
    }

    // encoding/json rejects non-finite float64 values; serde_json normalizes them to null.
    for name in ["not-a-number", "positive-infinity"] {
        let case = &authority[name];
        assert!(case.rejects, "{name} unexpectedly accepted by Go");
        assert_eq!(rust_json(value(case)), wrapped("null"));
    }
}
