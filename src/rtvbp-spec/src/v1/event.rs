use crate::v1::ext::ApiSupport;
use crate::v1::message::EventMessage;
use crate::v1::{Header, Message};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait EventExt: ApiSupport {
    fn event_name() -> &'static str;

    fn message(&self) -> Message {
        Message::Event(EventMessage {
            header: Header::default(),
            event: Self::event_name().to_string(),
            data: serde_json::to_value(&self).unwrap().into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ErrorEvent {
    pub message: String,
    pub code: u16,
    pub details: Option<String>,
}

impl EventExt for ErrorEvent {
    fn event_name() -> &'static str {
        "error"
    }
}

pub mod docs {
    use crate::v1::Header;
    use crate::v1::docs::Example;
    use crate::v1::docs::provider::EventSpecExt;
    use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct EventMessage<T> {
        #[serde(flatten)]
        pub headers: Header,

        pub event: String,

        pub data: T,
    }

    impl<T> JsonSchema for EventMessage<T>
    where
        T: EventSpecExt,
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
                            "event": { "type": "string", "const": T::event_name() },
                            "data": g.subschema_for::<T>(),
                        },
                        "required": ["event", "data"]
                    }
                ],
                "example": Self::example()
            })
        }
    }

    impl<T> Example for EventMessage<T>
    where
        T: EventSpecExt,
    {
        fn example() -> Self {
            EventMessage {
                headers: Header::example(),
                event: T::event_name().to_string(),
                data: T::example(),
            }
        }
    }
}
