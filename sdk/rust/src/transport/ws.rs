//! Semantic RTVBP transport over WebSocket.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::{Notify, mpsc, oneshot};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async, connect_async};

use super::{
    ControlChannel, KeepalivePolicy, MediaChannel, MediaFormat, MediaFrame, Received, Transport,
    TransportFactory,
};

/// The deployed classic WebSocket/envelope/catalog profile.
pub const DEFAULT_SUBPROTOCOL: &str = "rtvbp.v1";
const STATIC_AUDIO_ID: &str = "audio";

/// Optional configuration for an already-established WebSocket.
#[derive(Clone, Debug, Default)]
pub struct TransportConfig {
    pub audio_format: Option<MediaFormat>,
}

impl TransportConfig {
    fn validate(&self) -> Result<(), crate::Error> {
        if let Some(format) = &self.audio_format {
            format.frame_bytes()?;
        }
        Ok(())
    }
}

/// WebSocket client configuration.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub url: String,
    pub authorization: Option<String>,
    pub headers: Vec<(String, String)>,
    /// `None` offers `rtvbp.v1`; an explicitly empty vector sends no protocol header.
    pub subprotocols: Option<Vec<String>>,
    pub connect_timeout: Duration,
    pub audio_format: MediaFormat,
}

impl ClientConfig {
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            authorization: None,
            headers: Vec::new(),
            subprotocols: None,
            connect_timeout: Duration::from_secs(10),
            audio_format: default_audio_format(),
        }
    }

    fn protocols(&self) -> Vec<String> {
        self.subprotocols
            .clone()
            .unwrap_or_else(|| vec![DEFAULT_SUBPROTOCOL.to_owned()])
    }

    fn validate(&self) -> Result<(), crate::Error> {
        if self.url.is_empty() {
            return Err(crate::Error::Configuration(
                "WebSocket URL must not be empty".to_owned(),
            ));
        }
        if self.connect_timeout.is_zero() {
            return Err(crate::Error::Configuration(
                "WebSocket connect timeout must be positive".to_owned(),
            ));
        }
        self.audio_format.frame_bytes()?;
        if self
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("sec-websocket-protocol"))
        {
            return Err(crate::Error::Configuration(
                "configure WebSocket protocols through ClientConfig.subprotocols".to_owned(),
            ));
        }
        Ok(())
    }
}

/// A reusable session transport factory backed by [`ClientConfig`].
pub struct ClientFactory {
    config: ClientConfig,
}

impl ClientFactory {
    #[must_use]
    pub const fn new(config: ClientConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl TransportFactory for ClientFactory {
    async fn connect(
        &self,
        _envelope: Arc<dyn crate::Envelope>,
    ) -> Result<Arc<dyn Transport>, crate::Error> {
        Ok(connect(self.config.clone()).await? as Arc<dyn Transport>)
    }
}

/// One synchronous authentication rejection produced before the upgrade.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthRejection {
    pub status: StatusCode,
    pub message: String,
}

/// Authentication callback for [`Server`]. It executes synchronously before WebSocket upgrade.
pub type Authenticator = Arc<dyn Fn(&Request) -> Result<(), AuthRejection> + Send + Sync + 'static>;

/// Listener and accepted-transport configuration.
#[derive(Clone)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub subprotocols: Option<Vec<String>>,
    pub transport: TransportConfig,
    pub authenticate: Authenticator,
}

impl ServerConfig {
    #[must_use]
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            subprotocols: None,
            transport: TransportConfig::default(),
            authenticate: Arc::new(|_| Ok(())),
        }
    }
}

impl AuthRejection {
    #[must_use]
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
}

/// Connect and start a semantic WebSocket transport.
///
/// # Errors
///
/// Returns configuration, timeout, handshake, or transport-construction failures.
pub async fn connect(config: ClientConfig) -> Result<Arc<WsTransport>, crate::Error> {
    config.validate()?;
    let protocols = config.protocols();
    let mut request = config
        .url
        .as_str()
        .into_client_request()
        .map_err(transport_error)?;
    request.headers_mut().insert(
        "user-agent",
        HeaderValue::from_static("babelforce/rtvbp-rust"),
    );
    if let Some(authorization) = config.authorization {
        request.headers_mut().insert(
            "authorization",
            HeaderValue::from_str(&authorization).map_err(configuration_error)?,
        );
    }
    for (name, value) in config.headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(configuration_error)?;
        let value = HeaderValue::from_str(&value).map_err(configuration_error)?;
        request.headers_mut().append(name, value);
    }
    if !protocols.is_empty() {
        request.headers_mut().insert(
            "sec-websocket-protocol",
            HeaderValue::from_str(&protocols.join(", ")).map_err(configuration_error)?,
        );
    }

    let result = tokio::time::timeout(config.connect_timeout, connect_async(request))
        .await
        .map_err(|_| crate::Error::Timeout)?
        .map_err(transport_error)?;
    let (stream, response) = result;
    let wire_subprotocol = selected_protocol(response.headers())?;
    if !wire_subprotocol.is_empty() && !protocols.iter().any(|item| item == &wire_subprotocol) {
        return Err(crate::Error::UnsupportedSubprotocol(wire_subprotocol));
    }
    WsTransport::start(
        stream,
        &wire_subprotocol,
        TransportConfig {
            audio_format: Some(config.audio_format),
        },
    )
}

/// Authenticate, negotiate, and upgrade one accepted byte stream.
///
/// Authentication runs before any upgrade response is generated. Server preference determines the
/// selected protocol. A headerless peer gets the backward-compatible `rtvbp.v1` effective profile.
///
/// # Errors
///
/// Returns authentication, negotiation, handshake, or transport-construction failures.
#[allow(clippy::result_large_err)]
pub async fn accept<S, A>(
    stream: S,
    supported_subprotocols: Option<Vec<String>>,
    config: TransportConfig,
    authenticate: A,
) -> Result<Arc<WsTransport>, crate::Error>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    A: Fn(&Request) -> Result<(), AuthRejection> + Send + Unpin + 'static,
{
    config.validate()?;
    let supported = supported_subprotocols.unwrap_or_else(|| vec![DEFAULT_SUBPROTOCOL.to_owned()]);
    let selected = Arc::new(Mutex::new(String::new()));
    let captured = Arc::clone(&selected);
    let callback = move |request: &Request, mut response: Response| {
        if let Err(rejection) = authenticate(request) {
            return Err(error_response(rejection.status, rejection.message));
        }
        let offered = offered_protocols(request);
        if !offered.is_empty() {
            let Some(protocol) = supported
                .iter()
                .find(|supported| offered.iter().any(|offered| offered == *supported))
                .cloned()
            else {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "unsupported WebSocket subprotocol: offered {offered:?}, supported {supported:?}"
                    ),
                ));
            };
            let header = HeaderValue::from_str(&protocol).map_err(|error| {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            })?;
            response
                .headers_mut()
                .insert("sec-websocket-protocol", header);
            *mutex_lock(&captured) = protocol;
        }
        Ok(response)
    };
    let stream = accept_hdr_async(stream, callback)
        .await
        .map_err(transport_error)?;
    let wire_subprotocol = mutex_lock(&selected).clone();
    WsTransport::start(stream, &wire_subprotocol, config)
}

/// A drain-safe semantic transport over one established WebSocket.
pub struct WsTransport {
    wire_subprotocol: String,
    effective_subprotocol: String,
    control: Arc<WsControl>,
    media: Arc<WsMedia>,
    outgoing: Mutex<OutgoingState>,
    terminal: Mutex<Option<Terminal>>,
    done: Notify,
    pongs_tx: mpsc::UnboundedSender<Vec<u8>>,
    pongs_rx: tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<Vec<u8>>>>,
    ping_serial: AtomicU64,
    media_claimed: AtomicBool,
}

#[derive(Clone, Debug)]
enum Terminal {
    Orderly,
    Failed(String),
}

struct OutgoingState {
    sender: Option<mpsc::UnboundedSender<Outbound>>,
}

struct Outbound {
    message: Message,
    close: bool,
    written: Option<oneshot::Sender<Result<(), String>>>,
}

impl WsTransport {
    fn start<S>(
        stream: WebSocketStream<S>,
        wire_subprotocol: &str,
        config: TransportConfig,
    ) -> Result<Arc<Self>, crate::Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        config.validate()?;
        let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();
        let (pongs_tx, pongs_rx) = mpsc::unbounded_channel();
        let format = config.audio_format.unwrap_or_else(default_audio_format);
        let transport = Arc::new_cyclic(|weak| Self {
            wire_subprotocol: wire_subprotocol.to_owned(),
            effective_subprotocol: if wire_subprotocol.is_empty() {
                DEFAULT_SUBPROTOCOL.to_owned()
            } else {
                wire_subprotocol.to_owned()
            },
            control: Arc::new(WsControl {
                transport: weak.clone(),
                incoming: Arc::new(Inbox::new()),
            }),
            media: Arc::new(WsMedia {
                transport: weak.clone(),
                format,
                incoming: Arc::new(Inbox::new()),
                closed: AtomicBool::new(false),
            }),
            outgoing: Mutex::new(OutgoingState {
                sender: Some(outgoing_tx),
            }),
            terminal: Mutex::new(None),
            done: Notify::new(),
            pongs_tx,
            pongs_rx: tokio::sync::Mutex::new(Some(pongs_rx)),
            ping_serial: AtomicU64::new(0),
            media_claimed: AtomicBool::new(false),
        });
        let (writer, reader) = stream.split();
        tokio::spawn(write_pump(Arc::clone(&transport), outgoing_rx, writer));
        tokio::spawn(read_pump(Arc::clone(&transport), reader));
        Ok(transport)
    }

    #[must_use]
    pub fn subprotocol(&self) -> &str {
        &self.effective_subprotocol
    }

    #[must_use]
    pub fn wire_subprotocol(&self) -> &str {
        &self.wire_subprotocol
    }

    /// Wait until the socket reaches an orderly or failed terminal state.
    ///
    /// # Errors
    ///
    /// Returns the normalized transport failure.
    pub async fn wait_closed(&self) -> Result<(), crate::Error> {
        loop {
            let notified = self.done.notified();
            if let Some(terminal) = mutex_lock(&self.terminal).clone() {
                return terminal.result();
            }
            notified.await;
        }
    }

    fn enqueue(&self, message: Message) -> Result<(), crate::Error> {
        self.enqueue_command(Outbound {
            message,
            close: false,
            written: None,
        })
    }

    fn enqueue_command(&self, command: Outbound) -> Result<(), crate::Error> {
        let outgoing = mutex_lock(&self.outgoing);
        let Some(sender) = &outgoing.sender else {
            return Err(self.closed_error());
        };
        sender.send(command).map_err(|_| self.closed_error())
    }

    async fn enqueue_acknowledged(&self, message: Message) -> Result<(), crate::Error> {
        let (written_tx, written_rx) = oneshot::channel();
        self.enqueue_command(Outbound {
            message,
            close: false,
            written: Some(written_tx),
        })?;
        written_rx
            .await
            .map_err(|_| self.closed_error())?
            .map_err(crate::Error::Transport)
    }

    fn finish(&self, terminal: Terminal) {
        let first = {
            let mut state = mutex_lock(&self.terminal);
            if state.is_some() {
                false
            } else {
                *state = Some(terminal.clone());
                true
            }
        };
        if !first {
            return;
        }
        mutex_lock(&self.outgoing).sender.take();
        self.control.incoming.close(terminal.clone());
        self.media.close_from_transport(terminal);
        self.done.notify_waiters();
    }

    fn closed_error(&self) -> crate::Error {
        mutex_lock(&self.terminal)
            .clone()
            .unwrap_or(Terminal::Orderly)
            .error()
    }

    fn claim_media(&self) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        if mutex_lock(&self.terminal).is_some() {
            return Err(self.closed_error());
        }
        if self.media.closed.load(Ordering::Acquire) {
            return Err(crate::Error::Closed);
        }
        if self
            .media_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(crate::Error::MediaAlreadyOpen);
        }
        Ok(Arc::clone(&self.media) as Arc<dyn MediaChannel>)
    }
}

/// Authenticated WebSocket listener with an active-transport shutdown registry.
pub struct Server {
    local_addr: SocketAddr,
    config: ServerConfig,
    accepted_tx: Mutex<Option<mpsc::UnboundedSender<Arc<WsTransport>>>>,
    accepted_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<Arc<WsTransport>>>,
    active: Mutex<HashMap<u64, Arc<WsTransport>>>,
    serial: AtomicU64,
    stopping: AtomicBool,
    stop: Notify,
    admissions: std::sync::atomic::AtomicUsize,
    admission_idle: Notify,
    listener_finished: AtomicBool,
    listener_done: Notify,
}

impl Server {
    /// Bind and start accepting connections.
    ///
    /// # Errors
    ///
    /// Returns invalid transport configuration or TCP bind failures.
    pub async fn bind(config: ServerConfig) -> Result<Arc<Self>, crate::Error> {
        config.transport.validate()?;
        let listener = TcpListener::bind(config.addr)
            .await
            .map_err(transport_error)?;
        let local_addr = listener.local_addr().map_err(transport_error)?;
        let (accepted_tx, accepted_rx) = mpsc::unbounded_channel();
        let server = Arc::new(Self {
            local_addr,
            config,
            accepted_tx: Mutex::new(Some(accepted_tx)),
            accepted_rx: tokio::sync::Mutex::new(accepted_rx),
            active: Mutex::new(HashMap::new()),
            serial: AtomicU64::new(1),
            stopping: AtomicBool::new(false),
            stop: Notify::new(),
            admissions: std::sync::atomic::AtomicUsize::new(0),
            admission_idle: Notify::new(),
            listener_finished: AtomicBool::new(false),
            listener_done: Notify::new(),
        });
        tokio::spawn(accept_loop(Arc::clone(&server), listener));
        Ok(server)
    }

    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    #[must_use]
    pub fn url(&self) -> String {
        format!("ws://{}", self.local_addr)
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        mutex_lock(&self.active).len()
    }

    /// Wait for the next authenticated, upgraded transport.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] once shutdown closes admission.
    pub async fn accept(&self) -> Result<Arc<WsTransport>, crate::Error> {
        self.accepted_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(crate::Error::Closed)
    }

    /// Stop admission, close every active transport, and wait for in-flight upgrades.
    ///
    /// # Errors
    ///
    /// Returns joined active-transport close failures.
    pub async fn shutdown(&self) -> Result<(), crate::Error> {
        if !self.stopping.swap(true, Ordering::AcqRel) {
            self.stop.notify_waiters();
        }
        self.wait_listener().await;
        self.wait_admissions().await;
        mutex_lock(&self.accepted_tx).take();

        let transports: Vec<_> = mutex_lock(&self.active).values().cloned().collect();
        let mut failures = Vec::new();
        for transport in transports {
            if let Err(error) = transport.close().await {
                failures.push(error.to_string());
            }
        }
        self.wait_active_empty().await;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(crate::Error::Transport(failures.join("; ")))
        }
    }

    async fn wait_listener(&self) {
        while !self.listener_finished.load(Ordering::Acquire) {
            let notified = self.listener_done.notified();
            if self.listener_finished.load(Ordering::Acquire) {
                break;
            }
            notified.await;
        }
    }

    async fn wait_admissions(&self) {
        while self.admissions.load(Ordering::Acquire) != 0 {
            let notified = self.admission_idle.notified();
            if self.admissions.load(Ordering::Acquire) == 0 {
                break;
            }
            notified.await;
        }
    }

    async fn wait_active_empty(&self) {
        while !mutex_lock(&self.active).is_empty() {
            let notified = self.admission_idle.notified();
            if mutex_lock(&self.active).is_empty() {
                break;
            }
            notified.await;
        }
    }
}

async fn accept_loop(server: Arc<Server>, listener: TcpListener) {
    loop {
        tokio::select! {
            () = server.stop.notified() => break,
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else {
                    break;
                };
                if server.stopping.load(Ordering::Acquire) {
                    break;
                }
                server.admissions.fetch_add(1, Ordering::AcqRel);
                let admission_server = Arc::clone(&server);
                tokio::spawn(async move {
                    run_admission(admission_server, stream).await;
                });
            }
        }
    }
    server.listener_finished.store(true, Ordering::Release);
    server.listener_done.notify_waiters();
}

async fn run_admission(server: Arc<Server>, stream: tokio::net::TcpStream) {
    let authentication = Arc::clone(&server.config.authenticate);
    let result = accept(
        stream,
        server.config.subprotocols.clone(),
        server.config.transport.clone(),
        move |request| authentication(request),
    )
    .await;
    if let Ok(transport) = result {
        if server.stopping.load(Ordering::Acquire) {
            let _ = transport.close().await;
        } else {
            let id = server.serial.fetch_add(1, Ordering::Relaxed);
            mutex_lock(&server.active).insert(id, Arc::clone(&transport));
            let admitted = mutex_lock(&server.accepted_tx)
                .as_ref()
                .is_some_and(|sender| sender.send(Arc::clone(&transport)).is_ok());
            if admitted {
                let weak = Arc::downgrade(&server);
                tokio::spawn(async move {
                    let _ = transport.wait_closed().await;
                    if let Some(server) = weak.upgrade() {
                        mutex_lock(&server.active).remove(&id);
                        server.admission_idle.notify_waiters();
                    }
                });
            } else {
                mutex_lock(&server.active).remove(&id);
                let _ = transport.close().await;
            }
        }
    }
    if server.admissions.fetch_sub(1, Ordering::AcqRel) == 1 {
        server.admission_idle.notify_waiters();
    }
}

impl Terminal {
    fn result(self) -> Result<(), crate::Error> {
        match self {
            Self::Orderly => Ok(()),
            Self::Failed(message) => Err(crate::Error::Transport(message)),
        }
    }

    fn error(self) -> crate::Error {
        match self {
            Self::Orderly => crate::Error::Closed,
            Self::Failed(message) => crate::Error::Transport(message),
        }
    }
}

async fn write_pump<S>(
    transport: Arc<WsTransport>,
    mut outgoing: mpsc::UnboundedReceiver<Outbound>,
    mut writer: S,
) where
    S: Sink<Message, Error = WebSocketError> + Unpin,
{
    while let Some(command) = outgoing.recv().await {
        let result = writer
            .send(command.message)
            .await
            .map_err(|error| error.to_string());
        if let Some(written) = command.written {
            let _ = written.send(result.clone());
        }
        if command.close {
            transport.finish(match result {
                Ok(()) => Terminal::Orderly,
                Err(message) => normalize_error_message(message),
            });
            return;
        }
        if let Err(message) = result {
            transport.finish(normalize_error_message(message));
            return;
        }
    }
    let _ = writer.close().await;
}

async fn read_pump<S>(transport: Arc<WsTransport>, mut reader: S)
where
    S: Stream<Item = Result<Message, WebSocketError>> + Unpin,
{
    while let Some(message) = reader.next().await {
        match message {
            Ok(Message::Text(data)) => {
                let _ = transport.control.incoming.push(Received {
                    data: data.as_bytes().to_vec(),
                    received_at: SystemTime::now(),
                });
            }
            Ok(Message::Binary(data)) => {
                let _ = transport
                    .media
                    .incoming
                    .push(MediaFrame::untimed(data.to_vec()));
            }
            // Tungstenite queues and flushes the protocol-mandated Pong from its read path.
            // Enqueuing another one here would duplicate every response.
            Ok(Message::Ping(_) | Message::Frame(_)) => {}
            Ok(Message::Pong(data)) => {
                let _ = transport.pongs_tx.send(data.to_vec());
            }
            Ok(Message::Close(_)) => {
                transport.finish(Terminal::Orderly);
                return;
            }
            Err(error) => {
                transport.finish(normalize_socket_error(error));
                return;
            }
        }
    }
    transport.finish(Terminal::Orderly);
}

struct WsControl {
    transport: Weak<WsTransport>,
    incoming: Arc<Inbox<Received>>,
}

#[async_trait]
impl ControlChannel for WsControl {
    async fn send(&self, data: Vec<u8>) -> Result<(), crate::Error> {
        let text = String::from_utf8(data)
            .map_err(|error| crate::Error::Transport(format!("control is not UTF-8: {error}")))?;
        self.transport
            .upgrade()
            .ok_or(crate::Error::Closed)?
            .enqueue(Message::Text(text.into()))
    }

    async fn recv(&self) -> Result<Received, crate::Error> {
        self.incoming.pop().await
    }
}

struct WsMedia {
    transport: Weak<WsTransport>,
    format: MediaFormat,
    incoming: Arc<Inbox<MediaFrame>>,
    closed: AtomicBool,
}

impl WsMedia {
    fn close_from_transport(&self, terminal: Terminal) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.incoming.close(terminal);
        }
    }
}

#[async_trait]
impl MediaChannel for WsMedia {
    fn id(&self) -> &str {
        STATIC_AUDIO_ID
    }

    fn format(&self) -> &MediaFormat {
        &self.format
    }

    async fn write_frame(&self, frame: MediaFrame) -> Result<(), crate::Error> {
        if self.closed.load(Ordering::Acquire) {
            return Err(crate::Error::Closed);
        }
        self.transport
            .upgrade()
            .ok_or(crate::Error::Closed)?
            .enqueue(Message::Binary(frame.data.into()))
    }

    async fn read_frame(&self) -> Result<MediaFrame, crate::Error> {
        self.incoming.pop().await
    }

    async fn close(&self) -> Result<(), crate::Error> {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.incoming.close(Terminal::Orderly);
        }
        Ok(())
    }
}

#[async_trait]
impl Transport for WsTransport {
    fn control(&self) -> Arc<dyn ControlChannel> {
        Arc::clone(&self.control) as Arc<dyn ControlChannel>
    }

    async fn accept_media(&self) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        self.claim_media()
    }

    async fn open_media(
        &self,
        id: &str,
        format: MediaFormat,
    ) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        if id != STATIC_AUDIO_ID {
            return Err(crate::Error::MediaUnsupported);
        }
        format.frame_bytes()?;
        if format != self.media.format {
            return Err(crate::Error::AudioFormatConflict);
        }
        self.claim_media()
    }

    async fn close(&self) -> Result<(), crate::Error> {
        if let Some(terminal) = mutex_lock(&self.terminal).clone() {
            return terminal.result();
        }
        let (written_tx, written_rx) = oneshot::channel();
        let admitted = {
            let mut outgoing = mutex_lock(&self.outgoing);
            outgoing.sender.take().is_some_and(|sender| {
                sender
                    .send(Outbound {
                        message: Message::Close(Some(CloseFrame {
                            code: CloseCode::Normal,
                            reason: "Closed".into(),
                        })),
                        close: true,
                        written: Some(written_tx),
                    })
                    .is_ok()
            })
        };
        if admitted {
            match written_rx.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(message)) => Err(crate::Error::Transport(message)),
                Err(_) => self.wait_closed().await,
            }
        } else {
            self.wait_closed().await
        }
    }

    fn supports_keepalive(&self) -> bool {
        true
    }

    async fn monitor_keepalive(&self, policy: KeepalivePolicy) -> Result<(), crate::Error> {
        policy.validate()?;
        if !policy.enabled() {
            return Ok(());
        }
        let mut pongs = self.pongs_rx.lock().await.take().ok_or_else(|| {
            crate::Error::Configuration("keepalive monitor already running".to_owned())
        })?;
        let mut misses = 0_usize;
        loop {
            tokio::select! {
                () = tokio::time::sleep(policy.interval) => {}
                closed = self.wait_closed() => return closed,
            }
            while pongs.try_recv().is_ok() {}
            let serial = self.ping_serial.fetch_add(1, Ordering::Relaxed) + 1;
            let payload = format!("rtvbp:{serial}").into_bytes();
            tokio::select! {
                result = self.enqueue_acknowledged(Message::Ping(payload.clone().into())) => {
                    if let Err(error) = result {
                        return mutex_lock(&self.terminal).clone().map_or(Err(error), Terminal::result);
                    }
                }
                closed = self.wait_closed() => return closed,
            }
            let matched = tokio::select! {
                () = tokio::time::sleep(policy.timeout) => false,
                closed = self.wait_closed() => return closed,
                pong = async {
                    loop {
                        match pongs.recv().await {
                            Some(pong) if pong == payload => return true,
                            Some(_) => {}
                            None => return false,
                        }
                    }
                } => pong,
            };
            if matched {
                misses = 0;
            } else {
                misses += 1;
                if misses >= policy.max_misses {
                    self.finish(Terminal::Failed("keepalive timed out".to_owned()));
                    return Err(crate::Error::KeepaliveTimeout);
                }
            }
        }
    }
}

struct Inbox<T> {
    state: Mutex<InboxState<T>>,
    ready: Notify,
}

struct InboxState<T> {
    items: VecDeque<T>,
    terminal: Option<Terminal>,
}

impl<T> Inbox<T> {
    fn new() -> Self {
        Self {
            state: Mutex::new(InboxState {
                items: VecDeque::new(),
                terminal: None,
            }),
            ready: Notify::new(),
        }
    }

    fn push(&self, item: T) -> Result<(), crate::Error> {
        let mut state = mutex_lock(&self.state);
        if let Some(terminal) = state.terminal.clone() {
            return Err(terminal.error());
        }
        state.items.push_back(item);
        drop(state);
        self.ready.notify_one();
        Ok(())
    }

    async fn pop(&self) -> Result<T, crate::Error> {
        loop {
            let notified = self.ready.notified();
            {
                let mut state = mutex_lock(&self.state);
                if let Some(item) = state.items.pop_front() {
                    return Ok(item);
                }
                if let Some(terminal) = state.terminal.clone() {
                    return Err(terminal.error());
                }
            }
            notified.await;
        }
    }

    fn close(&self, terminal: Terminal) {
        let mut state = mutex_lock(&self.state);
        if state.terminal.is_none() {
            state.terminal = Some(terminal);
        }
        drop(state);
        self.ready.notify_waiters();
    }
}

fn default_audio_format() -> MediaFormat {
    MediaFormat {
        encoding: "L16".to_owned(),
        sample_rate: 8_000,
        bit_depth: 16,
        channels: 1,
        ptime: Duration::from_millis(20),
    }
}

fn mutex_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn offered_protocols(request: &Request) -> Vec<String> {
    request
        .headers()
        .get_all("sec-websocket-protocol")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn selected_protocol(
    headers: &tokio_tungstenite::tungstenite::http::HeaderMap,
) -> Result<String, crate::Error> {
    headers
        .get("sec-websocket-protocol")
        .map_or(Ok(String::new()), |value| {
            value.to_str().map(str::to_owned).map_err(transport_error)
        })
}

fn error_response(status: StatusCode, message: String) -> ErrorResponse {
    tokio_tungstenite::tungstenite::http::Response::builder()
        .status(status)
        .body(Some(message.clone()))
        .unwrap_or_else(|_| tokio_tungstenite::tungstenite::http::Response::new(Some(message)))
}

fn normalize_socket_error(error: WebSocketError) -> Terminal {
    match error {
        WebSocketError::ConnectionClosed
        | WebSocketError::AlreadyClosed
        | WebSocketError::Protocol(
            tokio_tungstenite::tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
        ) => Terminal::Orderly,
        other => Terminal::Failed(other.to_string()),
    }
}

fn normalize_error_message(message: String) -> Terminal {
    if message.contains("Connection closed") || message.contains("closed") {
        Terminal::Orderly
    } else {
        Terminal::Failed(message)
    }
}

fn transport_error(error: impl std::fmt::Display) -> crate::Error {
    crate::Error::Transport(error.to_string())
}

fn configuration_error(error: impl std::fmt::Display) -> crate::Error {
    crate::Error::Configuration(error.to_string())
}
