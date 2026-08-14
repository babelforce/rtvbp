use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rtvbp::catalog::babelforcev1 as catalog;
use rtvbp::catalog::babelforcev1::{
    ApplicationEventHandler, ApplicationHandler, VoiceEventHandler, VoiceHandler,
};
use rtvbp::catalog::demov1 as demo;
use rtvbp::envelope::v1classic;
use rtvbp::transport::memory::{Config as MemoryConfig, MemoryTransport};
use rtvbp::{
    ControlChannel, ControlFrame, Envelope, Error, FrameKind, Handler, HandlerContext, Session,
    SessionConfig, SessionState, Transport, Validate, WireError,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Deserialize)]
struct VectorCase {
    name: String,
    json: String,
    #[serde(default)]
    error: String,
}

#[derive(Deserialize)]
struct PayloadSide {
    valid: Vec<VectorCase>,
    invalid: Vec<VectorCase>,
}

#[derive(Deserialize)]
struct PayloadVector {
    method: String,
    request: PayloadSide,
    response: PayloadSide,
}

#[test]
fn every_generated_payload_vector_round_trips_and_validates() {
    let paths: Vec<_> = ["babelforce.v1", "demo.v1"]
        .into_iter()
        .flat_map(|catalog| json_files(&conformance_path(catalog).join("payloads")))
        .collect();
    assert_eq!(
        paths.len(),
        11,
        "all generated operation vectors are consumed"
    );
    for path in paths {
        let vector: PayloadVector = read_json(&path);
        check_payload_side(&vector.method, "request", &vector.request);
        check_payload_side(&vector.method, "response", &vector.response);
    }
}

fn check_payload_side(method: &str, side: &str, samples: &PayloadSide) {
    macro_rules! typed {
        ($request:ty, $response:ty) => {{
            if side == "request" {
                check_typed_samples::<$request>(method, side, samples);
            } else {
                check_typed_samples::<$response>(method, side, samples);
            }
        }};
    }
    match method {
        catalog::METHOD_APPLICATION_MOVE => typed!(
            catalog::ApplicationMoveRequest,
            catalog::ApplicationMoveResponse
        ),
        catalog::METHOD_AUDIO_BUFFER_CLEAR => typed!(
            catalog::AudioBufferClearRequest,
            catalog::AudioBufferClearResponse
        ),
        catalog::METHOD_CALL_HANGUP => typed!(catalog::CallHangupRequest, catalog::EmptyResponse),
        catalog::METHOD_PING => typed!(catalog::PingRequest, catalog::PingResponse),
        catalog::METHOD_RECORDING_START => typed!(
            catalog::RecordingStartRequest,
            catalog::RecordingStartResponse
        ),
        catalog::METHOD_RECORDING_STOP => {
            typed!(catalog::RecordingStopRequest, catalog::EmptyResponse);
        }
        catalog::METHOD_SESSION_GET => {
            typed!(catalog::SessionGetRequest, catalog::SessionGetResponse);
        }
        catalog::METHOD_SESSION_INITIALIZE => typed!(
            catalog::SessionInitializeRequest,
            catalog::SessionInitializeResponse
        ),
        catalog::METHOD_SESSION_SET => typed!(catalog::SessionSetRequest, catalog::EmptyResponse),
        catalog::METHOD_SESSION_TERMINATE => {
            typed!(catalog::SessionTerminateRequest, catalog::EmptyResponse);
        }
        demo::METHOD_DEMO_ECHO => typed!(demo::DemoEchoRequest, demo::DemoEchoResponse),
        other => panic!("unknown generated payload method {other:?}"),
    }
}

fn check_typed_samples<T>(method: &str, side: &str, samples: &PayloadSide)
where
    T: DeserializeOwned + Serialize + Validate,
{
    for sample in &samples.valid {
        let value: T = serde_json::from_str(&sample.json)
            .unwrap_or_else(|error| panic!("{method}/{side}/{} decode: {error}", sample.name));
        value
            .validate()
            .unwrap_or_else(|error| panic!("{method}/{side}/{} validate: {error}", sample.name));
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(encoded, sample.json, "{method}/{side}/{}", sample.name);
    }
    for sample in &samples.invalid {
        let decoded = serde_json::from_str::<T>(&sample.json);
        if sample.error == "decode" {
            assert!(decoded.is_err(), "{method}/{side}/{} decoded", sample.name);
        } else {
            let value = decoded.unwrap_or_else(|error| {
                panic!(
                    "{method}/{side}/{} unexpectedly failed decode: {error}",
                    sample.name
                )
            });
            assert!(
                value.validate().is_err(),
                "{method}/{side}/{} passed validation",
                sample.name
            );
        }
    }
}

#[derive(Deserialize)]
struct EnvelopeVector {
    envelope: String,
    encode: Vec<EnvelopeCase>,
    decode: Vec<EnvelopeCase>,
    invalid: Vec<InvalidFrame>,
}

#[derive(Deserialize)]
struct EnvelopeCase {
    name: String,
    frame: FrameSpec,
    bytes: String,
}

#[derive(Deserialize)]
struct InvalidFrame {
    name: String,
    bytes: String,
}

#[derive(Deserialize)]
struct FrameSpec {
    kind: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    response: String,
    error: Option<WireErrorSpec>,
    #[serde(default)]
    event: String,
    #[serde(flatten)]
    payloads: HashMap<String, Value>,
}

#[derive(Clone, Deserialize)]
struct WireErrorSpec {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl FrameSpec {
    fn frame(&self) -> ControlFrame {
        match self.kind.as_str() {
            "request" => ControlFrame::request(&self.id, &self.method, self.payload("params")),
            "response" => ControlFrame::response(
                &self.response,
                self.payload("result"),
                self.error.as_ref().map(WireErrorSpec::wire),
            ),
            "event" => ControlFrame::event(&self.id, &self.event, self.payload("data")),
            other => panic!("unknown generated frame kind {other:?}"),
        }
    }

    fn payload(&self, name: &str) -> Option<Value> {
        self.payloads.get(name).cloned()
    }
}

impl WireErrorSpec {
    fn wire(&self) -> WireError {
        WireError {
            code: self.code,
            message: self.message.clone(),
            data: self.data.clone(),
        }
    }
}

#[test]
fn every_generated_classic_envelope_vector_is_exact() {
    for catalog in ["babelforce.v1", "demo.v1"] {
        let vector: EnvelopeVector = read_json(
            &conformance_path(catalog)
                .join("envelope")
                .join("classic.v1")
                .join("frames.json"),
        );
        let codec = v1classic::Envelope;
        assert_eq!(codec.name(), vector.envelope);
        for sample in vector.encode {
            assert_eq!(
                String::from_utf8(codec.encode(&sample.frame.frame()).unwrap()).unwrap(),
                sample.bytes,
                "{catalog}/encode/{}",
                sample.name
            );
        }
        for sample in vector.decode {
            assert_eq!(
                codec.decode(sample.bytes.as_bytes()).unwrap(),
                sample.frame.frame(),
                "{catalog}/decode/{}",
                sample.name
            );
        }
        for sample in vector.invalid {
            assert!(
                codec.decode(sample.bytes.as_bytes()).is_err(),
                "{catalog}/invalid/{} decoded",
                sample.name
            );
        }
    }
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    roles: HashMap<String, String>,
    cases: Vec<ScenarioCase>,
}

#[derive(Clone, Deserialize)]
struct ScenarioCase {
    name: String,
    steps: Vec<ScenarioStep>,
}

#[derive(Clone, Deserialize)]
struct ScenarioStep {
    kind: String,
    from: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    method: String,
    params: Option<Value>,
    #[serde(default)]
    response: String,
    result: Option<Value>,
    error: Option<WireErrorSpec>,
    #[serde(default)]
    event: String,
    data: Option<Value>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_generated_typed_scenario_runs_with_either_role_local() {
    let mut count = 0;
    for catalog in ["babelforce.v1", "demo.v1"] {
        for path in json_files(&conformance_path(catalog).join("scenarios")) {
            count += 1;
            let scenario: Scenario = read_json(&path);
            for (local_name, local_role) in &scenario.roles {
                assert!(local_role == "application" || local_role == "voice");
                for case in &scenario.cases {
                    run_scenario_case(catalog, &scenario.name, case, local_name, local_role).await;
                }
            }
        }
    }
    assert_eq!(count, 5, "all generated scenarios are consumed");
}

#[allow(clippy::too_many_lines)]
async fn run_scenario_case(
    catalog_name: &str,
    scenario_name: &str,
    case: &ScenarioCase,
    local_name: &str,
    local_role: &str,
) {
    let (local, peer) = MemoryTransport::pair(MemoryConfig::default());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let scenario = Arc::new(ScenarioHandler {
        responses: scenario_responses(case),
        events: event_tx,
    });
    let handler = match (catalog_name, local_role) {
        ("babelforce.v1", "application") => Handler::new(
            catalog::application_handlers(Arc::clone(&scenario) as Arc<dyn ApplicationHandler>),
            catalog::application_event_handlers(
                Arc::clone(&scenario) as Arc<dyn ApplicationEventHandler>
            ),
        ),
        ("babelforce.v1", "voice") => Handler::new(
            catalog::voice_handlers(Arc::clone(&scenario) as Arc<dyn VoiceHandler>),
            catalog::voice_event_handlers(Arc::clone(&scenario) as Arc<dyn VoiceEventHandler>),
        ),
        ("demo.v1", "application") => Handler::new(
            demo::application_handlers(Arc::clone(&scenario) as Arc<dyn demo::ApplicationHandler>),
            demo::application_event_handlers(
                Arc::clone(&scenario) as Arc<dyn demo::ApplicationEventHandler>
            ),
        ),
        ("demo.v1", "voice") => Handler::new(
            demo::voice_handlers(Arc::clone(&scenario) as Arc<dyn demo::VoiceHandler>),
            demo::voice_event_handlers(Arc::clone(&scenario) as Arc<dyn demo::VoiceEventHandler>),
        ),
        other => panic!("unknown generated catalog/role {other:?}"),
    }
    .unwrap();
    let session = Session::new(
        Arc::new(v1classic::Envelope),
        handler,
        SessionConfig::with_transport(local),
    );
    let run = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    wait_active(&session).await;

    let codec = v1classic::Envelope;
    let control = peer.control();
    let mut bindings = HashMap::new();
    let mut pending: HashMap<String, JoinHandle<Result<Value, Error>>> = HashMap::new();
    for step in &case.steps {
        let local_origin = step.from == local_name;
        match step.kind.as_str() {
            "request" if local_origin => {
                let session = session.clone();
                let method = step.method.clone();
                let params = step.params.clone().unwrap_or_else(empty_object);
                let task =
                    tokio::spawn(async move { typed_request(&session, &method, params).await });
                let frame = receive_frame(&control, &codec).await;
                assert_eq!(
                    frame.kind,
                    FrameKind::Request,
                    "{scenario_name}/{}",
                    case.name
                );
                assert_eq!(frame.method, step.method, "{scenario_name}/{}", case.name);
                assert_eq!(frame.payload, step.params, "{scenario_name}/{}", case.name);
                bind_originated(&mut bindings, &step.id, &frame.id);
                pending.insert(step.id.clone(), task);
            }
            "request" => {
                let id = bind_peer(&mut bindings, &step.id);
                send_frame(
                    &control,
                    &codec,
                    &ControlFrame::request(id, &step.method, step.params.clone()),
                )
                .await;
            }
            "response" if local_origin => {
                let id = bound(&bindings, &step.response);
                let frame = receive_frame(&control, &codec).await;
                assert_eq!(
                    frame.kind,
                    FrameKind::Response,
                    "{scenario_name}/{}",
                    case.name
                );
                assert_eq!(frame.correlation_id, id, "{scenario_name}/{}", case.name);
                assert_response(frame.payload.as_ref(), frame.error.as_ref(), step);
            }
            "response" => {
                let id = bound(&bindings, &step.response);
                send_frame(
                    &control,
                    &codec,
                    &ControlFrame::response(
                        id,
                        step.result.clone(),
                        step.error.as_ref().map(WireErrorSpec::wire),
                    ),
                )
                .await;
                let outcome = pending.remove(&step.response).unwrap().await.unwrap();
                match &step.error {
                    None => assert_eq!(
                        outcome.unwrap(),
                        step.result.clone().unwrap_or_else(empty_object)
                    ),
                    Some(expected) => match outcome.unwrap_err() {
                        Error::Remote(actual) => {
                            assert_eq!(actual.code, expected.code);
                            assert_eq!(actual.message, expected.message);
                            assert_eq!(actual.data, expected.data);
                        }
                        other => panic!("wanted remote error, got {other:?}"),
                    },
                }
            }
            "event" if local_origin => {
                typed_event(
                    &session,
                    &step.event,
                    step.data.clone().unwrap_or_else(empty_object),
                )
                .await
                .unwrap();
                let frame = receive_frame(&control, &codec).await;
                assert_eq!(
                    frame.kind,
                    FrameKind::Event,
                    "{scenario_name}/{}",
                    case.name
                );
                assert_eq!(frame.method, step.event, "{scenario_name}/{}", case.name);
                assert_eq!(frame.payload, step.data, "{scenario_name}/{}", case.name);
                bind_originated(&mut bindings, &step.id, &frame.id);
            }
            "event" => {
                let id = bind_peer(&mut bindings, &step.id);
                send_frame(
                    &control,
                    &codec,
                    &ControlFrame::event(id, &step.event, step.data.clone()),
                )
                .await;
                let observed = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(observed, step.event, "{scenario_name}/{}", case.name);
            }
            other => panic!("unknown generated scenario step {other:?}"),
        }
    }
    assert!(
        pending.is_empty(),
        "{scenario_name}/{} has pending requests",
        case.name
    );
    session.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

async fn typed_request(session: &Session, method: &str, value: Value) -> Result<Value, Error> {
    macro_rules! request {
        ($request:ty) => {{
            let request: $request = serde_json::from_value(value).map_err(Error::envelope)?;
            serde_json::to_value(rtvbp::request_peer(session, request).await?)
                .map_err(Error::envelope)
        }};
    }
    match method {
        catalog::METHOD_APPLICATION_MOVE => request!(catalog::ApplicationMoveRequest),
        catalog::METHOD_AUDIO_BUFFER_CLEAR => request!(catalog::AudioBufferClearRequest),
        catalog::METHOD_CALL_HANGUP => request!(catalog::CallHangupRequest),
        catalog::METHOD_PING => request!(catalog::PingRequest),
        catalog::METHOD_RECORDING_START => request!(catalog::RecordingStartRequest),
        catalog::METHOD_RECORDING_STOP => request!(catalog::RecordingStopRequest),
        catalog::METHOD_SESSION_GET => request!(catalog::SessionGetRequest),
        catalog::METHOD_SESSION_INITIALIZE => request!(catalog::SessionInitializeRequest),
        catalog::METHOD_SESSION_SET => request!(catalog::SessionSetRequest),
        catalog::METHOD_SESSION_TERMINATE => request!(catalog::SessionTerminateRequest),
        demo::METHOD_DEMO_ECHO => request!(demo::DemoEchoRequest),
        other => panic!("unknown generated scenario method {other:?}"),
    }
}

async fn typed_event(session: &Session, event: &str, value: Value) -> Result<(), Error> {
    macro_rules! notify {
        ($event:ty) => {{
            let event: $event = serde_json::from_value(value).map_err(Error::envelope)?;
            rtvbp::notify_event(session, event).await
        }};
    }
    match event {
        catalog::EVENT_AGENT_TOOL_CALL => notify!(catalog::AgentToolCallEvent),
        catalog::EVENT_AUDIO_INFO => notify!(catalog::AudioInfoEvent),
        catalog::EVENT_AUDIO_SPEECH_STARTED => notify!(catalog::AudioSpeechStartedEvent),
        catalog::EVENT_CALL_HANGUP => notify!(catalog::CallHangupEvent),
        catalog::EVENT_DTMF => notify!(catalog::DtmfEvent),
        catalog::EVENT_INPUT_TRANSCRIPT => notify!(catalog::InputTranscriptEvent),
        catalog::EVENT_OUTPUT_TRANSCRIPT_DELTA => notify!(catalog::OutputTranscriptDeltaEvent),
        catalog::EVENT_OUTPUT_TRANSCRIPT_DONE => notify!(catalog::OutputTranscriptDoneEvent),
        catalog::EVENT_SESSION_UPDATED => notify!(catalog::SessionUpdatedEvent),
        demo::EVENT_DEMO_OBSERVED => notify!(demo::DemoObservedEvent),
        other => panic!("unknown generated scenario event {other:?}"),
    }
}

struct ScenarioHandler {
    responses: HashMap<String, Value>,
    events: mpsc::UnboundedSender<String>,
}

impl ScenarioHandler {
    fn response<T: DeserializeOwned>(&self, method: &str) -> Result<T, Error> {
        serde_json::from_value(
            self.responses
                .get(method)
                .unwrap_or_else(|| panic!("scenario has no response for {method}"))
                .clone(),
        )
        .map_err(Error::envelope)
    }

    fn event(&self, name: &str) -> Result<(), Error> {
        self.events.send(name.to_owned()).map_err(|error| {
            Error::RequestFailed(format!("scenario event observation failed: {error}"))
        })
    }
}

macro_rules! impl_handler {
    ($trait:path { $(($name:ident, $request:ty, $response:ty, $method:expr)),+ $(,)? }) => {
        #[async_trait]
        impl $trait for ScenarioHandler {
            $(
                async fn $name(
                    &self,
                    _: HandlerContext,
                    _: $request,
                ) -> Result<$response, Error> {
                    self.response($method)
                }
            )+
        }
    };
}

impl_handler!(ApplicationHandler {
    (ping, catalog::PingRequest, catalog::PingResponse, catalog::METHOD_PING),
    (
        session_initialize,
        catalog::SessionInitializeRequest,
        catalog::SessionInitializeResponse,
        catalog::METHOD_SESSION_INITIALIZE
    ),
    (
        session_terminate,
        catalog::SessionTerminateRequest,
        catalog::EmptyResponse,
        catalog::METHOD_SESSION_TERMINATE
    ),
});

impl_handler!(VoiceHandler {
    (
        application_move,
        catalog::ApplicationMoveRequest,
        catalog::ApplicationMoveResponse,
        catalog::METHOD_APPLICATION_MOVE
    ),
    (
        audio_buffer_clear,
        catalog::AudioBufferClearRequest,
        catalog::AudioBufferClearResponse,
        catalog::METHOD_AUDIO_BUFFER_CLEAR
    ),
    (
        call_hangup,
        catalog::CallHangupRequest,
        catalog::EmptyResponse,
        catalog::METHOD_CALL_HANGUP
    ),
    (ping, catalog::PingRequest, catalog::PingResponse, catalog::METHOD_PING),
    (
        recording_start,
        catalog::RecordingStartRequest,
        catalog::RecordingStartResponse,
        catalog::METHOD_RECORDING_START
    ),
    (
        recording_stop,
        catalog::RecordingStopRequest,
        catalog::EmptyResponse,
        catalog::METHOD_RECORDING_STOP
    ),
    (
        session_get,
        catalog::SessionGetRequest,
        catalog::SessionGetResponse,
        catalog::METHOD_SESSION_GET
    ),
    (
        session_set,
        catalog::SessionSetRequest,
        catalog::EmptyResponse,
        catalog::METHOD_SESSION_SET
    ),
});

macro_rules! impl_event_handler {
    ($trait:path { $(($name:ident, $event:ty, $constant:expr)),+ $(,)? }) => {
        #[async_trait]
        impl $trait for ScenarioHandler {
            $(
                async fn $name(
                    &self,
                    _: HandlerContext,
                    _: $event,
                ) -> Result<(), Error> {
                    self.event($constant)
                }
            )+
        }
    };
}

impl_event_handler!(ApplicationEventHandler {
    (audio_info, catalog::AudioInfoEvent, catalog::EVENT_AUDIO_INFO),
    (call_hangup, catalog::CallHangupEvent, catalog::EVENT_CALL_HANGUP),
    (dtmf, catalog::DtmfEvent, catalog::EVENT_DTMF),
    (
        session_updated,
        catalog::SessionUpdatedEvent,
        catalog::EVENT_SESSION_UPDATED
    ),
});

impl_event_handler!(VoiceEventHandler {
    (
        agent_tool_call,
        catalog::AgentToolCallEvent,
        catalog::EVENT_AGENT_TOOL_CALL
    ),
    (
        audio_speech_started,
        catalog::AudioSpeechStartedEvent,
        catalog::EVENT_AUDIO_SPEECH_STARTED
    ),
    (
        input_transcript,
        catalog::InputTranscriptEvent,
        catalog::EVENT_INPUT_TRANSCRIPT
    ),
    (
        output_transcript_delta,
        catalog::OutputTranscriptDeltaEvent,
        catalog::EVENT_OUTPUT_TRANSCRIPT_DELTA
    ),
    (
        output_transcript_done,
        catalog::OutputTranscriptDoneEvent,
        catalog::EVENT_OUTPUT_TRANSCRIPT_DONE
    ),
});

#[async_trait]
impl demo::ApplicationHandler for ScenarioHandler {
    async fn demo_echo(
        &self,
        _: HandlerContext,
        _: demo::DemoEchoRequest,
    ) -> Result<demo::DemoEchoResponse, Error> {
        self.response(demo::METHOD_DEMO_ECHO)
    }
}

#[async_trait]
impl demo::VoiceHandler for ScenarioHandler {}

#[async_trait]
impl demo::ApplicationEventHandler for ScenarioHandler {}

#[async_trait]
impl demo::VoiceEventHandler for ScenarioHandler {
    async fn demo_observed(
        &self,
        _: HandlerContext,
        _: demo::DemoObservedEvent,
    ) -> Result<(), Error> {
        self.event(demo::EVENT_DEMO_OBSERVED)
    }
}

fn scenario_responses(case: &ScenarioCase) -> HashMap<String, Value> {
    let methods: HashMap<_, _> = case
        .steps
        .iter()
        .filter(|step| step.kind == "request")
        .map(|step| (step.id.clone(), step.method.clone()))
        .collect();
    case.steps
        .iter()
        .filter(|step| step.kind == "response" && step.error.is_none())
        .map(|step| {
            (
                methods[&step.response].clone(),
                step.result.clone().unwrap_or_else(empty_object),
            )
        })
        .collect()
}

fn assert_response(actual: Option<&Value>, error: Option<&WireError>, expected: &ScenarioStep) {
    match &expected.error {
        None => {
            assert!(error.is_none());
            assert_eq!(actual.cloned(), expected.result);
        }
        Some(expected) => {
            let actual = error.unwrap();
            assert_eq!(actual.code, expected.code);
            assert_eq!(actual.message, expected.message);
            assert_eq!(actual.data, expected.data);
        }
    }
}

async fn send_frame(control: &Arc<dyn ControlChannel>, codec: &dyn Envelope, frame: &ControlFrame) {
    control.send(codec.encode(frame).unwrap()).await.unwrap();
}

async fn receive_frame(control: &Arc<dyn ControlChannel>, codec: &dyn Envelope) -> ControlFrame {
    let received = tokio::time::timeout(Duration::from_secs(2), control.recv())
        .await
        .unwrap()
        .unwrap();
    codec.decode(&received.data).unwrap()
}

async fn wait_active(session: &Session) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while session.state() != SessionState::Active {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

fn bind_originated(bindings: &mut HashMap<String, String>, name: &str, value: &str) {
    assert!(name.starts_with('$') && !value.is_empty());
    assert!(bindings.insert(name.to_owned(), value.to_owned()).is_none());
}

fn bind_peer(bindings: &mut HashMap<String, String>, name: &str) -> String {
    bindings
        .entry(name.to_owned())
        .or_insert_with(|| format!("peer-{}", name.trim_start_matches('$')))
        .clone()
}

fn bound(bindings: &HashMap<String, String>, name: &str) -> String {
    bindings[name].clone()
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn conformance_path(catalog: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("conformance")
        .join(catalog)
}

fn json_files(directory: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    paths
}

fn read_json<T: DeserializeOwned>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
