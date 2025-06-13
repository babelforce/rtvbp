use crate::v1::ext::ApiSupport;
use crate::v1::message::ResponseMessage;
use crate::v1::{Header, Message};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait ResponseExt: ApiSupport {
    fn new<D>(request_id: String, status: u16, data: Option<D>) -> Message
    where
        D: Serialize,
    {
        Message::Response(ResponseMessage {
            header: Header::default(),
            response: request_id,
            status,
            data: serde_json::to_value(&data).unwrap().into(),
        })
    }

    fn default_ok(request_id: String) -> Message
    where
        Self: Default,
    {
        let res = Self::default();
        Message::Response(ResponseMessage {
            header: Header::default(),
            response: request_id,
            status: 200,
            data: serde_json::to_value(&res).unwrap().into(),
        })
    }

    fn err<D>(request_id: String) -> Message
    where
        Self: Default,
        D: Serialize,
    {
        Message::Response(ResponseMessage {
            header: Header::default(),
            response: request_id,
            status: 500,
            data: None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResponseError {
    #[serde(skip_serializing_if = "Option::is_none")]
    error_message: Option<String>,
    status: u16,
}

pub mod docs {
    use crate::v1::Header;
    use crate::v1::docs::{Example, Examples};
    use crate::v1::header::docs::EXAMPLE_MESSAGE_ID;
    use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;

    #[derive(Serialize, Deserialize, JsonSchema)]
    #[schemars(inline)]
    #[serde(rename_all = "snake_case")]
    pub enum ResponseKind {
        Response,
    }

    #[derive(Serialize, Deserialize)]
    pub struct ResponseMessage<T> {
        #[serde(flatten)]
        pub headers: Header,

        /// ID of the request message
        pub response: String,

        pub status: u16,

        pub data: T,
    }

    impl<T> Example for ResponseMessage<T>
    where
        T: Example,
    {
        fn example() -> Self {
            ResponseMessage {
                headers: Header::example(),
                data: T::example(),
                response: EXAMPLE_MESSAGE_ID.to_string(),
                status: 200,
            }
        }
    }

    impl<T> JsonSchema for ResponseMessage<T>
    where
        T: JsonSchema + Serialize + Example,
    {
        fn schema_name() -> Cow<'static, str> {
            format!("{}Payload", T::schema_name()).into()
        }

        fn json_schema(g: &mut SchemaGenerator) -> Schema {
            json_schema!({
                "type": "object",
                "additionalProperties": false,
                "allOf": [
                    g.subschema_for::<Header>(),
                    {
                        "title": format!("{}", T::schema_name()),
                        "additionalProperties": false,
                        "properties": {
                            "response": g.subschema_for::<String>(),
                            "status": g.subschema_for::<u16>(),
                            "data": g.subschema_for::<T>(),
                        },
                        "required": ["response", "status", "data"]
                    }
                ],
                "examples": Self::examples()
            })
        }
    }
}
