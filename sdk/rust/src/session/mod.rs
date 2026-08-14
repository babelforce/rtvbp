//! Session lifecycle, correlated requests, serial dispatch, and audio ownership.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::audio::AudioStream;
use crate::{
    ControlFrame, Envelope, EventRegistration, FrameKind, HandlerReply, MediaChannel, MediaFormat,
    NamedEvent, NamedRequest, Notifier, RequestRegistration, Requester, Transport,
    TransportFactory, WireError,
};

type HookFuture<T> = Pin<Box<dyn Future<Output = Result<T, crate::Error>> + Send>>;
type BeginHook = dyn Fn(HandlerContext) -> HookFuture<()> + Send + Sync;
type RequestHook = dyn Fn(HandlerContext, InboundRequest) -> HookFuture<()> + Send + Sync;
type EventHook = dyn Fn(HandlerContext, InboundEvent) -> HookFuture<()> + Send + Sync;
type PendingSender = oneshot::Sender<Result<Option<Value>, crate::Error>>;
type PendingRequests = HashMap<String, PendingSender>;

/// One decoded request before typed catalog dispatch.
#[derive(Clone, Debug, PartialEq)]
pub struct InboundRequest {
    pub id: String,
    pub method: String,
    pub payload: Option<Value>,
    pub received_at: SystemTime,
}

/// One decoded event before typed catalog dispatch.
#[derive(Clone, Debug, PartialEq)]
pub struct InboundEvent {
    pub id: String,
    pub name: String,
    pub payload: Option<Value>,
    pub received_at: SystemTime,
}

/// Catalog dispatch table plus lifecycle and unknown-message hooks.
pub struct Handler {
    requests: HashMap<&'static str, RequestRegistration>,
    events: HashMap<&'static str, EventRegistration>,
    on_begin: Arc<BeginHook>,
    middleware: Vec<Arc<RequestHook>>,
    on_unknown_request: Option<Arc<RequestHook>>,
    on_unknown_event: Option<Arc<EventHook>>,
}

impl Handler {
    /// Build a dispatch table from generated registrations.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when a method or event is registered more than once.
    pub fn new(
        requests: impl IntoIterator<Item = RequestRegistration>,
        events: impl IntoIterator<Item = EventRegistration>,
    ) -> Result<Self, crate::Error> {
        let mut request_map = HashMap::new();
        for registration in requests {
            if request_map
                .insert(registration.method(), registration)
                .is_some()
            {
                return Err(crate::Error::Configuration(
                    "duplicate request registration".to_owned(),
                ));
            }
        }
        let mut event_map = HashMap::new();
        for registration in events {
            if event_map
                .insert(registration.event(), registration)
                .is_some()
            {
                return Err(crate::Error::Configuration(
                    "duplicate event registration".to_owned(),
                ));
            }
        }
        Ok(Self {
            requests: request_map,
            events: event_map,
            on_begin: Arc::new(|_| Box::pin(async { Ok(()) })),
            middleware: Vec::new(),
            on_unknown_request: None,
            on_unknown_event: None,
        })
    }

    /// Use a lifecycle callback after transport construction and reader startup.
    #[must_use]
    pub fn with_on_begin<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(HandlerContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), crate::Error>> + Send + 'static,
    {
        self.on_begin = Arc::new(move |context| Box::pin(callback(context)));
        self
    }

    /// Add raw request middleware. Middleware runs in insertion order before typed decoding.
    #[must_use]
    pub fn with_request_middleware<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(HandlerContext, InboundRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), crate::Error>> + Send + 'static,
    {
        self.middleware.push(Arc::new(move |context, request| {
            Box::pin(callback(context, request))
        }));
        self
    }

    /// Override the default 501 response for unknown request methods.
    #[must_use]
    pub fn with_unknown_request<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(HandlerContext, InboundRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), crate::Error>> + Send + 'static,
    {
        self.on_unknown_request = Some(Arc::new(move |context, request| {
            Box::pin(callback(context, request))
        }));
        self
    }

    /// Override the default ignore behavior for unknown events.
    #[must_use]
    pub fn with_unknown_event<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(HandlerContext, InboundEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), crate::Error>> + Send + 'static,
    {
        self.on_unknown_event = Some(Arc::new(move |context, event| {
            Box::pin(callback(context, event))
        }));
        self
    }
}

/// Observable session lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionState {
    #[default]
    Inactive,
    Connecting,
    Active,
    Closing,
    Closed,
    Failed,
}

/// Session timing, identity, transport construction, and audio capacity.
pub struct SessionConfig {
    pub id: String,
    pub request_timeout: Duration,
    pub close_timeout: Duration,
    /// Compatibility grace between a terminal response and transport close.
    ///
    /// Published `rtvbp-go v0.37.2` can otherwise observe the close frame before its inbound
    /// control queue dispatches the immediately preceding response.
    pub terminal_close_grace: Duration,
    pub audio_buffer_size: usize,
    pub keepalive: crate::KeepalivePolicy,
    pub transport_factory: Arc<dyn TransportFactory>,
    pub id_generator: Arc<dyn Fn() -> String + Send + Sync>,
}

impl SessionConfig {
    #[must_use]
    pub fn new(transport_factory: Arc<dyn TransportFactory>) -> Self {
        static SESSION_IDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let request_ids = Arc::new(std::sync::atomic::AtomicU64::new(1));
        Self {
            id: format!(
                "session-{}",
                SESSION_IDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            request_timeout: Duration::from_secs(5),
            close_timeout: Duration::from_secs(5),
            terminal_close_grace: Duration::from_millis(100),
            audio_buffer_size: 1024 * 1024,
            keepalive: crate::KeepalivePolicy::default(),
            transport_factory,
            id_generator: Arc::new(move || {
                format!(
                    "request-{}",
                    request_ids.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                )
            }),
        }
    }

    #[must_use]
    pub fn with_transport(transport: Arc<dyn Transport>) -> Self {
        Self::new(Arc::new(FixedTransportFactory(transport)))
    }
}

struct FixedTransportFactory(Arc<dyn Transport>);

#[async_trait]
impl TransportFactory for FixedTransportFactory {
    async fn connect(
        &self,
        _envelope: Arc<dyn Envelope>,
    ) -> Result<Arc<dyn Transport>, crate::Error> {
        Ok(Arc::clone(&self.0))
    }
}

/// Cloneable handle to one session owner.
#[derive(Clone)]
pub struct Session {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    envelope: Arc<dyn Envelope>,
    handler: Handler,
    config: SessionConfig,
    state: RwLock<SessionState>,
    run_started: AtomicBool,
    closing: AtomicBool,
    transport: RwLock<Option<Arc<dyn Transport>>>,
    stop: Mutex<StopState>,
    stop_notify: Notify,
    done_notify: Notify,
    final_error: Mutex<Option<String>>,
    pending: Mutex<PendingRequests>,
    audio: Arc<AudioStream>,
    media: Mutex<MediaBinding>,
    media_tasks: Mutex<Vec<JoinHandle<()>>>,
}

#[derive(Default)]
struct StopState {
    requested: bool,
    failures: Vec<String>,
}

enum MediaBinding {
    Unbound,
    Binding,
    Bound(Arc<dyn MediaChannel>),
}

impl Session {
    #[must_use]
    pub fn new(envelope: Arc<dyn Envelope>, handler: Handler, config: SessionConfig) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                envelope,
                handler,
                audio: Arc::new(AudioStream::new(config.audio_buffer_size)),
                config,
                state: RwLock::new(SessionState::Inactive),
                run_started: AtomicBool::new(false),
                closing: AtomicBool::new(false),
                transport: RwLock::new(None),
                stop: Mutex::new(StopState::default()),
                stop_notify: Notify::new(),
                done_notify: Notify::new(),
                final_error: Mutex::new(None),
                pending: Mutex::new(HashMap::new()),
                media: Mutex::new(MediaBinding::Unbound),
                media_tasks: Mutex::new(Vec::new()),
            }),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.inner.config.id
    }

    #[must_use]
    pub fn state(&self) -> SessionState {
        *read_lock(&self.inner.state)
    }

    #[must_use]
    pub fn audio(&self) -> Arc<AudioStream> {
        Arc::clone(&self.inner.audio)
    }

    /// Own the transport and session workers until terminal shutdown.
    ///
    /// # Errors
    ///
    /// Returns duplicate-run, construction, lifecycle, handler, transport, or shutdown failures.
    pub async fn run(&self) -> Result<(), crate::Error> {
        if self
            .inner
            .run_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(crate::Error::SessionAlreadyRun);
        }
        self.set_state(SessionState::Connecting);
        if let Err(error) = self.inner.config.keepalive.validate() {
            self.request_failure(error.to_string());
            return self.finish_without_transport();
        }

        let stop = self.inner.stop_notify.notified();
        tokio::pin!(stop);
        let transport = tokio::select! {
            result = self.inner.config.transport_factory.connect(Arc::clone(&self.inner.envelope)) => {
                match result {
                    Ok(transport) => transport,
                    Err(error) => {
                        self.request_failure(error.to_string());
                        return self.finish_without_transport();
                    }
                }
            }
            () = &mut stop => return self.finish_without_transport(),
        };
        *write_lock(&self.inner.transport) = Some(Arc::clone(&transport));

        let (dispatch_tx, dispatch_rx) = mpsc::unbounded_channel();
        let mut reader = tokio::spawn(self.clone().read_control(dispatch_tx));
        let mut dispatcher = tokio::spawn(self.clone().dispatch_control(dispatch_rx));
        let keepalive_transport = Arc::clone(&transport);
        let keepalive_policy = self.inner.config.keepalive;
        let mut keepalive = tokio::spawn(async move {
            if keepalive_policy.enabled() && keepalive_transport.supports_keepalive() {
                keepalive_transport
                    .monitor_keepalive(keepalive_policy)
                    .await
            } else {
                std::future::pending().await
            }
        });
        let begin_context = HandlerContext::session(self);
        let begin = Arc::clone(&self.inner.handler.on_begin);
        let mut begin_task = tokio::spawn(async move { begin(begin_context).await });
        let mut reader_finished = false;
        let mut begin_finished = false;

        if !self.stop_requested() {
            tokio::select! {
                result = &mut begin_task => {
                    begin_finished = true;
                    match result {
                        Ok(Ok(())) => self.set_state(SessionState::Active),
                        Ok(Err(error)) => self.request_failure(format!("handler on_begin: {error}")),
                        Err(error) => self.request_failure(format!("handler on_begin task: {error}")),
                    }
                }
                result = &mut reader => {
                    reader_finished = true;
                    self.record_reader_result(result);
                }
                result = &mut keepalive => self.record_keepalive_result(result),
                () = self.inner.stop_notify.notified() => {}
            }
        }

        if self.state() == SessionState::Active && !self.stop_requested() {
            tokio::select! {
                result = &mut reader => {
                    reader_finished = true;
                    self.record_reader_result(result);
                }
                result = &mut keepalive => self.record_keepalive_result(result),
                () = self.inner.stop_notify.notified() => {}
            }
        }

        self.shutdown(
            transport,
            &mut reader,
            reader_finished,
            &mut dispatcher,
            &mut begin_task,
            begin_finished,
            &mut keepalive,
        )
        .await
    }

    /// Request graceful shutdown and wait for finalization.
    ///
    /// # Errors
    ///
    /// Returns the terminal session failure when shutdown completed as failed.
    pub async fn close(&self) -> Result<(), crate::Error> {
        if self
            .inner
            .run_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            self.inner.closing.store(true, Ordering::Release);
            self.set_state(SessionState::Closed);
            self.inner.done_notify.notify_waiters();
            return Ok(());
        }
        self.request_close();
        self.wait_done().await
    }

    async fn wait_done(&self) -> Result<(), crate::Error> {
        loop {
            let notified = self.inner.done_notify.notified();
            match self.state() {
                SessionState::Closed => return Ok(()),
                SessionState::Failed => {
                    let message = mutex_lock(&self.inner.final_error)
                        .clone()
                        .unwrap_or_else(|| "unknown failure".to_owned());
                    return Err(crate::Error::SessionFailed(message));
                }
                _ => notified.await,
            }
        }
    }

    fn finish_without_transport(&self) -> Result<(), crate::Error> {
        self.inner.closing.store(true, Ordering::Release);
        self.fail_pending();
        self.inner.audio.close();
        self.finish_terminal()
    }

    #[allow(clippy::too_many_arguments)]
    async fn shutdown(
        &self,
        transport: Arc<dyn Transport>,
        reader: &mut JoinHandle<Result<(), crate::Error>>,
        reader_finished: bool,
        dispatcher: &mut JoinHandle<()>,
        begin: &mut JoinHandle<Result<(), crate::Error>>,
        begin_finished: bool,
        keepalive: &mut JoinHandle<Result<(), crate::Error>>,
    ) -> Result<(), crate::Error> {
        self.set_state(SessionState::Closing);
        self.inner.closing.store(true, Ordering::Release);
        self.fail_pending();
        self.inner.audio.close();

        let media = match &*mutex_lock(&self.inner.media) {
            MediaBinding::Bound(media) => Some(Arc::clone(media)),
            MediaBinding::Unbound | MediaBinding::Binding => None,
        };
        if let Some(media) = media
            && let Err(error) = media.close().await
        {
            self.request_failure(format!("media close: {error}"));
        }
        let close = tokio::time::timeout(self.inner.config.close_timeout, transport.close()).await;
        match close {
            Ok(Ok(())) => {}
            Ok(Err(error)) => self.request_failure(format!("transport close: {error}")),
            Err(_) => self.request_failure("transport close timed out".to_owned()),
        }

        if !reader_finished {
            reader.abort();
        }
        dispatcher.abort();
        if !begin_finished {
            begin.abort();
        }
        keepalive.abort();
        for task in mutex_lock(&self.inner.media_tasks).drain(..) {
            task.abort();
        }
        self.finish_terminal()
    }

    fn finish_terminal(&self) -> Result<(), crate::Error> {
        let failures = mutex_lock(&self.inner.stop).failures.clone();
        if failures.is_empty() {
            self.set_state(SessionState::Closed);
            self.inner.done_notify.notify_waiters();
            Ok(())
        } else {
            let message = failures.join("; ");
            *mutex_lock(&self.inner.final_error) = Some(message.clone());
            self.set_state(SessionState::Failed);
            self.inner.done_notify.notify_waiters();
            Err(crate::Error::SessionFailed(message))
        }
    }

    fn set_state(&self, state: SessionState) {
        *write_lock(&self.inner.state) = state;
    }

    fn stop_requested(&self) -> bool {
        mutex_lock(&self.inner.stop).requested
    }

    fn request_close(&self) {
        mutex_lock(&self.inner.stop).requested = true;
        self.inner.stop_notify.notify_waiters();
    }

    fn request_failure(&self, message: String) {
        let mut stop = mutex_lock(&self.inner.stop);
        stop.requested = true;
        stop.failures.push(message);
        drop(stop);
        self.inner.stop_notify.notify_waiters();
    }

    fn record_reader_result(
        &self,
        result: Result<Result<(), crate::Error>, tokio::task::JoinError>,
    ) {
        match result {
            Ok(Ok(()) | Err(crate::Error::Closed)) => self.request_close(),
            Ok(Err(error)) => self.request_failure(format!("control reader: {error}")),
            Err(error) if error.is_cancelled() => self.request_close(),
            Err(error) => self.request_failure(format!("control reader task: {error}")),
        }
    }

    fn record_keepalive_result(
        &self,
        result: Result<Result<(), crate::Error>, tokio::task::JoinError>,
    ) {
        match result {
            Ok(Ok(())) => {
                self.request_failure("keepalive monitor stopped without an error".to_owned());
            }
            Ok(Err(error)) => self.request_failure(format!("keepalive: {error}")),
            Err(error) if error.is_cancelled() => self.request_close(),
            Err(error) => self.request_failure(format!("keepalive task: {error}")),
        }
    }

    async fn read_control(
        self,
        dispatch: mpsc::UnboundedSender<ControlFrame>,
    ) -> Result<(), crate::Error> {
        let control = self.control()?;
        loop {
            let received = control.recv().await?;
            let Ok(mut frame) = self.inner.envelope.decode(&received.data) else {
                continue;
            };
            frame.received_at = Some(received.received_at);
            if frame.kind == FrameKind::Response {
                self.resolve_pending(frame);
            } else if dispatch.send(frame).is_err() {
                return Ok(());
            }
        }
    }

    async fn dispatch_control(self, mut dispatch: mpsc::UnboundedReceiver<ControlFrame>) {
        while let Some(frame) = dispatch.recv().await {
            if self.stop_requested() || self.inner.closing.load(Ordering::Acquire) {
                return;
            }
            match frame.kind {
                FrameKind::Request => self.handle_request(frame).await,
                FrameKind::Event => self.handle_event(frame).await,
                FrameKind::Response => {}
            }
        }
    }

    async fn handle_request(&self, frame: ControlFrame) {
        let reply = Arc::new(ReplyState::new(frame.id.clone()));
        let received_at = frame.received_at.unwrap_or_else(SystemTime::now);
        let context = HandlerContext::request(self, Arc::clone(&reply), received_at);
        let request = InboundRequest {
            id: frame.id,
            method: frame.method,
            payload: frame.payload,
            received_at,
        };

        for middleware in &self.inner.handler.middleware {
            if let Err(error) = middleware(context.clone(), request.clone()).await {
                self.respond_handler_error(&context, error).await;
                return;
            }
        }

        if let Some(registration) = self.inner.handler.requests.get(request.method.as_str()) {
            let payload = request
                .payload
                .clone()
                .unwrap_or_else(|| Value::Object(serde_json::Map::default()));
            match registration.handle(context.clone(), payload).await {
                Ok(HandlerReply { payload, terminal }) => {
                    if context.reply_status() == REPLY_UNCLAIMED {
                        let result = context
                            .respond_internal(Some(payload), None, terminal)
                            .await;
                        if let Err(error) = result {
                            self.request_failure(format!("send response: {error}"));
                        }
                    }
                }
                Err(error) => self.respond_handler_error(&context, error).await,
            }
            return;
        }

        let result = if let Some(hook) = &self.inner.handler.on_unknown_request {
            hook(context.clone(), request.clone()).await
        } else {
            Err(crate::Error::Handler(WireError {
                code: 501,
                message: format!("unknown method: {}", request.method),
                data: None,
            }))
        };
        match result {
            Err(error) => self.respond_handler_error(&context, error).await,
            Ok(()) if context.reply_status() == REPLY_UNCLAIMED => {
                self.respond_handler_error(
                    &context,
                    crate::Error::RequestFailed(
                        "request handler returned without responding or deferring".to_owned(),
                    ),
                )
                .await;
            }
            Ok(()) => {}
        }
    }

    async fn respond_handler_error(&self, context: &HandlerContext, error: crate::Error) {
        if context.reply_status() == REPLY_SENT {
            return;
        }
        let wire = match error {
            crate::Error::Handler(wire) => wire,
            other => WireError {
                code: 500,
                message: other.to_string(),
                data: None,
            },
        };
        if let Err(error) = context.respond_internal(None, Some(wire), false).await
            && !matches!(error, crate::Error::SessionClosed)
        {
            self.request_failure(format!("send error response: {error}"));
        }
    }

    async fn handle_event(&self, frame: ControlFrame) {
        let context = HandlerContext::session(self);
        let event = InboundEvent {
            id: frame.id,
            name: frame.method,
            payload: frame.payload,
            received_at: frame.received_at.unwrap_or_else(SystemTime::now),
        };
        if let Some(registration) = self.inner.handler.events.get(event.name.as_str()) {
            let payload = event
                .payload
                .clone()
                .unwrap_or_else(|| Value::Object(serde_json::Map::default()));
            let _ = registration.handle(context, payload).await;
        } else if let Some(hook) = &self.inner.handler.on_unknown_event {
            let _ = hook(context, event).await;
        }
    }

    fn control(&self) -> Result<Arc<dyn crate::ControlChannel>, crate::Error> {
        read_lock(&self.inner.transport)
            .as_ref()
            .map(|transport| transport.control())
            .ok_or(crate::Error::SessionClosed)
    }

    async fn send_frame(&self, frame: ControlFrame) -> Result<(), crate::Error> {
        if self.inner.closing.load(Ordering::Acquire) {
            return Err(crate::Error::SessionClosed);
        }
        let encoded = self.inner.envelope.encode(&frame)?;
        self.control()?.send(encoded).await
    }

    async fn send_response(
        &self,
        correlation_id: String,
        payload: Option<Value>,
        error: Option<WireError>,
    ) -> Result<(), crate::Error> {
        self.send_frame(ControlFrame::response(correlation_id, payload, error))
            .await
    }

    async fn request_value(
        &self,
        method: &'static str,
        payload: Value,
    ) -> Result<Value, crate::Error> {
        if self.inner.closing.load(Ordering::Acquire) {
            return Err(crate::Error::SessionClosed);
        }
        let id = (self.inner.config.id_generator)();
        if id.is_empty() {
            return Err(crate::Error::RequestFailed(
                "id generator returned an empty id".to_owned(),
            ));
        }
        let (sender, mut receiver) = oneshot::channel();
        {
            let mut pending = mutex_lock(&self.inner.pending);
            if pending.insert(id.clone(), sender).is_some() {
                return Err(crate::Error::RequestFailed(format!(
                    "id generator returned duplicate id {id:?}"
                )));
            }
        }
        let frame = ControlFrame::request(id.clone(), method, Some(payload));
        if let Err(error) = self.send_frame(frame).await {
            if self.cancel_pending(&id) {
                return Err(error);
            }
            return receive_pending(&mut receiver).await;
        }

        match tokio::time::timeout(self.inner.config.request_timeout, &mut receiver).await {
            Ok(result) => pending_result(result),
            Err(_) if self.cancel_pending(&id) => Err(crate::Error::RequestTimeout),
            Err(_) => receive_pending(&mut receiver).await,
        }
    }

    fn cancel_pending(&self, id: &str) -> bool {
        mutex_lock(&self.inner.pending).remove(id).is_some()
    }

    fn resolve_pending(&self, frame: ControlFrame) {
        let sender = mutex_lock(&self.inner.pending).remove(&frame.correlation_id);
        if let Some(sender) = sender {
            let result = frame.error.map_or_else(
                || Ok(frame.payload),
                |error| Err(crate::Error::Remote(error)),
            );
            let _ = sender.send(result);
        }
    }

    fn fail_pending(&self) {
        for (_, sender) in mutex_lock(&self.inner.pending).drain() {
            let _ = sender.send(Err(crate::Error::SessionClosed));
        }
    }

    /// Open and bind the session's sole `audio` channel.
    ///
    /// # Errors
    ///
    /// Returns format, lifecycle, duplicate-binding, or transport failures.
    pub async fn open_audio(&self, format: MediaFormat) -> Result<(), crate::Error> {
        format.frame_bytes()?;
        let transport = self.begin_audio_bind(Some(&format))?;
        match transport.open_media("audio", format.clone()).await {
            Ok(channel) => self.finish_audio_bind(channel, Some(&format)),
            Err(error) => {
                self.abort_audio_bind();
                Err(error)
            }
        }
    }

    /// Accept and bind the session's sole `audio` channel.
    ///
    /// # Errors
    ///
    /// Returns lifecycle, duplicate-binding, invalid-format, or transport failures.
    pub async fn accept_audio(&self) -> Result<(), crate::Error> {
        let transport = self.begin_audio_bind(None)?;
        match transport.accept_media().await {
            Ok(channel) => self.finish_audio_bind(channel, None),
            Err(error) => {
                self.abort_audio_bind();
                Err(error)
            }
        }
    }

    fn begin_audio_bind(
        &self,
        requested: Option<&MediaFormat>,
    ) -> Result<Arc<dyn Transport>, crate::Error> {
        if self.inner.closing.load(Ordering::Acquire) {
            return Err(crate::Error::SessionClosed);
        }
        let transport = read_lock(&self.inner.transport)
            .clone()
            .ok_or(crate::Error::AudioUnavailable)?;
        let mut media = mutex_lock(&self.inner.media);
        match &*media {
            MediaBinding::Binding => Err(crate::Error::AudioAlreadyBound),
            MediaBinding::Bound(channel) => {
                if requested.is_some_and(|format| channel.format() != format) {
                    Err(crate::Error::AudioFormatConflict)
                } else {
                    Err(crate::Error::AudioAlreadyBound)
                }
            }
            MediaBinding::Unbound => {
                *media = MediaBinding::Binding;
                Ok(transport)
            }
        }
    }

    fn abort_audio_bind(&self) {
        let mut media = mutex_lock(&self.inner.media);
        if matches!(*media, MediaBinding::Binding) {
            *media = MediaBinding::Unbound;
        }
    }

    fn finish_audio_bind(
        &self,
        channel: Arc<dyn MediaChannel>,
        requested: Option<&MediaFormat>,
    ) -> Result<(), crate::Error> {
        if channel.id() != "audio" {
            self.abort_audio_bind();
            return Err(crate::Error::Configuration(format!(
                "accepted media channel {:?}, want \"audio\"",
                channel.id()
            )));
        }
        channel.format().frame_bytes()?;
        if requested.is_some_and(|format| channel.format() != format) {
            self.abort_audio_bind();
            return Err(crate::Error::AudioFormatConflict);
        }
        self.inner.audio.set_format(channel.format().clone())?;
        *mutex_lock(&self.inner.media) = MediaBinding::Bound(Arc::clone(&channel));
        self.spawn_audio_pumps(channel);
        Ok(())
    }

    fn spawn_audio_pumps(&self, channel: Arc<dyn MediaChannel>) {
        let inbound_session = self.clone();
        let inbound_channel = Arc::clone(&channel);
        let inbound = tokio::spawn(async move {
            loop {
                match inbound_channel.read_frame().await {
                    Ok(frame) => {
                        if inbound_session
                            .inner
                            .audio
                            .push_inbound_frame(frame)
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(crate::Error::Closed) => return,
                    Err(error) => {
                        inbound_session.request_failure(format!("audio read: {error}"));
                        return;
                    }
                }
            }
        });
        let outbound_session = self.clone();
        let outbound = tokio::spawn(async move {
            loop {
                match outbound_session.inner.audio.read_outbound_frame().await {
                    Ok(data) => {
                        if let Err(error) =
                            channel.write_frame(crate::MediaFrame::untimed(data)).await
                        {
                            if !matches!(error, crate::Error::Closed) {
                                outbound_session.request_failure(format!("audio write: {error}"));
                            }
                            return;
                        }
                    }
                    Err(crate::Error::Closed) => return,
                    Err(error) => {
                        outbound_session.request_failure(format!("audio buffer: {error}"));
                        return;
                    }
                }
            }
        });
        mutex_lock(&self.inner.media_tasks).extend([inbound, outbound]);
    }
}

#[async_trait]
impl Requester for Session {
    async fn request(&self, method: &'static str, payload: Value) -> Result<Value, crate::Error> {
        self.request_value(method, payload).await
    }
}

#[async_trait]
impl Notifier for Session {
    async fn notify(&self, event: &'static str, payload: Value) -> Result<(), crate::Error> {
        let id = (self.inner.config.id_generator)();
        self.send_frame(ControlFrame::event(id, event, Some(payload)))
            .await
    }
}

const REPLY_UNCLAIMED: u8 = 0;
const REPLY_DEFERRED: u8 = 1;
const REPLY_SENT: u8 = 2;

struct ReplyState {
    status: AtomicU8,
    request_id: String,
}

impl ReplyState {
    fn new(request_id: String) -> Self {
        Self {
            status: AtomicU8::new(REPLY_UNCLAIMED),
            request_id,
        }
    }
}

struct ContextInner {
    session: Weak<SessionInner>,
    reply: Option<Arc<ReplyState>>,
    received_at: Option<SystemTime>,
}

/// Request-scoped session capability supplied to generated handlers.
#[derive(Clone, Default)]
pub struct HandlerContext {
    inner: Option<Arc<ContextInner>>,
}

impl fmt::Debug for HandlerContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandlerContext")
            .field("attached", &self.inner.is_some())
            .finish()
    }
}

impl HandlerContext {
    fn session(session: &Session) -> Self {
        Self {
            inner: Some(Arc::new(ContextInner {
                session: Arc::downgrade(&session.inner),
                reply: None,
                received_at: None,
            })),
        }
    }

    fn request(session: &Session, reply: Arc<ReplyState>, received_at: SystemTime) -> Self {
        Self {
            inner: Some(Arc::new(ContextInner {
                session: Arc::downgrade(&session.inner),
                reply: Some(reply),
                received_at: Some(received_at),
            })),
        }
    }

    fn attached(&self) -> Result<(Session, &ContextInner), crate::Error> {
        let context = self
            .inner
            .as_deref()
            .ok_or(crate::Error::NoRequestContext)?;
        let inner = context
            .session
            .upgrade()
            .ok_or(crate::Error::SessionClosed)?;
        Ok((Session { inner }, context))
    }

    fn reply_status(&self) -> u8 {
        self.inner
            .as_ref()
            .and_then(|context| context.reply.as_ref())
            .map_or(REPLY_SENT, |reply| reply.status.load(Ordering::Acquire))
    }

    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.inner
            .as_ref()
            .and_then(|context| context.session.upgrade())
            .map(|session| session.config.id.clone())
    }

    #[must_use]
    pub fn state(&self) -> Option<SessionState> {
        self.inner
            .as_ref()
            .and_then(|context| context.session.upgrade())
            .map(|session| *read_lock(&session.state))
    }

    /// Return the transport receive timestamp of the current request.
    #[must_use]
    pub fn received_at(&self) -> Option<SystemTime> {
        self.inner.as_ref().and_then(|inner| inner.received_at)
    }

    #[must_use]
    pub fn audio(&self) -> Option<Arc<AudioStream>> {
        self.inner
            .as_ref()
            .and_then(|context| context.session.upgrade())
            .map(|session| Arc::clone(&session.audio))
    }

    /// Issue a generated typed nested request.
    ///
    /// # Errors
    ///
    /// Returns detached-context, validation, transport, remote, timeout, or decode failures.
    pub async fn request_typed<Q: NamedRequest>(
        &self,
        request: Q,
    ) -> Result<Q::Response, crate::Error> {
        crate::request_peer(self, request).await
    }

    /// Emit a generated typed event.
    ///
    /// # Errors
    ///
    /// Returns detached-context, validation, encoding, or transport failures.
    pub async fn notify_typed<E: NamedEvent>(&self, event: E) -> Result<(), crate::Error> {
        crate::notify_event(self, event).await
    }

    /// Send a successful response for the current inbound request.
    ///
    /// # Errors
    ///
    /// Returns detached-context, duplicate-response, encoding, or transport failures.
    pub async fn respond(&self, payload: Option<Value>) -> Result<(), crate::Error> {
        self.respond_internal(payload, None, false).await
    }

    /// Send a successful response and request graceful shutdown after admission.
    ///
    /// # Errors
    ///
    /// Returns detached-context, duplicate-response, encoding, or transport failures.
    pub async fn respond_then_close(&self, payload: Option<Value>) -> Result<(), crate::Error> {
        self.respond_internal(payload, None, true).await
    }

    /// Send an error response for the current inbound request.
    ///
    /// # Errors
    ///
    /// Returns detached-context, duplicate-response, encoding, or transport failures.
    pub async fn respond_error(&self, error: WireError) -> Result<(), crate::Error> {
        self.respond_internal(None, Some(error), false).await
    }

    async fn respond_internal(
        &self,
        payload: Option<Value>,
        error: Option<WireError>,
        close_after: bool,
    ) -> Result<(), crate::Error> {
        let (session, context) = self.attached()?;
        let reply = context
            .reply
            .as_ref()
            .ok_or(crate::Error::NoRequestContext)?;
        loop {
            let status = reply.status.load(Ordering::Acquire);
            if status == REPLY_SENT {
                return Err(crate::Error::ResponseAlreadySent);
            }
            if reply
                .status
                .compare_exchange(status, REPLY_SENT, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
        session
            .send_response(reply.request_id.clone(), payload, error)
            .await?;
        if close_after {
            tokio::time::sleep(session.inner.config.terminal_close_grace).await;
            session.request_close();
        }
        Ok(())
    }

    /// Claim the current response for later exactly-once completion.
    ///
    /// # Errors
    ///
    /// Returns detached-context or duplicate-response failures.
    pub fn defer_response(&self) -> Result<DeferredResponse, crate::Error> {
        let (_, context) = self.attached()?;
        let reply = context
            .reply
            .as_ref()
            .ok_or(crate::Error::NoRequestContext)?;
        reply
            .status
            .compare_exchange(
                REPLY_UNCLAIMED,
                REPLY_DEFERRED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| crate::Error::ResponseAlreadySent)?;
        Ok(DeferredResponse {
            context: self.clone(),
        })
    }

    /// Open the session audio channel.
    ///
    /// # Errors
    ///
    /// Returns detached-context or audio binding failures.
    pub async fn open_audio(&self, format: MediaFormat) -> Result<(), crate::Error> {
        self.attached()?.0.open_audio(format).await
    }

    /// Accept the session audio channel.
    ///
    /// # Errors
    ///
    /// Returns detached-context or audio binding failures.
    pub async fn accept_audio(&self) -> Result<(), crate::Error> {
        self.attached()?.0.accept_audio().await
    }

    /// Request graceful shutdown without waiting on the current dispatch worker.
    ///
    /// # Errors
    ///
    /// Returns a detached-context failure.
    pub fn close(&self) -> Result<(), crate::Error> {
        self.attached()?.0.request_close();
        Ok(())
    }
}

#[async_trait]
impl Requester for HandlerContext {
    async fn request(&self, method: &'static str, payload: Value) -> Result<Value, crate::Error> {
        self.attached()?.0.request_value(method, payload).await
    }
}

#[async_trait]
impl Notifier for HandlerContext {
    async fn notify(&self, event: &'static str, payload: Value) -> Result<(), crate::Error> {
        self.attached()?.0.notify(event, payload).await
    }
}

/// One deferred exactly-once response handle.
#[derive(Clone, Debug)]
pub struct DeferredResponse {
    context: HandlerContext,
}

impl DeferredResponse {
    /// Complete the deferred response.
    ///
    /// # Errors
    ///
    /// Returns duplicate-response, session, encoding, or transport failures.
    pub async fn respond(&self, payload: Option<Value>) -> Result<(), crate::Error> {
        self.context.respond(payload).await
    }

    /// Complete the deferred response and request graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns duplicate-response, session, encoding, or transport failures.
    pub async fn respond_then_close(&self, payload: Option<Value>) -> Result<(), crate::Error> {
        self.context.respond_then_close(payload).await
    }

    /// Complete the deferred response with a wire error.
    ///
    /// # Errors
    ///
    /// Returns duplicate-response, session, encoding, or transport failures.
    pub async fn respond_error(&self, error: WireError) -> Result<(), crate::Error> {
        self.context.respond_error(error).await
    }
}

async fn receive_pending(
    receiver: &mut oneshot::Receiver<Result<Option<Value>, crate::Error>>,
) -> Result<Value, crate::Error> {
    pending_result(receiver.await)
}

fn pending_result(
    result: Result<Result<Option<Value>, crate::Error>, oneshot::error::RecvError>,
) -> Result<Value, crate::Error> {
    result
        .map_err(|_| crate::Error::SessionClosed)?
        .map(|payload| payload.unwrap_or(Value::Null))
}

fn mutex_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn read_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::v1classic;
    use crate::transport::memory::{Config as MemoryConfig, MemoryTransport};
    use tokio::sync::Barrier;

    fn test_session() -> Session {
        let (transport, _) = MemoryTransport::pair(MemoryConfig::default());
        Session::new(
            Arc::new(v1classic::Envelope),
            Handler::new([], []).unwrap(),
            SessionConfig::with_transport(transport),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pending_response_and_cancellation_have_exactly_one_winner() {
        for iteration in 0..1_000 {
            let session = test_session();
            let id = format!("request-{iteration}");
            let (sender, receiver) = oneshot::channel();
            mutex_lock(&session.inner.pending).insert(id.clone(), sender);
            let start = Arc::new(Barrier::new(3));
            let complete = tokio::spawn({
                let session = session.clone();
                let id = id.clone();
                let start = Arc::clone(&start);
                async move {
                    start.wait().await;
                    session.resolve_pending(ControlFrame::response(
                        id,
                        Some(serde_json::json!({"ok": true})),
                        None,
                    ));
                }
            });
            let cancel = tokio::spawn({
                let session = session.clone();
                let id = id.clone();
                let start = Arc::clone(&start);
                async move {
                    start.wait().await;
                    session.cancel_pending(&id)
                }
            });
            start.wait().await;
            complete.await.unwrap();
            let cancellation_won = cancel.await.unwrap();
            match (cancellation_won, receiver.await) {
                (true, Err(_)) => {}
                (false, Ok(Ok(Some(value)))) => assert_eq!(value, serde_json::json!({"ok": true})),
                result => panic!("iteration {iteration}: invalid race result {result:?}"),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pending_failure_and_cancellation_have_exactly_one_winner() {
        for iteration in 0..1_000 {
            let session = test_session();
            let id = format!("request-{iteration}");
            let (sender, receiver) = oneshot::channel();
            mutex_lock(&session.inner.pending).insert(id.clone(), sender);
            let start = Arc::new(Barrier::new(3));
            let fail = tokio::spawn({
                let session = session.clone();
                let start = Arc::clone(&start);
                async move {
                    start.wait().await;
                    session.fail_pending();
                }
            });
            let cancel = tokio::spawn({
                let session = session.clone();
                let start = Arc::clone(&start);
                async move {
                    start.wait().await;
                    session.cancel_pending(&id)
                }
            });
            start.wait().await;
            fail.await.unwrap();
            let cancellation_won = cancel.await.unwrap();
            match (cancellation_won, receiver.await) {
                (true, Err(_)) | (false, Ok(Err(crate::Error::SessionClosed))) => {}
                result => panic!("iteration {iteration}: invalid race result {result:?}"),
            }
        }
    }
}
