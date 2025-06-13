use crate::v1::ext::ApiSupport;
use crate::v1::message::RequestMessage;
use crate::v1::response::ResponseExt;
use crate::v1::{Header, Message};

pub trait RequestExt: ApiSupport {
    type Response: ResponseExt;

    fn request_method_name() -> &'static str;

    fn message(&self) -> Message {
        Message::Request(RequestMessage {
            header: Header::default(),
            request: Self::request_method_name().to_string(),
            data: serde_json::to_value(&self).unwrap().into(),
        })
    }
}

pub mod docs {
    use crate::v1::Header;
    use crate::v1::docs::Example;
    use crate::v1::docs::provider::RequestSpecExt;
    use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;

    #[derive(Serialize, Deserialize, JsonSchema)]
    #[schemars(inline)]
    #[serde(rename_all = "snake_case")]
    pub enum RequestKind {
        Request,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct RequestMessage<T> {
        #[serde(flatten)]
        pub headers: Header,

        /// Method to call
        pub request: String,

        pub data: T,
    }

    impl<T> Example for RequestMessage<T>
    where
        T: RequestSpecExt,
    {
        fn example() -> RequestMessage<T> {
            RequestMessage {
                headers: Header::example(),
                request: T::request_method_name().to_string(),
                data: T::example(),
            }
        }
    }

    impl<T> JsonSchema for RequestMessage<T>
    where
        T: Serialize + JsonSchema + RequestSpecExt + Example,
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
                            "request": { "type": "string", "const": T::request_method_name() },
                            "data": g.subschema_for::<T>(),
                        },
                        "required": ["kind", "method", "data"]
                    }
                ],
                "example": Self::example(),
            })
        }
    }
}
