use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rtvbp_spec_babelforce_v1::catalog;
use rtvbp_spec_model::classic_v1;

const EVENTS: [&str; 9] = [
    "agent.tool.call",
    "audio.info",
    "audio.speech.started",
    "call.hangup",
    "dtmf",
    "input.transcript",
    "output.transcript.delta",
    "output.transcript.done",
    "session.updated",
];

const ENVELOPES: [&str; 10] = [
    "event.json",
    "request.json",
    "request-with-params.json",
    "response-error-internal.json",
    "response-error-not-implemented.json",
    "response-error-unknown.json",
    "response-error.json",
    "response-ok-no-result.json",
    "response-ok-null-result.json",
    "response-ok.json",
];

const PAYLOAD_VARIANTS: [(&str, &str, bool); 5] = [
    (
        "variants/payloads/application.move.request-empty.json",
        "application.move",
        true,
    ),
    (
        "variants/payloads/application.move.response-no-next.json",
        "application.move",
        false,
    ),
    (
        "variants/payloads/ping.request-no-optionals.json",
        "ping",
        true,
    ),
    (
        "variants/payloads/ping.response-no-data.json",
        "ping",
        false,
    ),
    (
        "variants/payloads/recording.start.request-no-tags.json",
        "recording.start",
        true,
    ),
];

const EVENT_VARIANTS: [(&str, &str); 2] = [
    ("variants/events/audio.info-nonzero.json", "audio.info"),
    ("variants/events/call.hangup-no-reason.json", "call.hangup"),
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
    for event in EVENTS {
        expected.insert(format!("events/{event}.json"));
    }
    for envelope in ENVELOPES {
        expected.insert(format!("envelope/classic.v1/{envelope}"));
    }
    expected.extend(PAYLOAD_VARIANTS.map(|(path, _, _)| path.to_owned()));
    expected.extend(EVENT_VARIANTS.map(|(path, _)| path.to_owned()));

    assert_eq!(fixture_inventory(&golden_root()), expected);
    assert_eq!(expected.len(), 46);
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

    for name in EVENTS {
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

    for (fixture_name, method, request) in PAYLOAD_VARIANTS {
        let operation = catalog
            .operations
            .iter()
            .find(|operation| operation.method == method)
            .unwrap();
        let fixture = golden(fixture_name);
        let payload_type = if request {
            &operation.request
        } else {
            &operation.response
        };
        assert_eq!(
            payload_type.round_trip_bytes(&fixture).unwrap(),
            fixture,
            "{fixture_name}"
        );
    }

    for (fixture_name, name) in EVENT_VARIANTS {
        let event = catalog
            .events
            .iter()
            .find(|event| event.name == name)
            .unwrap();
        let fixture = golden(fixture_name);
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
