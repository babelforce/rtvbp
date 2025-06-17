use crate::v1::docs::asyncapischema::{Channel, Components, Info, MessageMap, Ref, Schema};
use crate::v1::docs::provider::{EventSpecExt, RequestSpecExt};
use crate::v1::message;
use crate::v1::op::application::ApplicationMoveRequest;
use crate::v1::op::audio::{AudioStreamStartRequest, AudioStreamStopRequest};
use crate::v1::op::ping::PingRequest;
use crate::v1::op::playback::PlaybackStartRequest;
use crate::v1::op::recording::{
    RecordingFinishedEvent, RecordingStartRequest, RecordingStopRequest,
};
use crate::v1::op::session::{SessionTerminatedEvent, SessionUpdatedEvent};
use indexmap::IndexMap;
use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;

pub fn jsonschema_generator() -> SchemaGenerator {
    let mut settings = SchemaSettings::default();
    settings.meta_schema = Some("https://asyncapi.com/definitions/3.0.0/asyncapi.json".into());
    settings.definitions_path = "/components/schemas".into();
    SchemaGenerator::new(settings.for_serialize())
}

pub fn json_schema() -> schemars::Schema {
    let mut g = jsonschema_generator();
    g.root_schema_for::<message::Message>()
}

pub fn async_api_schema() -> Schema {
    let mut g = jsonschema_generator();

    let mut schema = Schema {
        id: "urn:com.babelforce:rtvbp".to_string(),
        asyncapi: "3.0.0".to_string(),
        info: Info {
            version: "1.0.0".into(),
            title: "Realtime Voice Bridge Protocol".to_string(),
            description: None,
        }
        .into(),
        default_content_type: "application/json".into(),
        components: Components {
            schemas: IndexMap::new(),
            messages: MessageMap::new().into(),
        },
        ..Default::default()
    };

    let mut channel = Channel {
        messages: IndexMap::new(),
        description: None,
        address: "/stream".to_string(),
    };

    vec![
        // Requests (... and their responses)
        PingRequest::schema(&mut g),
        AudioStreamStartRequest::schema(&mut g),
        AudioStreamStopRequest::schema(&mut g),
        RecordingStartRequest::schema(&mut g),
        RecordingStopRequest::schema(&mut g),
        PlaybackStartRequest::schema(&mut g),
        ApplicationMoveRequest::schema(&mut g),
        // Events
        RecordingFinishedEvent::schema(&mut g),
        SessionUpdatedEvent::schema(&mut g),
        SessionTerminatedEvent::schema(&mut g),
    ]
    .iter()
    .for_each(|other| schema.merge(other));

    schema.components.messages.clone().map(|m| {
        m.iter().for_each(|(name, _)| {
            channel.messages.insert(
                name.clone(),
                Ref {
                    ref_path: format!("#/components/messages/{}", name),
                },
            );
        });
        m
    });

    schema.channels = IndexMap::from([("stream".to_string(), channel)]);

    g.definitions().iter().for_each(|(name, v)| {
        schema
            .components
            .schemas
            .insert(name.clone(), serde_json::from_value(v.clone()).unwrap());
    });

    schema
}

#[cfg(test)]
mod tests {
    use crate::v1::docs::generate::async_api_schema;

    #[test]
    fn test_generate() {
        println!("{}", serde_yaml::to_string(&async_api_schema()).unwrap());
    }
}
