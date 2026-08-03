use std::collections::HashSet;
use std::fmt;
use std::path::{Component, Path};

use schemars::{JsonSchema, Schema, schema_for};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Method prefix reserved for envelope-independent transport signaling.
pub const RESERVED_TRANSPORT_METHOD_PREFIX: &str = "transport.";

/// A versioned payload catalog and all operations and events it declares.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub id: CatalogId,
    pub operations: Vec<Operation>,
    pub events: Vec<Event>,
    pub fixtures: Vec<CatalogFixture>,
}

impl Catalog {
    #[must_use]
    pub fn new(name: impl Into<String>, major: u32) -> Self {
        Self {
            id: CatalogId::new(name, major),
            operations: Vec::new(),
            events: Vec::new(),
            fixtures: Vec::new(),
        }
    }

    #[must_use]
    pub fn operation(mut self, operation: Operation) -> Self {
        self.operations.push(operation);
        self
    }

    #[must_use]
    pub fn event(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }

    #[must_use]
    pub fn fixtures(mut self, fixtures: impl IntoIterator<Item = CatalogFixture>) -> Self {
        self.fixtures.extend(fixtures);
        self
    }

    /// Validate registry uniqueness, documentation, and typed examples.
    pub fn validate(&self) -> Result<(), CatalogValidationErrors> {
        let mut issues = Vec::new();
        let mut operation_names = HashSet::new();
        for operation in &self.operations {
            if operation.handled_by.is_none() {
                issues.push(CatalogValidationError::MissingRole {
                    kind: CatalogItemKind::Operation,
                    name: operation.method.clone(),
                });
            }
            if operation
                .method
                .starts_with(RESERVED_TRANSPORT_METHOD_PREFIX)
            {
                issues.push(CatalogValidationError::ReservedOperationNamespace {
                    method: operation.method.clone(),
                });
            }
            if !operation_names.insert(operation.method.as_str()) {
                issues.push(CatalogValidationError::DuplicateOperation {
                    method: operation.method.clone(),
                });
            }
            validate_item_metadata(
                &mut issues,
                CatalogItemKind::Operation,
                &operation.method,
                operation.docs.as_deref(),
                operation
                    .examples
                    .iter()
                    .map(|example| example.name.as_str()),
            );
            for example in &operation.examples {
                validate_example_value(
                    &mut issues,
                    CatalogItemKind::Operation,
                    &operation.method,
                    &example.name,
                    ExampleSide::Request,
                    &operation.request,
                    &example.request,
                );
                validate_example_value(
                    &mut issues,
                    CatalogItemKind::Operation,
                    &operation.method,
                    &example.name,
                    ExampleSide::Response,
                    &operation.response,
                    &example.response,
                );
            }
        }

        let mut event_names = HashSet::new();
        for event in &self.events {
            if event.emitted_by.is_none() {
                issues.push(CatalogValidationError::MissingRole {
                    kind: CatalogItemKind::Event,
                    name: event.name.clone(),
                });
            }
            if !event_names.insert(event.name.as_str()) {
                issues.push(CatalogValidationError::DuplicateEvent {
                    name: event.name.clone(),
                });
            }
            validate_item_metadata(
                &mut issues,
                CatalogItemKind::Event,
                &event.name,
                event.docs.as_deref(),
                event.examples.iter().map(|example| example.name.as_str()),
            );
            for example in &event.examples {
                validate_example_value(
                    &mut issues,
                    CatalogItemKind::Event,
                    &event.name,
                    &example.name,
                    ExampleSide::EventData,
                    &event.data,
                    &example.data,
                );
            }
        }

        let mut fixture_paths = HashSet::new();
        for fixture in &self.fixtures {
            if fixture.path.is_empty()
                || !Path::new(&fixture.path)
                    .components()
                    .all(|component| matches!(component, Component::Normal(_)))
            {
                issues.push(CatalogValidationError::InvalidFixturePath {
                    path: fixture.path.clone(),
                });
            }
            if !fixture_paths.insert(fixture.path.as_str()) {
                issues.push(CatalogValidationError::DuplicateFixturePath {
                    path: fixture.path.clone(),
                });
            }
            let payload_type = match &fixture.target {
                FixtureTarget::OperationRequest { method } => self
                    .operations
                    .iter()
                    .find(|operation| operation.method == *method)
                    .map(|operation| &operation.request),
                FixtureTarget::OperationResponse { method } => self
                    .operations
                    .iter()
                    .find(|operation| operation.method == *method)
                    .map(|operation| &operation.response),
                FixtureTarget::Event { name } => self
                    .events
                    .iter()
                    .find(|event| event.name == *name)
                    .map(|event| &event.data),
            };
            let Some(payload_type) = payload_type else {
                issues.push(CatalogValidationError::UnknownFixtureTarget {
                    path: fixture.path.clone(),
                    target: fixture.target.clone(),
                });
                continue;
            };
            match payload_type.round_trip_bytes(&fixture.bytes) {
                Ok(actual) if actual != fixture.bytes => {
                    issues.push(CatalogValidationError::FixtureChanged {
                        path: fixture.path.clone(),
                        expected: fixture.bytes.clone(),
                        actual,
                    });
                }
                Ok(_) => {}
                Err(source) => issues.push(CatalogValidationError::FixtureRoundTrip {
                    path: fixture.path.clone(),
                    payload_type: payload_type.name.clone(),
                    source,
                }),
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(CatalogValidationErrors { issues })
        }
    }
}

/// The stable `name.vMAJOR` identity of a payload catalog.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct CatalogId {
    pub name: String,
    pub major: u32,
}

impl CatalogId {
    #[must_use]
    pub fn new(name: impl Into<String>, major: u32) -> Self {
        Self {
            name: name.into(),
            major,
        }
    }
}

impl fmt::Display for CatalogId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.v{}", self.name, self.major)
    }
}

/// The peer role responsible for an operation or event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Voice,
    Application,
    Both,
}

/// A named Rust payload type and its complete JSON Schema.
#[derive(Clone)]
pub struct TypeRef {
    pub name: String,
    pub schema: Schema,
    round_trip: fn(&Value) -> serde_json::Result<Value>,
    round_trip_bytes: fn(&[u8]) -> serde_json::Result<Vec<u8>>,
}

impl TypeRef {
    #[must_use]
    pub fn of<T>() -> Self
    where
        T: DeserializeOwned + JsonSchema + Serialize,
    {
        Self {
            name: T::schema_name().into_owned(),
            schema: schema_for!(T),
            round_trip: round_trip::<T>,
            round_trip_bytes: round_trip_bytes::<T>,
        }
    }

    /// Deserialize and reserialize a JSON value through the concrete payload type.
    pub fn round_trip(&self, value: &Value) -> serde_json::Result<Value> {
        (self.round_trip)(value)
    }

    /// Deserialize and reserialize bytes through the concrete payload type.
    pub fn round_trip_bytes(&self, bytes: &[u8]) -> serde_json::Result<Vec<u8>> {
        (self.round_trip_bytes)(bytes)
    }
}

impl fmt::Debug for TypeRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypeRef")
            .field("name", &self.name)
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

impl PartialEq for TypeRef {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.schema == other.schema
    }
}

/// A request/response operation in a catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    pub method: String,
    pub handled_by: Option<Role>,
    pub request: TypeRef,
    pub response: TypeRef,
    pub terminal: bool,
    pub docs: Option<String>,
    pub examples: Vec<OperationExample>,
}

impl Operation {
    #[must_use]
    pub fn new<Request, Response>(method: impl Into<String>, handled_by: Role) -> Self
    where
        Request: DeserializeOwned + JsonSchema + Serialize,
        Response: DeserializeOwned + JsonSchema + Serialize,
    {
        Self {
            method: method.into(),
            handled_by: Some(handled_by),
            request: TypeRef::of::<Request>(),
            response: TypeRef::of::<Response>(),
            terminal: false,
            docs: None,
            examples: Vec::new(),
        }
    }

    #[must_use]
    pub fn terminal(mut self) -> Self {
        self.terminal = true;
        self
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn example(mut self, name: impl Into<String>, request: Value, response: Value) -> Self {
        self.examples.push(OperationExample {
            name: name.into(),
            request,
            response,
        });
        self
    }
}

/// Canonical payload values for one operation example.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperationExample {
    pub name: String,
    pub request: Value,
    pub response: Value,
}

/// A fire-and-forget event in a catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub name: String,
    pub emitted_by: Option<Role>,
    pub data: TypeRef,
    pub docs: Option<String>,
    pub examples: Vec<EventExample>,
}

impl Event {
    #[must_use]
    pub fn new<Data>(name: impl Into<String>, emitted_by: Role) -> Self
    where
        Data: DeserializeOwned + JsonSchema + Serialize,
    {
        Self {
            name: name.into(),
            emitted_by: Some(emitted_by),
            data: TypeRef::of::<Data>(),
            docs: None,
            examples: Vec::new(),
        }
    }

    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    #[must_use]
    pub fn example(mut self, name: impl Into<String>, data: Value) -> Self {
        self.examples.push(EventExample {
            name: name.into(),
            data,
        });
        self
    }
}

/// Canonical payload data for one event example.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EventExample {
    pub name: String,
    pub data: Value,
}

/// One frozen payload/event fixture and the catalog type that owns it.
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogFixture {
    pub path: String,
    pub target: FixtureTarget,
    pub bytes: Vec<u8>,
}

impl CatalogFixture {
    #[must_use]
    pub fn operation_request(
        method: impl Into<String>,
        path: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            path: path.into(),
            target: FixtureTarget::OperationRequest {
                method: method.into(),
            },
            bytes: bytes.into(),
        }
    }

    #[must_use]
    pub fn operation_response(
        method: impl Into<String>,
        path: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            path: path.into(),
            target: FixtureTarget::OperationResponse {
                method: method.into(),
            },
            bytes: bytes.into(),
        }
    }

    #[must_use]
    pub fn event(
        name: impl Into<String>,
        path: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            path: path.into(),
            target: FixtureTarget::Event { name: name.into() },
            bytes: bytes.into(),
        }
    }
}

/// The catalog request, response, or event type exercised by a fixture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixtureTarget {
    OperationRequest { method: String },
    OperationResponse { method: String },
    Event { name: String },
}

impl fmt::Display for FixtureTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OperationRequest { method } => write!(formatter, "operation {method:?} request"),
            Self::OperationResponse { method } => {
                write!(formatter, "operation {method:?} response")
            }
            Self::Event { name } => write!(formatter, "event {name:?} data"),
        }
    }
}

/// All issues found while validating one catalog.
#[derive(Debug)]
pub struct CatalogValidationErrors {
    pub issues: Vec<CatalogValidationError>,
}

impl fmt::Display for CatalogValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "catalog validation failed with {} issue(s):",
            self.issues.len()
        )?;
        for issue in &self.issues {
            writeln!(formatter, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CatalogValidationErrors {}

/// One actionable catalog validation issue.
#[derive(Debug, Error)]
pub enum CatalogValidationError {
    #[error("operation method {method:?} claims the reserved transport.* namespace")]
    ReservedOperationNamespace { method: String },
    #[error("duplicate operation method {method:?}")]
    DuplicateOperation { method: String },
    #[error("duplicate event name {name:?}")]
    DuplicateEvent { name: String },
    #[error("{kind} {name:?} has no role")]
    MissingRole { kind: CatalogItemKind, name: String },
    #[error("duplicate conformance fixture path {path:?}")]
    DuplicateFixturePath { path: String },
    #[error("conformance fixture path must be relative and confined: {path:?}")]
    InvalidFixturePath { path: String },
    #[error("conformance fixture {path:?} refers to unknown {target}")]
    UnknownFixtureTarget { path: String, target: FixtureTarget },
    #[error("conformance fixture {path:?} does not match {payload_type}: {source}")]
    FixtureRoundTrip {
        path: String,
        payload_type: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("conformance fixture {path:?} changes after typed round-trip")]
    FixtureChanged {
        path: String,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    #[error("{kind} {name:?} has no documentation")]
    MissingDocumentation { kind: CatalogItemKind, name: String },
    #[error("{kind} {name:?} has no canonical examples")]
    MissingExamples { kind: CatalogItemKind, name: String },
    #[error("{kind} {name:?} has a blank canonical example name")]
    BlankExampleName { kind: CatalogItemKind, name: String },
    #[error("{kind} {name:?} has duplicate canonical example {example:?}")]
    DuplicateExampleName {
        kind: CatalogItemKind,
        name: String,
        example: String,
    },
    #[error(
        "{kind} {name:?} canonical example {example:?} {side} does not match {payload_type}: {source}"
    )]
    ExampleRoundTrip {
        kind: CatalogItemKind,
        name: String,
        example: String,
        side: ExampleSide,
        payload_type: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "{kind} {name:?} canonical example {example:?} {side} changes after typed round-trip: expected {expected}, got {actual}"
    )]
    ExampleChanged {
        kind: CatalogItemKind,
        name: String,
        example: String,
        side: ExampleSide,
        expected: Value,
        actual: Value,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogItemKind {
    Operation,
    Event,
}

impl fmt::Display for CatalogItemKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Operation => "operation",
            Self::Event => "event",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExampleSide {
    Request,
    Response,
    EventData,
}

impl fmt::Display for ExampleSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::EventData => "event data",
        })
    }
}

fn round_trip<T>(value: &Value) -> serde_json::Result<Value>
where
    T: DeserializeOwned + Serialize,
{
    serde_json::from_value::<T>(value.clone()).and_then(serde_json::to_value)
}

fn round_trip_bytes<T>(bytes: &[u8]) -> serde_json::Result<Vec<u8>>
where
    T: DeserializeOwned + Serialize,
{
    serde_json::from_slice::<T>(bytes).and_then(|value| serde_json::to_vec(&value))
}

fn validate_item_metadata<'a>(
    issues: &mut Vec<CatalogValidationError>,
    kind: CatalogItemKind,
    name: &str,
    docs: Option<&str>,
    example_names: impl Iterator<Item = &'a str>,
) {
    if docs.is_none_or(|docs| docs.trim().is_empty()) {
        issues.push(CatalogValidationError::MissingDocumentation {
            kind,
            name: name.to_owned(),
        });
    }

    let mut seen = HashSet::new();
    let mut count = 0;
    for example in example_names {
        count += 1;
        if example.trim().is_empty() {
            issues.push(CatalogValidationError::BlankExampleName {
                kind,
                name: name.to_owned(),
            });
        } else if !seen.insert(example) {
            issues.push(CatalogValidationError::DuplicateExampleName {
                kind,
                name: name.to_owned(),
                example: example.to_owned(),
            });
        }
    }
    if count == 0 {
        issues.push(CatalogValidationError::MissingExamples {
            kind,
            name: name.to_owned(),
        });
    }
}

fn validate_example_value(
    issues: &mut Vec<CatalogValidationError>,
    kind: CatalogItemKind,
    name: &str,
    example: &str,
    side: ExampleSide,
    payload_type: &TypeRef,
    value: &Value,
) {
    match payload_type.round_trip(value) {
        Ok(actual) if actual != *value => issues.push(CatalogValidationError::ExampleChanged {
            kind,
            name: name.to_owned(),
            example: example.to_owned(),
            side,
            expected: value.clone(),
            actual,
        }),
        Ok(_) => {}
        Err(source) => issues.push(CatalogValidationError::ExampleRoundTrip {
            kind,
            name: name.to_owned(),
            example: example.to_owned(),
            side,
            payload_type: payload_type.name.clone(),
            source,
        }),
    }
}
