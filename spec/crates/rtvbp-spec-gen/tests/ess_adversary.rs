//! Adversary cases for `--emit=ess` (wave spec-first-1, pass 1).

use rtvbp_spec_gen::emit::{ess_variant, render_ess_domain};
use serde_json::{Value, json};

fn manifest(method: &str, description: &str, field: &str) -> Value {
    json!({
        "formatVersion": 1,
        "catalog": {"id": "test.v1"},
        "operations": [{
            "method": method,
            "handledBy": "voice",
            "terminal": false,
            "description": description,
            "request": {"$ref": "#/schemas/Thing"},
            "response": {"$ref": "#/schemas/Thing"},
        }],
        "events": [],
        "schemas": {"Thing": {
            "type": "object",
            "required": [field],
            "properties": {field: {"type": "string"}},
        }},
    })
}

/// `null`, `true` and `false` match `[a-z][a-z0-9]*`, so `variant()` admits them, and the emitter
/// writes them as plain YAML scalars: `{name: null, ...}`. `ess specify validate` then refuses the
/// domain ("invalid type: unit value, expected a string"). The strict rule should refuse them, or
/// the emitter should quote them.
#[test]
fn a_wire_name_that_is_a_yaml_keyword_is_refused() {
    for wire in ["null", "true", "false"] {
        assert!(
            ess_variant(wire).is_err(),
            "{wire:?} is admitted and emitted as `{}`, which YAML reads as a non-string",
            ess_variant(wire).unwrap()
        );
    }
}

/// A field name is written unquoted too: `- name: null` is not a string in YAML.
#[test]
fn a_field_named_like_a_yaml_keyword_is_refused_or_quoted() {
    match render_ess_domain(&manifest("thing.do", "Do it.", "null"), "rtvbp.test_v1") {
        Err(_) => {}
        Ok(text) => assert!(
            !text.lines().any(|line| line == "      - name: null"),
            "field `null` emitted as a YAML null"
        ),
    }
}

/// An operation description goes into a `#` comment verbatim; a newline in it ends the comment and
/// the rest is read as YAML. The catalog model does not refuse a newline in a description.
#[test]
fn a_description_with_a_newline_cannot_inject_yaml() {
    match render_ess_domain(
        &manifest("thing.do", "Do it.\ninjected: true", "a"),
        "rtvbp.test_v1",
    ) {
        Err(_) => {}
        Ok(text) => assert!(
            !text.lines().any(|line| line == "injected: true"),
            "the description's second line became a YAML key"
        ),
    }
}

/// Boundaries of the variant rule that the unit's tests do not name; these stay green.
#[test]
fn variant_rule_boundaries() {
    for refused in [
        "café.x", "a.ß", "é", "a.b ", " a", "a.b\n", "a.b.", "..", ".", "a.-b",
    ] {
        assert!(ess_variant(refused).is_err(), "{refused:?} admitted");
    }
    assert_eq!(ess_variant("a1.b2").unwrap(), "name: A1B2, wire: a1.b2");
    assert_eq!(ess_variant("x").unwrap(), "name: x");
    assert_eq!(ess_variant("a.b.c.d").unwrap(), "name: ABCD, wire: a.b.c.d");
}

/// The variant mapping is injective on its domain: the uppercase letters mark the segment starts
/// and an undotted name stays lowercase. So the collision refusal is reachable only by a repeated
/// wire name. Exhaustive over every admitted name of up to 5 characters over `{a, b, 1, .}`.
#[test]
fn variant_mapping_is_injective() {
    let alphabet = ['a', 'b', '1', '.'];
    let mut seen = std::collections::BTreeMap::<String, String>::new();
    let mut admitted = 0;
    let mut stack = vec![String::new()];
    while let Some(prefix) = stack.pop() {
        if !prefix.is_empty()
            && let Ok(variant) = ess_variant(&prefix)
        {
            admitted += 1;
            let name = variant
                .strip_prefix("name: ")
                .unwrap()
                .split(',')
                .next()
                .unwrap()
                .to_owned();
            if let Some(earlier) = seen.insert(name.clone(), prefix.clone()) {
                panic!("{earlier:?} and {prefix:?} both map to {name}");
            }
        }
        if prefix.len() < 5 {
            for c in alphabet {
                stack.push(format!("{prefix}{c}"));
            }
        }
    }
    assert!(admitted > 100, "only {admitted} names admitted");
}
