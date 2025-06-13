use crate::v1::event::EventExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogEvent {
    message: String,
    data: Option<Value>,
}

impl LogEvent {
    pub fn new(message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            message: message.into(),
            data,
        }
    }
}

impl EventExt for LogEvent {
    fn event_name() -> &'static str {
        "log"
    }
}
