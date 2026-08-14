use std::future::pending;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rtvbp::envelope::v1classic;
use rtvbp::transport::memory::{Config as MemoryConfig, MemoryTransport};
use rtvbp::{
    ControlChannel, EventRegistration, Handler, KeepalivePolicy, MediaChannel, MediaFormat,
    NamedEvent, NamedRequest, Notifier, RequestRegistration, Requester, Session, SessionConfig,
    SessionState, Transport, TransportFactory, Validate,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct OuterRequest {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct OuterResponse {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct InnerRequest {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct InnerResponse {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SequenceEvent {
    sequence: usize,
}

impl Validate for OuterRequest {}
impl Validate for OuterResponse {}
impl Validate for InnerRequest {}
impl Validate for InnerResponse {}
impl Validate for SequenceEvent {}

impl NamedRequest for OuterRequest {
    type Response = OuterResponse;
    const METHOD: &'static str = "test.outer";
}

impl NamedRequest for InnerRequest {
    type Response = InnerResponse;
    const METHOD: &'static str = "test.inner";
}

impl NamedEvent for SequenceEvent {
    const EVENT: &'static str = "test.sequence";
}

fn session(transport: Arc<dyn Transport>, handler: Handler) -> Session {
    Session::new(
        Arc::new(v1classic::Envelope),
        handler,
        SessionConfig::with_transport(transport),
    )
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

async fn finish_pair(
    first: &Session,
    first_task: tokio::task::JoinHandle<Result<(), rtvbp::Error>>,
    second_task: tokio::task::JoinHandle<Result<(), rtvbp::Error>>,
) {
    first.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), first_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), second_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_fast_path_allows_nested_request_during_serial_dispatch() {
    let (left, right) = MemoryTransport::pair(MemoryConfig::default());
    let outer = RequestRegistration::typed::<OuterRequest, OuterResponse, _, _>(
        OuterRequest::METHOD,
        false,
        |context, request| async move {
            let nested = context
                .request_typed(InnerRequest {
                    value: request.value,
                })
                .await?;
            Ok(OuterResponse {
                value: format!("outer({})", nested.value),
            })
        },
    );
    let inner = RequestRegistration::typed::<InnerRequest, InnerResponse, _, _>(
        InnerRequest::METHOD,
        false,
        |_, request| async move {
            Ok(InnerResponse {
                value: format!("inner({})", request.value),
            })
        },
    );
    let first = session(left, Handler::new([outer], []).unwrap());
    let second = session(right, Handler::new([inner], []).unwrap());
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.run().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    wait_active(&first).await;
    wait_active(&second).await;

    let response = rtvbp::request_peer(
        &second,
        OuterRequest {
            value: "voice".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(response.value, "outer(inner(voice))");
    finish_pair(&first, first_task, second_task).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requests_and_events_dispatch_serially_in_admission_order() {
    let (left, right) = MemoryTransport::pair(MemoryConfig::default());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let (complete_tx, mut complete_rx) = mpsc::unbounded_channel();
    let events = EventRegistration::typed::<SequenceEvent, _, _>(SequenceEvent::EVENT, {
        let observed = Arc::clone(&observed);
        move |_, event| {
            let observed = Arc::clone(&observed);
            let complete_tx = complete_tx.clone();
            async move {
                if event.sequence == 1 {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                }
                observed.lock().unwrap().push(event.sequence);
                complete_tx.send(()).unwrap();
                Ok(())
            }
        }
    });
    let first = session(left, Handler::new([], [events]).unwrap());
    let second = session(right, Handler::new([], []).unwrap());
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.run().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    wait_active(&first).await;
    wait_active(&second).await;

    rtvbp::notify_event(&second, SequenceEvent { sequence: 1 })
        .await
        .unwrap();
    rtvbp::notify_event(&second, SequenceEvent { sequence: 2 })
        .await
        .unwrap();
    complete_rx.recv().await.unwrap();
    complete_rx.recv().await.unwrap();
    assert_eq!(*observed.lock().unwrap(), [1, 2]);
    finish_pair(&first, first_task, second_task).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_response_is_exactly_once_and_terminal_response_flushes() {
    let (left, right) = MemoryTransport::pair(MemoryConfig::default());
    let (second_attempt_tx, second_attempt_rx) = oneshot::channel();
    let second_attempt_tx = Arc::new(Mutex::new(Some(second_attempt_tx)));
    let deferred = RequestRegistration::typed::<OuterRequest, OuterResponse, _, _>(
        OuterRequest::METHOD,
        false,
        move |context, request| {
            let second_attempt_tx = Arc::clone(&second_attempt_tx);
            async move {
                let deferred = context.defer_response()?;
                let duplicate = deferred.clone();
                tokio::spawn(async move {
                    deferred
                        .respond(Some(
                            serde_json::to_value(OuterResponse {
                                value: format!("deferred({})", request.value),
                            })
                            .unwrap(),
                        ))
                        .await
                        .unwrap();
                    let result = duplicate.respond(Some(json!({"value": "duplicate"}))).await;
                    second_attempt_tx
                        .lock()
                        .unwrap()
                        .take()
                        .unwrap()
                        .send(result)
                        .ok();
                });
                Ok(OuterResponse {
                    value: "ignored".to_owned(),
                })
            }
        },
    );
    let terminal = RequestRegistration::typed::<InnerRequest, InnerResponse, _, _>(
        InnerRequest::METHOD,
        true,
        |_, request| async move {
            Ok(InnerResponse {
                value: format!("terminal({})", request.value),
            })
        },
    );
    let first = session(left, Handler::new([deferred, terminal], []).unwrap());
    let second = session(right, Handler::new([], []).unwrap());
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.run().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    wait_active(&first).await;
    wait_active(&second).await;

    let response = rtvbp::request_peer(
        &second,
        OuterRequest {
            value: "one".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(response.value, "deferred(one)");
    assert!(matches!(
        second_attempt_rx.await.unwrap(),
        Err(rtvbp::Error::ResponseAlreadySent)
    ));

    let terminal_response = rtvbp::request_peer(
        &second,
        InnerRequest {
            value: "two".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(terminal_response.value, "terminal(two)");
    tokio::time::timeout(Duration::from_secs(2), first_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), second_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(first.state(), SessionState::Closed);
    assert_eq!(second.state(), SessionState::Closed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn middleware_unknown_hooks_and_request_timeout_are_observable() {
    let (left, right) = MemoryTransport::pair(MemoryConfig::default());
    let (middleware_tx, middleware_rx) = oneshot::channel();
    let middleware_tx = Arc::new(Mutex::new(Some(middleware_tx)));
    let (event_tx, event_rx) = oneshot::channel();
    let event_tx = Arc::new(Mutex::new(Some(event_tx)));
    let handler = Handler::new([], [])
        .unwrap()
        .with_request_middleware(move |_, request| {
            let middleware_tx = Arc::clone(&middleware_tx);
            async move {
                middleware_tx
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(request.method)
                    .ok();
                Ok(())
            }
        })
        .with_unknown_request(|context, request| async move {
            context
                .respond(Some(json!({"method": request.method})))
                .await
        })
        .with_unknown_event(move |_, event| {
            let event_tx = Arc::clone(&event_tx);
            async move {
                event_tx
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(event.name)
                    .ok();
                Ok(())
            }
        });
    let mut first_config = SessionConfig::with_transport(left);
    first_config.request_timeout = Duration::from_millis(40);
    let first = Session::new(Arc::new(v1classic::Envelope), handler, first_config);
    let second_config = SessionConfig::with_transport(right);
    let second_handler =
        Handler::new([], [])
            .unwrap()
            .with_unknown_request(|context, _| async move {
                let _deferred = context.defer_response()?;
                Ok(())
            });
    let second = Session::new(Arc::new(v1classic::Envelope), second_handler, second_config);
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.run().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    wait_active(&first).await;
    wait_active(&second).await;

    let response = Requester::request(&second, "unknown.request", json!({}))
        .await
        .unwrap();
    assert_eq!(response, json!({"method": "unknown.request"}));
    assert_eq!(middleware_rx.await.unwrap(), "unknown.request");
    Notifier::notify(&second, "unknown.event", json!({}))
        .await
        .unwrap();
    assert_eq!(event_rx.await.unwrap(), "unknown.event");

    let timeout = Requester::request(&first, "never.answered", json!({})).await;
    assert!(matches!(timeout, Err(rtvbp::Error::RequestTimeout)));
    finish_pair(&first, first_task, second_task).await;
}

struct NeverFactory;

#[async_trait]
impl TransportFactory for NeverFactory {
    async fn connect(
        &self,
        _envelope: Arc<dyn rtvbp::Envelope>,
    ) -> Result<Arc<dyn Transport>, rtvbp::Error> {
        pending().await
    }
}

struct FailingFactory;

#[async_trait]
impl TransportFactory for FailingFactory {
    async fn connect(
        &self,
        _envelope: Arc<dyn rtvbp::Envelope>,
    ) -> Result<Arc<dyn Transport>, rtvbp::Error> {
        Err(rtvbp::Error::Configuration("dial failed".to_owned()))
    }
}

#[tokio::test]
async fn connecting_close_is_orderly_and_factory_failure_is_failed() {
    let connecting = Session::new(
        Arc::new(v1classic::Envelope),
        Handler::new([], []).unwrap(),
        SessionConfig::new(Arc::new(NeverFactory)),
    );
    let connecting_task = tokio::spawn({
        let connecting = connecting.clone();
        async move { connecting.run().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while connecting.state() != SessionState::Connecting {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    connecting.close().await.unwrap();
    connecting_task.await.unwrap().unwrap();
    assert_eq!(connecting.state(), SessionState::Closed);

    let failing = Session::new(
        Arc::new(v1classic::Envelope),
        Handler::new([], []).unwrap(),
        SessionConfig::new(Arc::new(FailingFactory)),
    );
    assert!(matches!(
        failing.run().await,
        Err(rtvbp::Error::SessionFailed(_))
    ));
    assert_eq!(failing.state(), SessionState::Failed);
    assert!(matches!(
        failing.run().await,
        Err(rtvbp::Error::SessionAlreadyRun)
    ));
}

struct FailingKeepaliveTransport {
    inner: Arc<dyn Transport>,
    fail: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl Transport for FailingKeepaliveTransport {
    fn control(&self) -> Arc<dyn ControlChannel> {
        self.inner.control()
    }

    async fn accept_media(&self) -> Result<Arc<dyn MediaChannel>, rtvbp::Error> {
        self.inner.accept_media().await
    }

    async fn open_media(
        &self,
        id: &str,
        format: MediaFormat,
    ) -> Result<Arc<dyn MediaChannel>, rtvbp::Error> {
        self.inner.open_media(id, format).await
    }

    async fn close(&self) -> Result<(), rtvbp::Error> {
        self.inner.close().await
    }

    fn supports_keepalive(&self) -> bool {
        true
    }

    async fn monitor_keepalive(&self, _policy: KeepalivePolicy) -> Result<(), rtvbp::Error> {
        self.fail.notified().await;
        Err(rtvbp::Error::KeepaliveTimeout)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keepalive_failure_fails_the_session() {
    let (left, right) = MemoryTransport::pair(MemoryConfig::default());
    let fail = Arc::new(tokio::sync::Notify::new());
    let transport: Arc<dyn Transport> = Arc::new(FailingKeepaliveTransport {
        inner: left,
        fail: Arc::clone(&fail),
    });
    let mut config = SessionConfig::with_transport(transport);
    config.keepalive = KeepalivePolicy {
        interval: Duration::from_secs(1),
        timeout: Duration::from_secs(1),
        max_misses: 1,
    };
    let first = Session::new(
        Arc::new(v1classic::Envelope),
        Handler::new([], []).unwrap(),
        config,
    );
    let second = session(right, Handler::new([], []).unwrap());
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.run().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    wait_active(&first).await;
    wait_active(&second).await;
    fail.notify_one();

    assert!(matches!(
        first_task.await.unwrap(),
        Err(rtvbp::Error::SessionFailed(message)) if message.contains("keepalive timed out")
    ));
    assert_eq!(first.state(), SessionState::Failed);
    second_task.await.unwrap().unwrap();
}

fn audio_format() -> MediaFormat {
    MediaFormat {
        encoding: "L16".to_owned(),
        sample_rate: 8_000,
        bit_depth: 16,
        channels: 1,
        ptime: Duration::from_millis(20),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_audio_binds_once_chunks_ptime_and_preserves_timed_inbound_frames() {
    let (left, right) = MemoryTransport::pair(MemoryConfig { media: true });
    let first_handler = Handler::new([], [])
        .unwrap()
        .with_on_begin(|context| async move { context.open_audio(audio_format()).await });
    let second_handler = Handler::new([], [])
        .unwrap()
        .with_on_begin(|context| async move { context.accept_audio().await });
    let first = session(left, first_handler);
    let second = session(right, second_handler);
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.run().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    wait_active(&first).await;
    wait_active(&second).await;
    assert_eq!(first.audio().format(), Some(audio_format()));
    assert_eq!(second.audio().format(), Some(audio_format()));
    assert!(matches!(
        first.open_audio(audio_format()).await,
        Err(rtvbp::Error::AudioAlreadyBound)
    ));
    let mut conflicting = audio_format();
    conflicting.sample_rate = 16_000;
    assert!(matches!(
        first.open_audio(conflicting).await,
        Err(rtvbp::Error::AudioFormatConflict)
    ));

    let first_frame = vec![0x11; 320];
    let second_frame = vec![0x22; 320];
    let partial = vec![0x33; 160];
    first
        .audio()
        .write(&[first_frame.clone(), second_frame.clone(), partial].concat())
        .await
        .unwrap();
    let mut received = vec![0; 640];
    let mut offset = 0;
    while offset < received.len() {
        offset += second.audio().read(&mut received[offset..]).await.unwrap();
    }
    assert_eq!(received, [first_frame, second_frame].concat());
    assert_eq!(second.audio().read_timed_frame().await.unwrap().pts, None);
    assert_eq!(second.audio().read_timed_frame().await.unwrap().pts, None);

    first.close().await.unwrap();
    let mut final_byte = [0; 1];
    assert!(matches!(
        second.audio().read(&mut final_byte).await,
        Err(rtvbp::Error::Closed)
    ));
    first_task.await.unwrap().unwrap();
    second_task.await.unwrap().unwrap();
}
