use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rtvbp::bridge::babelforcev1::{
    DtmfCallback, HangupCallback, TelephonyAdapter, VoiceBridge, VoiceBridgeConfig,
    default_media_format, new_ping_request,
};
use rtvbp::catalog::babelforcev1 as catalog;
use rtvbp::envelope::v1classic;
use rtvbp::transport::memory::{Config as MemoryConfig, MemoryTransport};
use rtvbp::{Handler, HandlerContext, Session, SessionConfig, SessionState, Transport};
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;

#[derive(Default)]
struct FakeTelephony {
    variables: Mutex<Map<String, Value>>,
    moved: Mutex<Option<catalog::ApplicationMoveRequest>>,
    hung_up: Mutex<bool>,
    dtmf: Mutex<Option<DtmfCallback>>,
    hangup: Mutex<Option<HangupCallback>>,
}

impl FakeTelephony {
    fn emit_dtmf(&self, event: catalog::DtmfEvent) {
        mutex_lock(&self.dtmf).as_ref().unwrap()(event);
    }

    fn emit_hangup(&self, event: catalog::CallHangupEvent) {
        mutex_lock(&self.hangup).as_ref().unwrap()(event);
    }
}

#[async_trait]
impl TelephonyAdapter for FakeTelephony {
    async fn application_move(
        &self,
        request: catalog::ApplicationMoveRequest,
    ) -> Result<catalog::ApplicationMoveResponse, rtvbp::Error> {
        *mutex_lock(&self.moved) = Some(request.clone());
        Ok(catalog::ApplicationMoveResponse {
            next_application_id: request.application_id.or_else(|| Some("<next>".to_owned())),
        })
    }

    async fn hangup(&self, _request: catalog::CallHangupRequest) -> Result<(), rtvbp::Error> {
        *mutex_lock(&self.hung_up) = true;
        Ok(())
    }

    async fn session_variables_set(
        &self,
        request: catalog::SessionSetRequest,
    ) -> Result<(), rtvbp::Error> {
        mutex_lock(&self.variables).extend(request.data);
        Ok(())
    }

    async fn session_variables_get(
        &self,
        request: catalog::SessionGetRequest,
    ) -> Result<Map<String, Value>, rtvbp::Error> {
        let variables = mutex_lock(&self.variables);
        if request.keys.is_empty() {
            return Ok(variables.clone());
        }
        Ok(request
            .keys
            .into_iter()
            .filter_map(|key| variables.get(&key).cloned().map(|value| (key, value)))
            .collect())
    }

    async fn recording_start(
        &self,
        _request: catalog::RecordingStartRequest,
    ) -> Result<catalog::RecordingStartResponse, rtvbp::Error> {
        Ok(catalog::RecordingStartResponse {
            id: "recording-1".to_owned(),
        })
    }

    async fn recording_stop(&self, recording_id: String) -> Result<(), rtvbp::Error> {
        if recording_id.is_empty() {
            Err(rtvbp::Error::Configuration(
                "recording ID is required".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    fn on_dtmf(&self, callback: DtmfCallback) -> Result<(), rtvbp::Error> {
        let mut slot = mutex_lock(&self.dtmf);
        if slot.replace(callback).is_some() {
            return Err(rtvbp::Error::Configuration(
                "DTMF callback already registered".to_owned(),
            ));
        }
        Ok(())
    }

    fn on_hangup(&self, callback: HangupCallback) -> Result<(), rtvbp::Error> {
        let mut slot = mutex_lock(&self.hangup);
        if slot.replace(callback).is_some() {
            return Err(rtvbp::Error::Configuration(
                "hangup callback already registered".to_owned(),
            ));
        }
        Ok(())
    }
}

struct Application {
    updated: mpsc::UnboundedSender<catalog::SessionUpdatedEvent>,
    events: mpsc::UnboundedSender<ApplicationEvent>,
}

#[derive(Debug)]
enum ApplicationEvent {
    Audio(catalog::AudioInfoEvent),
    Hangup(catalog::CallHangupEvent),
    Dtmf(catalog::DtmfEvent),
}

#[async_trait]
impl catalog::ApplicationHandler for Application {
    async fn ping(
        &self,
        context: HandlerContext,
        request: catalog::PingRequest,
    ) -> Result<catalog::PingResponse, rtvbp::Error> {
        let t1 = millis(context.received_at().unwrap());
        let t2 = millis(SystemTime::now());
        Ok(catalog::PingResponse {
            t0: request.t0,
            t1,
            t2,
            owd: t2 - request.t0,
            data: request.data,
        })
    }

    async fn session_initialize(
        &self,
        context: HandlerContext,
        request: catalog::SessionInitializeRequest,
    ) -> Result<catalog::SessionInitializeResponse, rtvbp::Error> {
        context.open_audio(default_media_format()).await?;
        Ok(catalog::SessionInitializeResponse {
            audio_codec: request.audio_codec_offerings.into_iter().next(),
        })
    }

    async fn session_terminate(
        &self,
        _context: HandlerContext,
        _request: catalog::SessionTerminateRequest,
    ) -> Result<catalog::EmptyResponse, rtvbp::Error> {
        Ok(catalog::EmptyResponse(Map::new()))
    }
}

#[async_trait]
impl catalog::ApplicationEventHandler for Application {
    async fn audio_info(
        &self,
        _context: HandlerContext,
        event: catalog::AudioInfoEvent,
    ) -> Result<(), rtvbp::Error> {
        self.events.send(ApplicationEvent::Audio(event)).unwrap();
        Ok(())
    }

    async fn call_hangup(
        &self,
        _context: HandlerContext,
        event: catalog::CallHangupEvent,
    ) -> Result<(), rtvbp::Error> {
        self.events.send(ApplicationEvent::Hangup(event)).unwrap();
        Ok(())
    }

    async fn dtmf(
        &self,
        _context: HandlerContext,
        event: catalog::DtmfEvent,
    ) -> Result<(), rtvbp::Error> {
        self.events.send(ApplicationEvent::Dtmf(event)).unwrap();
        Ok(())
    }

    async fn session_updated(
        &self,
        _context: HandlerContext,
        event: catalog::SessionUpdatedEvent,
    ) -> Result<(), rtvbp::Error> {
        self.updated.send(event).unwrap();
        Ok(())
    }
}

struct RunningBridge {
    application: Session,
    voice: Session,
    bridge: Arc<VoiceBridge>,
    telephony: Arc<FakeTelephony>,
    events: mpsc::UnboundedReceiver<ApplicationEvent>,
    application_task: tokio::task::JoinHandle<Result<(), rtvbp::Error>>,
    voice_task: tokio::task::JoinHandle<Result<(), rtvbp::Error>>,
}

async fn start_bridge(observe: bool) -> RunningBridge {
    let (left, right) = MemoryTransport::pair(MemoryConfig { media: true });
    let (updated_tx, mut updated_rx) = mpsc::unbounded_channel();
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let application_role = Arc::new(Application {
        updated: updated_tx,
        events: events_tx,
    });
    let application_requests =
        catalog::application_handlers(Arc::clone(&application_role) as Arc<_>);
    let application_events =
        catalog::application_event_handlers(Arc::clone(&application_role) as Arc<_>);
    let application = session(
        left,
        Handler::new(application_requests, application_events).unwrap(),
    );

    let telephony = Arc::new(FakeTelephony::default());
    let config = VoiceBridgeConfig {
        call: catalog::CallInfo {
            id: "call-1".to_owned(),
            session_id: "session-1".to_owned(),
            from: "1000".to_owned(),
            to: "1001".to_owned(),
        },
        application: catalog::AppInfo {
            id: "app-1".to_owned(),
        },
        metadata: Some(Map::from_iter([("test".to_owned(), json!(true))])),
        audio_format: default_media_format(),
    };
    let bridge = VoiceBridge::new(Arc::clone(&telephony) as Arc<_>, config);
    if observe {
        bridge.observe_audio(Duration::from_millis(10)).unwrap();
    }
    let voice = session(right, bridge.handler().unwrap());
    let application_task = tokio::spawn({
        let application = application.clone();
        async move { application.run().await }
    });
    wait_active(&application).await;
    let voice_task = tokio::spawn({
        let voice = voice.clone();
        async move { voice.run().await }
    });
    tokio::time::timeout(Duration::from_secs(2), updated_rx.recv())
        .await
        .unwrap()
        .unwrap();
    wait_active(&voice).await;
    RunningBridge {
        application,
        voice,
        bridge,
        telephony,
        events: events_rx,
        application_task,
        voice_task,
    }
}

fn session(transport: Arc<dyn Transport>, handler: Handler) -> Session {
    let mut config = SessionConfig::with_transport(transport);
    config.request_timeout = Duration::from_secs(2);
    config.close_timeout = Duration::from_secs(2);
    Session::new(Arc::new(v1classic::Envelope), handler, config)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generated_bridge_initializes_dispatches_callbacks_ping_and_observation() {
    let mut running = start_bridge(true).await;
    let peer = catalog::VoicePeer::new(running.application.clone());

    let request = new_ping_request().unwrap();
    let response = peer.ping(request.clone()).await.unwrap();
    assert_eq!(response.t0, request.t0);
    assert!(response.t1 > 0 && response.t2 >= response.t1);

    peer.session_set(catalog::SessionSetRequest {
        data: Map::from_iter([
            ("foo".to_owned(), json!("bar")),
            ("count".to_owned(), json!(23)),
        ]),
    })
    .await
    .unwrap();
    let values = peer
        .session_get(catalog::SessionGetRequest {
            keys: vec!["foo".to_owned(), "count".to_owned(), "missing".to_owned()],
        })
        .await
        .unwrap();
    assert_eq!(values.0.get("foo"), Some(&json!("bar")));
    assert_eq!(values.0.get("count"), Some(&json!(23)));

    let now = millis(SystemTime::now());
    running.telephony.emit_dtmf(catalog::DtmfEvent {
        seq: 99,
        pressed_at: now,
        released_at: now + 100,
        digit: "5".to_owned(),
    });
    match tokio::time::timeout(Duration::from_secs(2), running.events.recv())
        .await
        .unwrap()
        .unwrap()
    {
        ApplicationEvent::Dtmf(event) => {
            assert_eq!(event.seq, 0);
            assert_eq!(event.digit, "5");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    running.voice.audio().write(&[1; 320]).await.unwrap();
    let mut application_audio = [0_u8; 320];
    running
        .application
        .audio()
        .read(&mut application_audio)
        .await
        .unwrap();
    running.application.audio().write(&[2; 320]).await.unwrap();
    let mut voice_audio = [0_u8; 320];
    running.voice.audio().read(&mut voice_audio).await.unwrap();
    loop {
        match tokio::time::timeout(Duration::from_secs(2), running.events.recv())
            .await
            .unwrap()
            .unwrap()
        {
            ApplicationEvent::Audio(event) if event.read.bytes > 0 && event.write.bytes > 0 => {
                assert_eq!(event.read.bytes_total, 320);
                assert_eq!(event.write.bytes_total, 320);
                break;
            }
            ApplicationEvent::Audio(_) => {}
            other => panic!("unexpected event: {other:?}"),
        }
    }

    running.bridge.terminate("end_of_test").await.unwrap();
    running.voice_task.await.unwrap().unwrap();
    running.application_task.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generated_terminal_policy_flushes_move_response_then_closes_both_sessions() {
    let running = start_bridge(false).await;
    let response = catalog::VoicePeer::new(running.application.clone())
        .application_move(catalog::ApplicationMoveRequest {
            reason: Some("handoff".to_owned()),
            application_id: Some("app-2".to_owned()),
        })
        .await
        .unwrap();
    assert_eq!(response.next_application_id.as_deref(), Some("app-2"));
    assert_eq!(
        mutex_lock(&running.telephony.moved)
            .as_ref()
            .and_then(|request| request.application_id.as_deref()),
        Some("app-2")
    );
    running.voice_task.await.unwrap().unwrap();
    running.application_task.await.unwrap().unwrap();
    assert_eq!(running.voice.state(), SessionState::Closed);
    assert_eq!(running.application.state(), SessionState::Closed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telephony_hangup_callback_emits_event_and_terminates() {
    let mut running = start_bridge(false).await;
    running.telephony.emit_hangup(catalog::CallHangupEvent {
        reason: Some("caller".to_owned()),
    });
    match tokio::time::timeout(Duration::from_secs(2), running.events.recv())
        .await
        .unwrap()
        .unwrap()
    {
        ApplicationEvent::Hangup(event) => assert_eq!(event.reason.as_deref(), Some("caller")),
        other => panic!("unexpected event: {other:?}"),
    }
    running.voice_task.await.unwrap().unwrap();
    running.application_task.await.unwrap().unwrap();
}

fn millis(time: SystemTime) -> i64 {
    i64::try_from(time.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap()
}

fn mutex_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
