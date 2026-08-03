use std::fmt;

use schemars::{JsonSchema, Schema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A versioned payload catalog and all operations and events it declares.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    pub id: CatalogId,
    pub operations: Vec<Operation>,
    pub events: Vec<Event>,
}

impl Catalog {
    #[must_use]
    pub fn new(name: impl Into<String>, major: u32) -> Self {
        Self {
            id: CatalogId::new(name, major),
            operations: Vec::new(),
            events: Vec::new(),
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
#[derive(Clone, Debug, PartialEq)]
pub struct TypeRef {
    pub name: String,
    pub schema: Schema,
}

impl TypeRef {
    #[must_use]
    pub fn of<T: JsonSchema>() -> Self {
        Self {
            name: T::schema_name().into_owned(),
            schema: schema_for!(T),
        }
    }
}

/// A request/response operation in a catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    pub method: String,
    pub handled_by: Role,
    pub request: TypeRef,
    pub response: TypeRef,
    pub terminal: bool,
    pub docs: Option<String>,
    pub examples: Vec<OperationExample>,
}

impl Operation {
    #[must_use]
    pub fn new<Request: JsonSchema, Response: JsonSchema>(
        method: impl Into<String>,
        handled_by: Role,
    ) -> Self {
        Self {
            method: method.into(),
            handled_by,
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
    pub emitted_by: Role,
    pub data: TypeRef,
    pub docs: Option<String>,
    pub examples: Vec<EventExample>,
}

impl Event {
    #[must_use]
    pub fn new<Data: JsonSchema>(name: impl Into<String>, emitted_by: Role) -> Self {
        Self {
            name: name.into(),
            emitted_by,
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
