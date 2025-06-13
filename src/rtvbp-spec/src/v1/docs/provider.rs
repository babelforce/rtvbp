use crate::v1::docs::Example;
use crate::v1::docs::asyncapischema::{
    Action, Components, Message, Operation, Ref, Reply, Schema as AsyncApiSchema,
};
use crate::v1::event::EventExt;
use crate::v1::event::docs::EventMessage;
use crate::v1::request::RequestExt;
use crate::v1::request::docs::RequestMessage;
use crate::v1::response::ResponseExt;
use crate::v1::response::docs::ResponseMessage;
use convert_case::{Case, Casing};
use indexmap::IndexMap;
use schemars::SchemaGenerator;

impl<T> EventSpecExt for T where T: EventExt + Example {}

impl<T> RequestSpecExt for T
where
    T: RequestExt + Example,
    T::Response: Example,
{
    type SpecResponse = T::Response;
}

pub trait EventSpecExt: EventExt + Example {
    fn spec_event_name() -> String {
        format!("{}Event", Self::event_name().to_case(Case::Pascal))
    }

    fn spec_event_payload_name() -> String {
        format!("{}Payload", Self::spec_event_name().to_case(Case::Pascal))
    }

    fn spec_operation_name() -> String {
        format!("{}", Self::spec_event_name())
    }

    fn messages() -> IndexMap<String, Message> {
        IndexMap::from([(
            format!("{}Message", Self::spec_event_name()),
            Message {
                payload: Ref {
                    ref_path: format!("#/components/schemas/{}", Self::spec_event_payload_name())
                        .to_string(),
                },
            },
        )])
    }

    fn schema(g: &mut SchemaGenerator) -> AsyncApiSchema {
        let mut schemas = IndexMap::new();
        schemas.insert(
            Self::spec_event_payload_name(),
            g.subschema_for::<EventMessage<Self>>(),
        );

        let operations =
            IndexMap::from([(Self::spec_operation_name().to_string(), Self::operation())]);
        AsyncApiSchema {
            operations,
            components: Components {
                messages: Self::messages().into(),
                schemas,
            }
            .into(),
            ..Default::default()
        }
    }

    fn operation() -> Operation {
        Operation {
            summary: None,
            // TODO: summary
            action: Action::Receive,
            channel: Ref {
                ref_path: "#/channels/stream".to_string(),
            },
            messages: vec![Ref {
                ref_path: format!(
                    "#/channels/stream/messages/{}Message",
                    Self::spec_operation_name()
                ),
            }],
            reply: None,
        }
    }
}

pub trait RequestSpecExt: RequestExt + Example {
    type SpecResponse: ResponseExt + Example;

    /// Get the Operation ID
    fn operation_id() -> String {
        Self::request_method_name().to_case(Case::Pascal)
    }

    fn messages() -> IndexMap<String, Message> {
        let req_name = format!("{}Request", Self::operation_id());
        let res_name = format!("{}Response", Self::operation_id());
        IndexMap::from([
            (
                format!("{}Message", req_name),
                Message {
                    payload: Ref {
                        ref_path: format!("#/components/schemas/{}Payload", req_name).to_string(),
                    },
                },
            ),
            (
                format!("{}Message", res_name),
                Message {
                    payload: Ref {
                        ref_path: format!("#/components/schemas/{}Payload", res_name).to_string(),
                    },
                },
            ),
        ])
    }

    fn schema(g: &mut SchemaGenerator) -> AsyncApiSchema {
        let mut schemas = IndexMap::new();
        schemas.insert(
            format!("{}RequestPayload", Self::operation_id()),
            g.subschema_for::<RequestMessage<Self>>(),
        );
        schemas.insert(
            format!("{}ResponsePayload", Self::operation_id()),
            g.subschema_for::<ResponseMessage<Self::SpecResponse>>(),
        );

        let operations = IndexMap::from([(Self::operation_id().to_string(), Self::operation())]);
        AsyncApiSchema {
            operations,
            components: Components {
                messages: Self::messages().into(),
                schemas,
            }
            .into(),
            ..Default::default()
        }
    }

    fn operation() -> Operation {
        Operation {
            summary: None,
            // TODO: summary
            action: Action::Send,
            channel: Ref {
                ref_path: "#/channels/stream".to_string(),
            },
            messages: vec![Ref {
                ref_path: format!(
                    "#/channels/stream/messages/{}RequestMessage",
                    Self::operation_id()
                ),
            }],
            reply: Reply {
                channel: Ref {
                    ref_path: "#/channels/stream".to_string(),
                },
                messages: vec![Ref {
                    ref_path: format!(
                        "#/channels/stream/messages/{}ResponseMessage",
                        Self::operation_id()
                    ),
                }],
            }
            .into(),
        }
    }
}
