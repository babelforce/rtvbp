use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Role, WireError};

/// A generated conformance scenario with one or more independent session cases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub roles: BTreeMap<String, Role>,
    pub cases: Vec<ScenarioCase>,
}

impl Scenario {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            roles: BTreeMap::new(),
            cases: Vec::new(),
        }
    }

    #[must_use]
    pub fn role(mut self, name: impl Into<String>, role: Role) -> Self {
        self.roles.insert(name.into(), role);
        self
    }

    #[must_use]
    pub fn case(mut self, case: ScenarioCase) -> Self {
        self.cases.push(case);
        self
    }
}

/// One independent session within a conformance scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioCase {
    pub name: String,
    pub steps: Vec<ScenarioStep>,
}

impl ScenarioCase {
    #[must_use]
    pub fn new(name: impl Into<String>, steps: impl IntoIterator<Item = ScenarioStep>) -> Self {
        Self {
            name: name.into(),
            steps: steps.into_iter().collect(),
        }
    }
}

/// One typed semantic exchange step. IDs beginning with `$` bind generated wire IDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ScenarioStep {
    Request {
        from: String,
        id: String,
        method: String,
        params: Value,
    },
    Response {
        from: String,
        response: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<WireError>,
    },
    Event {
        from: String,
        id: String,
        event: String,
        data: Value,
    },
}

impl ScenarioStep {
    #[must_use]
    pub fn request<T: Serialize>(
        from: impl Into<String>,
        id: impl Into<String>,
        method: impl Into<String>,
        params: &T,
    ) -> Self {
        Self::Request {
            from: from.into(),
            id: id.into(),
            method: method.into(),
            params: serde_json::to_value(params).expect("typed scenario request must serialize"),
        }
    }

    #[must_use]
    pub fn response<T: Serialize>(
        from: impl Into<String>,
        response: impl Into<String>,
        result: &T,
    ) -> Self {
        Self::Response {
            from: from.into(),
            response: response.into(),
            result: Some(
                serde_json::to_value(result).expect("typed scenario response must serialize"),
            ),
            error: None,
        }
    }

    #[must_use]
    pub fn error(from: impl Into<String>, response: impl Into<String>, error: WireError) -> Self {
        Self::Response {
            from: from.into(),
            response: response.into(),
            result: None,
            error: Some(error),
        }
    }

    #[must_use]
    pub fn event<T: Serialize>(
        from: impl Into<String>,
        id: impl Into<String>,
        event: impl Into<String>,
        data: &T,
    ) -> Self {
        Self::Event {
            from: from.into(),
            id: id.into(),
            event: event.into(),
            data: serde_json::to_value(data).expect("typed scenario event must serialize"),
        }
    }
}
