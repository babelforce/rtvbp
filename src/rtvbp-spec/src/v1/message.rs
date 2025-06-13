use crate::v1::header::Header;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
pub enum Message {
    Request(RequestMessage),
    Response(ResponseMessage),
    Event(EventMessage),
    Binary(Vec<u8>),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RequestMessage {
    #[serde(flatten)]
    pub header: Header,

    pub request: String,

    pub data: Option<Value>,
}

impl RequestMessage {
    pub fn id(&self) -> String {
        self.header.id.clone()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResponseMessage {
    #[serde(flatten)]
    pub header: Header,

    /// Request message ID
    pub response: String,

    pub status: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct EventMessage {
    #[serde(flatten)]
    pub header: Header,

    pub event: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Message {
    pub fn id(&self) -> String {
        match self {
            Self::Request(req) => req.header.id.clone(),
            Self::Response(res) => res.header.id.clone(),
            Self::Event(evt) => evt.header.id.clone(),
            Self::Binary(_) => "".to_string(),
        }
    }

    pub fn headers(&self) -> Header {
        match self {
            Self::Request(req) => req.header.clone(),
            Self::Response(res) => res.header.clone(),
            Self::Event(evt) => evt.header.clone(),
            Self::Binary(_) => Header::default(),
        }
    }

    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request(_))
    }

    pub fn is_response(&self) -> bool {
        matches!(self, Self::Response(_))
    }

    pub fn is_event(&self) -> bool {
        matches!(self, Self::Event(_))
    }

    pub fn is_binary(&self) -> bool {
        matches!(self, Self::Binary(_))
    }

    pub fn is_error(&self) -> bool {
        todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResponseBody<D>
where
    D: Serialize,
{
    /// ID of the request message
    pub request_id: String,
    pub status: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<D>,
}

#[cfg(test)]
mod tests {
    use crate::v1::event::EventExt;
    use crate::v1::op::log::LogEvent;
    use crate::v1::op::ping::{PingRequest, PingResponse};
    use crate::v1::request::RequestExt;
    use crate::v1::response::ResponseExt;

    #[test]
    fn test_serialization_of_messages() {
        let req = PingRequest::default().message();
        println!("{}", serde_json::to_string_pretty(&req).unwrap());

        let res = PingResponse::default_ok(req.id());
        println!("{}", serde_json::to_string_pretty(&res).unwrap());

        let evt = LogEvent::new("hello", None).message();
        println!("{}", serde_json::to_string_pretty(&evt).unwrap());
        //assert_eq!(msg.header.id, "1");
    }
}
