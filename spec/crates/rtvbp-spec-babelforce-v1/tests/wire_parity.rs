use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rtvbp_spec_babelforce_v1::catalog;
use rtvbp_spec_model::classic_v1;

const DEPLOYED_EVENTS: [&str; 5] = [
    "audio.info",
    "audio.speech.started",
    "call.hangup",
    "dtmf",
    "session.updated",
];

const ENVELOPES: [&str; 4] = [
    "event.json",
    "request.json",
    "response-error.json",
    "response-ok.json",
];

fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../conformance/babelforce.v1/golden")
}

fn golden(relative: &str) -> Vec<u8> {
    fs::read(golden_root().join(relative)).unwrap()
}

#[test]
fn every_frozen_fixture_is_owned_by_the_bidirectional_spec_proof() {
    let catalog = catalog();
    let mut expected = BTreeSet::new();
    for operation in &catalog.operations {
        expected.insert(format!("payloads/{}.request.json", operation.method));
        expected.insert(format!("payloads/{}.response.json", operation.method));
    }
    for event in DEPLOYED_EVENTS {
        expected.insert(format!("events/{event}.json"));
    }
    for envelope in ENVELOPES {
        expected.insert(format!("envelope/classic.v1/{envelope}"));
    }

    assert_eq!(fixture_inventory(&golden_root()), expected);
    assert_eq!(expected.len(), 29);
}

#[test]
fn every_payload_fixture_deserializes_and_reserializes_to_identical_bytes() {
    let catalog = catalog();

    for operation in &catalog.operations {
        let request_name = format!("payloads/{}.request.json", operation.method);
        let request = golden(&request_name);
        assert_eq!(
            operation.request.round_trip_bytes(&request).unwrap(),
            request,
            "{request_name}"
        );

        let response_name = format!("payloads/{}.response.json", operation.method);
        let response = golden(&response_name);
        assert_eq!(
            operation.response.round_trip_bytes(&response).unwrap(),
            response,
            "{response_name}"
        );
    }

    for name in DEPLOYED_EVENTS {
        let event = catalog
            .events
            .iter()
            .find(|event| event.name == name)
            .unwrap();
        let fixture_name = format!("events/{name}.json");
        let fixture = golden(&fixture_name);
        assert_eq!(
            event.data.round_trip_bytes(&fixture).unwrap(),
            fixture,
            "{fixture_name}"
        );
    }
}

#[test]
fn every_envelope_fixture_decodes_and_reencodes_to_identical_bytes() {
    let codec = classic_v1();

    for name in ENVELOPES {
        let fixture_name = format!("envelope/classic.v1/{name}");
        let fixture = golden(&fixture_name);
        let frame = codec.decode(&fixture).unwrap();
        assert_eq!(codec.encode(&frame).unwrap(), fixture, "{fixture_name}");
    }
}

fn fixture_inventory(root: &Path) -> BTreeSet<String> {
    let mut inventory = BTreeSet::new();
    visit(root, root, &mut inventory);
    inventory
}

fn visit(root: &Path, directory: &Path, inventory: &mut BTreeSet<String>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            visit(root, &path, inventory);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            inventory.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}
