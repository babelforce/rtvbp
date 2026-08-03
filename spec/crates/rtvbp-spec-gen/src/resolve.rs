use std::collections::BTreeMap;

use rtvbp_spec_model::{Catalog, CatalogId, EventExample, OperationExample, Role, TypeRef};
use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCatalog {
    pub id: CatalogId,
    pub operations: Vec<ResolvedOperation>,
    pub events: Vec<ResolvedEvent>,
    pub schemas: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOperation {
    pub method: String,
    pub handled_by: Role,
    pub terminal: bool,
    pub docs: String,
    pub request: String,
    pub response: String,
    pub examples: Vec<OperationExample>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedEvent {
    pub name: String,
    pub emitted_by: Role,
    pub docs: String,
    pub data: String,
    pub examples: Vec<EventExample>,
}

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("operation {name:?} has no role after validation")]
    MissingOperationRole { name: String },
    #[error("event {name:?} has no role after validation")]
    MissingEventRole { name: String },
    #[error("{kind} {name:?} has no documentation after validation")]
    MissingDocumentation { kind: &'static str, name: String },
    #[error("schema name {name:?} resolves to conflicting definitions")]
    ConflictingSchema { name: String },
}

/// Resolve one validated catalog into the complete, emitter-facing model.
pub fn resolve(catalog: Catalog) -> Result<ResolvedCatalog, ResolveError> {
    let mut schemas = BTreeMap::new();
    let mut operations = Vec::with_capacity(catalog.operations.len());
    for mut operation in catalog.operations {
        register_type(&mut schemas, &operation.request)?;
        register_type(&mut schemas, &operation.response)?;
        operation
            .examples
            .sort_by(|left, right| left.name.cmp(&right.name));
        operations.push(ResolvedOperation {
            method: operation.method.clone(),
            handled_by: operation
                .handled_by
                .ok_or_else(|| ResolveError::MissingOperationRole {
                    name: operation.method.clone(),
                })?,
            terminal: operation.terminal,
            docs: operation.docs.ok_or(ResolveError::MissingDocumentation {
                kind: "operation",
                name: operation.method,
            })?,
            request: operation.request.name,
            response: operation.response.name,
            examples: operation.examples,
        });
    }
    operations.sort_by(|left, right| left.method.cmp(&right.method));

    let mut events = Vec::with_capacity(catalog.events.len());
    for mut event in catalog.events {
        register_type(&mut schemas, &event.data)?;
        event
            .examples
            .sort_by(|left, right| left.name.cmp(&right.name));
        events.push(ResolvedEvent {
            name: event.name.clone(),
            emitted_by: event
                .emitted_by
                .ok_or_else(|| ResolveError::MissingEventRole {
                    name: event.name.clone(),
                })?,
            docs: event.docs.ok_or(ResolveError::MissingDocumentation {
                kind: "event",
                name: event.name,
            })?,
            data: event.data.name,
            examples: event.examples,
        });
    }
    events.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(ResolvedCatalog {
        id: catalog.id,
        operations,
        events,
        schemas,
    })
}

fn register_type(
    schemas: &mut BTreeMap<String, Value>,
    type_ref: &TypeRef,
) -> Result<(), ResolveError> {
    let mut root = type_ref.schema.clone().to_value();
    let definitions = root
        .as_object_mut()
        .and_then(|object| object.remove("$defs"))
        .and_then(|definitions| definitions.as_object().cloned())
        .unwrap_or_default();

    if let Some(object) = root.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
    }

    rewrite_local_refs(&mut root);
    register_schema(schemas, type_ref.name.clone(), root)?;
    for (name, mut schema) in definitions {
        rewrite_local_refs(&mut schema);
        register_schema(schemas, name, schema)?;
    }
    Ok(())
}

fn register_schema(
    schemas: &mut BTreeMap<String, Value>,
    name: String,
    schema: Value,
) -> Result<(), ResolveError> {
    match schemas.get(&name) {
        Some(existing) if existing != &schema => Err(ResolveError::ConflictingSchema { name }),
        Some(_) => Ok(()),
        None => {
            schemas.insert(name, schema);
            Ok(())
        }
    }
}

fn rewrite_local_refs(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref")
                && let Some(name) = reference.strip_prefix("#/$defs/")
            {
                *reference = format!("#/schemas/{name}");
            }
            for child in object.values_mut() {
                rewrite_local_refs(child);
            }
        }
        Value::Array(values) => {
            for value in values {
                rewrite_local_refs(value);
            }
        }
        _ => {}
    }
}
