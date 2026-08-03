use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Component, Path};

use schemars::{JsonSchema, Schema, schema_for};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{Scenario, ScenarioStep};

/// Method prefix reserved for envelope-independent transport signaling.
pub const RESERVED_TRANSPORT_METHOD_PREFIX: &str = "transport.";

/// A versioned payload catalog and all operations and events it declares.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub id: CatalogId,
    pub operations: Vec<Operation>,
    pub events: Vec<Event>,
    pub fixtures: Vec<CatalogFixture>,
    pub scenarios: Vec<Scenario>,
}

impl Catalog {
    #[must_use]
    pub fn new(name: impl Into<String>, major: u32) -> Self {
        Self {
            id: CatalogId::new(name, major),
            operations: Vec::new(),
            events: Vec::new(),
            fixtures: Vec::new(),
            scenarios: Vec::new(),
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

    #[must_use]
    pub fn scenarios(mut self, scenarios: impl IntoIterator<Item = Scenario>) -> Self {
        self.scenarios.extend(scenarios);
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
            validate_schema_rules(&mut issues, &operation.request);
            validate_schema_rules(&mut issues, &operation.response);
            validate_operation_rejections(&mut issues, operation);
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
            validate_schema_rules(&mut issues, &event.data);
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

        validate_scenarios(&mut issues, self);

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

impl fmt::Display for Role {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Voice => "voice",
            Self::Application => "application",
            Self::Both => "both",
        })
    }
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
    pub rejections: Vec<OperationRejection>,
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
            rejections: Vec::new(),
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

    /// Declare the exact error returned when a role that does not handle this operation receives it.
    #[must_use]
    pub fn reject(mut self, rejection: OperationRejection) -> Self {
        self.rejections.push(rejection);
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

/// An operation error intentionally returned by one concrete role that does not handle it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OperationRejection {
    pub role: Role,
    pub code: i64,
    pub message: String,
}

impl OperationRejection {
    #[must_use]
    pub fn new(role: Role, code: i64, message: impl Into<String>) -> Self {
        Self {
            role,
            code,
            message: message.into(),
        }
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
    #[error("payload schema {payload_type} at {location}: {message}")]
    InvalidSchemaRule {
        payload_type: String,
        location: String,
        message: String,
    },
    #[error("operation {method:?} rejection must name voice or application, not both")]
    AmbiguousRejectionRole { method: String },
    #[error("operation {method:?} rejection role {role} is already handled by {handled_by}")]
    RejectionForHandledRole {
        method: String,
        role: Role,
        handled_by: Role,
    },
    #[error("operation {method:?} has duplicate rejection for {role}")]
    DuplicateRejection { method: String, role: Role },
    #[error("operation {method:?} rejection for {role} error code must be non-zero")]
    RejectionZeroCode { method: String, role: Role },
    #[error("operation {method:?} rejection for {role} error message must be non-empty")]
    RejectionEmptyMessage { method: String, role: Role },
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
    #[error("conformance scenario {scenario:?} case {case:?} step {step}: {message}")]
    InvalidScenario {
        scenario: String,
        case: String,
        step: usize,
        message: String,
    },
    #[error("duplicate conformance scenario name {name:?}")]
    DuplicateScenario { name: String },
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

fn validate_scenarios(issues: &mut Vec<CatalogValidationError>, catalog: &Catalog) {
    let mut scenario_names = HashSet::new();
    for scenario in &catalog.scenarios {
        if scenario.name.trim().is_empty() {
            scenario_issue(
                issues,
                &scenario.name,
                "<name>",
                0,
                "scenario name must not be empty",
            );
        }
        if !scenario_names.insert(scenario.name.as_str()) {
            issues.push(CatalogValidationError::DuplicateScenario {
                name: scenario.name.clone(),
            });
        }
        let concrete_roles = scenario.roles.values().copied().collect::<HashSet<_>>();
        if scenario.roles.len() != 2
            || concrete_roles != HashSet::from([Role::Voice, Role::Application])
        {
            issues.push(CatalogValidationError::InvalidScenario {
                scenario: scenario.name.clone(),
                case: "<roles>".to_owned(),
                step: 0,
                message: "roles must contain concrete voice and application peers".to_owned(),
            });
        }
        if scenario.cases.is_empty() {
            scenario_issue(
                issues,
                &scenario.name,
                "<cases>",
                0,
                "scenario must contain at least one case",
            );
        }
        let mut case_names = HashSet::new();
        for case in &scenario.cases {
            if case.name.trim().is_empty() {
                scenario_issue(
                    issues,
                    &scenario.name,
                    &case.name,
                    0,
                    "case name must not be empty",
                );
            }
            if !case_names.insert(case.name.as_str()) {
                scenario_issue(issues, &scenario.name, &case.name, 0, "duplicate case name");
            }
            if case.steps.is_empty() {
                scenario_issue(
                    issues,
                    &scenario.name,
                    &case.name,
                    0,
                    "case must contain at least one step",
                );
                continue;
            }
            let mut bindings = HashMap::new();
            for (index, step) in case.steps.iter().enumerate() {
                let from = match step {
                    ScenarioStep::Request { from, .. }
                    | ScenarioStep::Response { from, .. }
                    | ScenarioStep::Event { from, .. } => from,
                };
                let Some(sender) = scenario.roles.get(from).copied() else {
                    scenario_issue(
                        issues,
                        &scenario.name,
                        &case.name,
                        index,
                        &format!("unknown peer {from:?}"),
                    );
                    continue;
                };
                match step {
                    ScenarioStep::Request {
                        id, method, params, ..
                    } => {
                        if !valid_binding(id) || bindings.contains_key(id) {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                "request id must be a unique $binding",
                            );
                            continue;
                        }
                        let Some(operation) = catalog
                            .operations
                            .iter()
                            .find(|operation| operation.method == *method)
                        else {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                &format!("unknown operation {method:?}"),
                            );
                            continue;
                        };
                        let receiver = peer_role(sender);
                        let rejection = operation
                            .rejections
                            .iter()
                            .find(|rejection| rejection.role == receiver);
                        let accepted = operation
                            .handled_by
                            .is_some_and(|role| role == receiver || role == Role::Both)
                            || rejection.is_some();
                        if !accepted {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                &format!("{sender} cannot send operation {method:?}"),
                            );
                        }
                        validate_scenario_value(
                            issues,
                            &scenario.name,
                            &case.name,
                            index,
                            &operation.request,
                            params,
                        );
                        bindings.insert(id.clone(), Some((operation, sender, rejection)));
                    }
                    ScenarioStep::Response {
                        response,
                        result,
                        error,
                        ..
                    } => {
                        let Some(Some((operation, requester, rejection))) =
                            bindings.get(response).copied()
                        else {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                &format!("response references unknown binding {response:?}"),
                            );
                            continue;
                        };
                        if sender != peer_role(requester) {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                "response must come from the request peer",
                            );
                        }
                        if result.is_some() == error.is_some() {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                "response must contain exactly one of result or error",
                            );
                        }
                        if let Some(result) = result {
                            validate_scenario_value(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                &operation.response,
                                result,
                            );
                        }
                        if let Some(error) = error
                            && (error.code == 0 || error.message.trim().is_empty())
                        {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                "response error requires a non-zero code and message",
                            );
                        }
                        if let Some(rejection) = rejection
                            && error.as_ref().is_none_or(|error| {
                                error.code != rejection.code || error.message != rejection.message
                            })
                        {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                "response must match the declared role rejection",
                            );
                        }
                        bindings.remove(response);
                    }
                    ScenarioStep::Event {
                        id, event, data, ..
                    } => {
                        if !valid_binding(id) || bindings.contains_key(id) {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                "event id must be a unique $binding",
                            );
                        }
                        let Some(event_spec) =
                            catalog.events.iter().find(|item| item.name == *event)
                        else {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                &format!("unknown event {event:?}"),
                            );
                            continue;
                        };
                        if event_spec
                            .emitted_by
                            .is_some_and(|role| role != sender && role != Role::Both)
                        {
                            scenario_issue(
                                issues,
                                &scenario.name,
                                &case.name,
                                index,
                                &format!("{sender} cannot emit event {event:?}"),
                            );
                        }
                        validate_scenario_value(
                            issues,
                            &scenario.name,
                            &case.name,
                            index,
                            &event_spec.data,
                            data,
                        );
                        bindings.insert(id.clone(), None);
                    }
                }
            }
            for (binding, request) in bindings {
                if request.is_some() {
                    scenario_issue(
                        issues,
                        &scenario.name,
                        &case.name,
                        case.steps.len(),
                        &format!("request binding {binding:?} has no response"),
                    );
                }
            }
        }
    }
}

fn validate_scenario_value(
    issues: &mut Vec<CatalogValidationError>,
    scenario: &str,
    case: &str,
    step: usize,
    payload: &TypeRef,
    value: &Value,
) {
    match payload.round_trip(value) {
        Ok(actual) if actual == *value => {}
        Ok(_) => scenario_issue(
            issues,
            scenario,
            case,
            step,
            &format!("payload changes after typed {} round-trip", payload.name),
        ),
        Err(error) => scenario_issue(
            issues,
            scenario,
            case,
            step,
            &format!("payload does not match {}: {error}", payload.name),
        ),
    }
}

fn scenario_issue(
    issues: &mut Vec<CatalogValidationError>,
    scenario: &str,
    case: &str,
    step: usize,
    message: &str,
) {
    issues.push(CatalogValidationError::InvalidScenario {
        scenario: scenario.to_owned(),
        case: case.to_owned(),
        step,
        message: message.to_owned(),
    });
}

fn valid_binding(value: &str) -> bool {
    value.strip_prefix('$').is_some_and(|name| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

fn peer_role(role: Role) -> Role {
    match role {
        Role::Voice => Role::Application,
        Role::Application => Role::Voice,
        Role::Both => Role::Both,
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

fn validate_operation_rejections(issues: &mut Vec<CatalogValidationError>, operation: &Operation) {
    let mut roles = HashSet::new();
    for rejection in &operation.rejections {
        if rejection.role == Role::Both {
            issues.push(CatalogValidationError::AmbiguousRejectionRole {
                method: operation.method.clone(),
            });
            continue;
        }
        if !roles.insert(rejection.role) {
            issues.push(CatalogValidationError::DuplicateRejection {
                method: operation.method.clone(),
                role: rejection.role,
            });
        }
        if let Some(handled_by) = operation.handled_by
            && (handled_by == Role::Both || handled_by == rejection.role)
        {
            issues.push(CatalogValidationError::RejectionForHandledRole {
                method: operation.method.clone(),
                role: rejection.role,
                handled_by,
            });
        }
        if rejection.code == 0 {
            issues.push(CatalogValidationError::RejectionZeroCode {
                method: operation.method.clone(),
                role: rejection.role,
            });
        }
        if rejection.message.trim().is_empty() {
            issues.push(CatalogValidationError::RejectionEmptyMessage {
                method: operation.method.clone(),
                role: rejection.role,
            });
        }
    }
}

fn validate_schema_rules(issues: &mut Vec<CatalogValidationError>, payload_type: &TypeRef) {
    validate_schema_node(
        issues,
        &payload_type.name,
        "$",
        payload_type.schema.as_value(),
    );
}

fn validate_schema_node(
    issues: &mut Vec<CatalogValidationError>,
    payload_type: &str,
    location: &str,
    schema: &Value,
) {
    let Some(object) = schema.as_object() else {
        return;
    };

    if let Some(min_length) = object.get("minLength") {
        if min_length.as_u64().is_none() {
            schema_issue(
                issues,
                payload_type,
                location,
                "minLength must be a non-negative integer",
            );
        }
        if !schema_accepts_type(object, "string") {
            schema_issue(
                issues,
                payload_type,
                location,
                "minLength requires a string field",
            );
        }
    }

    if let Some(minimum) = object.get("minimum") {
        if !minimum.is_number() {
            schema_issue(issues, payload_type, location, "minimum must be a number");
        }
        if !schema_accepts_type(object, "integer") && !schema_accepts_type(object, "number") {
            schema_issue(
                issues,
                payload_type,
                location,
                "minimum requires a numeric field",
            );
        }
    }

    if let Some(nonzero) = object.get("x-rtvbp-nonzero") {
        if nonzero != &Value::Bool(true) {
            schema_issue(
                issues,
                payload_type,
                location,
                "x-rtvbp-nonzero must be true",
            );
        }
        if !schema_accepts_type(object, "integer") {
            schema_issue(
                issues,
                payload_type,
                location,
                "x-rtvbp-nonzero requires an integer field",
            );
        }
    }

    if let Some(order) = object.get("x-rtvbp-field-order") {
        validate_field_order(issues, payload_type, location, object, order);
    }

    for (key, value) in object {
        match value {
            Value::Object(_) => {
                validate_schema_node(issues, payload_type, &format!("{location}.{key}"), value)
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    validate_schema_node(
                        issues,
                        payload_type,
                        &format!("{location}.{key}[{index}]"),
                        item,
                    );
                }
            }
            _ => {}
        }
    }
}

fn validate_field_order(
    issues: &mut Vec<CatalogValidationError>,
    payload_type: &str,
    location: &str,
    schema: &serde_json::Map<String, Value>,
    order: &Value,
) {
    if !schema_accepts_type(schema, "object") {
        schema_issue(
            issues,
            payload_type,
            location,
            "x-rtvbp-field-order requires an object schema",
        );
        return;
    }
    let Some(entries) = order.as_array().filter(|entries| !entries.is_empty()) else {
        schema_issue(
            issues,
            payload_type,
            location,
            "x-rtvbp-field-order must be a non-empty array",
        );
        return;
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        schema_issue(
            issues,
            payload_type,
            location,
            "x-rtvbp-field-order requires object properties",
        );
        return;
    };
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();

    for entry in entries {
        let Some(entry) = entry.as_object() else {
            schema_issue(
                issues,
                payload_type,
                location,
                "x-rtvbp-field-order entries require exactly lower and upper",
            );
            continue;
        };
        if entry.len() != 2 || !entry.contains_key("lower") || !entry.contains_key("upper") {
            schema_issue(
                issues,
                payload_type,
                location,
                "x-rtvbp-field-order entries require exactly lower and upper",
            );
            continue;
        }
        let (Some(lower), Some(upper)) = (
            entry.get("lower").and_then(Value::as_str),
            entry.get("upper").and_then(Value::as_str),
        ) else {
            schema_issue(
                issues,
                payload_type,
                location,
                "x-rtvbp-field-order lower and upper must be field names",
            );
            continue;
        };
        for (bound, field) in [("lower", lower), ("upper", upper)] {
            let Some(field_schema) = properties.get(field).and_then(Value::as_object) else {
                schema_issue(
                    issues,
                    payload_type,
                    location,
                    &format!("x-rtvbp-field-order references unknown {bound} field {field:?}"),
                );
                continue;
            };
            if !schema_accepts_type(field_schema, "integer") {
                schema_issue(
                    issues,
                    payload_type,
                    location,
                    &format!("x-rtvbp-field-order {bound} field {field:?} must be an integer"),
                );
            }
            if !required.contains(field) {
                schema_issue(
                    issues,
                    payload_type,
                    location,
                    &format!("x-rtvbp-field-order {bound} field {field:?} must be required"),
                );
            }
        }
        if lower == upper {
            schema_issue(
                issues,
                payload_type,
                location,
                "x-rtvbp-field-order lower and upper must be different fields",
            );
        }
    }
}

fn schema_accepts_type(schema: &serde_json::Map<String, Value>, expected: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(actual)) => actual == expected,
        Some(Value::Array(actual)) => actual.iter().any(|item| item == expected),
        _ => false,
    }
}

fn schema_issue(
    issues: &mut Vec<CatalogValidationError>,
    payload_type: &str,
    location: &str,
    message: &str,
) {
    issues.push(CatalogValidationError::InvalidSchemaRule {
        payload_type: payload_type.to_owned(),
        location: location.to_owned(),
        message: message.to_owned(),
    });
}
