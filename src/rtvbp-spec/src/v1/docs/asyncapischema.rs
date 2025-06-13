use indexmap::IndexMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Schema {
    pub asyncapi: String,
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<Info>,

    #[serde(rename = "defaultContentType")]
    pub default_content_type: String,

    // TODO: servers
    // TODO: channels
    pub channels: IndexMap<String, Channel>,

    pub components: Components,

    pub operations: IndexMap<String, Operation>,
}

impl Schema {
    pub fn merge(&mut self, other: &Schema) {
        other.components.messages.clone().map(|messages| {
            self.components.messages.as_mut().unwrap().extend(messages);
        });

        self.operations.extend(other.operations.clone());
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Channel {
    pub address: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub messages: IndexMap<String, Ref>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Info {
    pub title: String,
    pub version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ref {
    #[serde(rename = "$ref")]
    pub ref_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Send,
    Receive,
}

#[derive(Debug, Clone, Serialize)]
pub struct Operation {
    pub channel: Ref,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub action: Action,
    pub messages: Vec<Ref>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<Reply>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reply {
    pub channel: Ref,
    pub messages: Vec<Ref>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub(crate) payload: Ref,
}

pub type MessageMap = IndexMap<String, Message>;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Components {
    pub(crate) schemas: IndexMap<String, schemars::Schema>,
    pub(crate) messages: Option<MessageMap>,
}
