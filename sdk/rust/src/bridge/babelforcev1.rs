//! Operational bridge for the frozen `babelforce.v1` catalog.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::catalog::babelforcev1 as catalog;
use crate::{AudioObserver, Handler, HandlerContext, MediaFormat, SessionState};

/// Default packetization interval used by the v1 bridge.
pub const DEFAULT_PTIME: Duration = Duration::from_millis(20);

type AudioFuture = Pin<Box<dyn Future<Output = Result<(), crate::Error>> + Send>>;
type AudioHook = dyn Fn(HandlerContext) -> AudioFuture + Send + Sync;

/// Voice-side bridge identity and media configuration.
#[derive(Clone, Debug)]
pub struct VoiceBridgeConfig {
    pub call: catalog::CallInfo,
    pub application: catalog::AppInfo,
    pub metadata: Option<Map<String, Value>>,
    pub audio_format: MediaFormat,
}

impl VoiceBridgeConfig {
    #[must_use]
    pub fn new(call: catalog::CallInfo, application: catalog::AppInfo) -> Self {
        Self {
            call,
            application,
            metadata: None,
            audio_format: default_media_format(),
        }
    }
}

/// Callback registered with a telephony implementation for DTMF events.
pub type DtmfCallback = Arc<dyn Fn(catalog::DtmfEvent) + Send + Sync>;
/// Callback registered with a telephony implementation for hangup events.
pub type HangupCallback = Arc<dyn Fn(catalog::CallHangupEvent) + Send + Sync>;

/// The non-protocol telephony operations required by [`VoiceBridge`].
#[async_trait]
pub trait TelephonyAdapter: Send + Sync {
    async fn application_move(
        &self,
        request: catalog::ApplicationMoveRequest,
    ) -> Result<catalog::ApplicationMoveResponse, crate::Error>;

    async fn hangup(&self, request: catalog::CallHangupRequest) -> Result<(), crate::Error>;

    async fn session_variables_set(
        &self,
        request: catalog::SessionSetRequest,
    ) -> Result<(), crate::Error>;

    async fn session_variables_get(
        &self,
        request: catalog::SessionGetRequest,
    ) -> Result<Map<String, Value>, crate::Error>;

    async fn recording_start(
        &self,
        request: catalog::RecordingStartRequest,
    ) -> Result<catalog::RecordingStartResponse, crate::Error>;

    async fn recording_stop(&self, recording_id: String) -> Result<(), crate::Error>;

    /// Register the sole DTMF callback.
    ///
    /// # Errors
    ///
    /// Returns adapter registration failures.
    fn on_dtmf(&self, callback: DtmfCallback) -> Result<(), crate::Error>;

    /// Register the sole hangup callback.
    ///
    /// # Errors
    ///
    /// Returns adapter registration failures.
    fn on_hangup(&self, callback: HangupCallback) -> Result<(), crate::Error>;
}

/// Voice-side generated-role implementation plus the v1 initialization policy.
pub struct VoiceBridge {
    telephony: Arc<dyn TelephonyAdapter>,
    config: VoiceBridgeConfig,
    audio_hook: RwLock<Arc<AudioHook>>,
    initialized: AtomicBool,
    initializing: AtomicBool,
    context: Mutex<Option<HandlerContext>>,
    dtmf_sequence: Arc<AtomicI64>,
    observation_interval: Mutex<Option<Duration>>,
}

impl VoiceBridge {
    #[must_use]
    pub fn new(telephony: Arc<dyn TelephonyAdapter>, config: VoiceBridgeConfig) -> Arc<Self> {
        Arc::new(Self {
            telephony,
            config,
            audio_hook: RwLock::new(Arc::new(|_| Box::pin(async { Ok(()) }))),
            initialized: AtomicBool::new(false),
            initializing: AtomicBool::new(false),
            context: Mutex::new(None),
            dtmf_sequence: Arc::new(AtomicI64::new(0)),
            observation_interval: Mutex::new(None),
        })
    }

    /// Run an application callback after negotiated audio is bound.
    pub fn set_audio_hook<F, Fut>(&self, hook: F)
    where
        F: Fn(HandlerContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), crate::Error>> + Send + 'static,
    {
        *write_lock(&self.audio_hook) = Arc::new(move |context| Box::pin(hook(context)));
    }

    /// Emit generated `audio.info` events at `interval` after initialization.
    ///
    /// # Errors
    ///
    /// Returns a configuration error for a zero interval or after initialization has started.
    pub fn observe_audio(&self, interval: Duration) -> Result<(), crate::Error> {
        if interval.is_zero() {
            return Err(crate::Error::Configuration(
                "audio observation interval must be positive".to_owned(),
            ));
        }
        if self.initializing.load(Ordering::Acquire) || self.initialized.load(Ordering::Acquire) {
            return Err(crate::Error::Configuration(
                "audio observation must be configured before session initialization".to_owned(),
            ));
        }
        *mutex_lock(&self.observation_interval) = Some(interval);
        Ok(())
    }

    /// Build the runtime handler from generated Voice registrations.
    ///
    /// # Errors
    ///
    /// Returns duplicate-registration configuration failures.
    pub fn handler(self: &Arc<Self>) -> Result<Handler, crate::Error> {
        let role: Arc<dyn catalog::VoiceHandler> = Arc::clone(self) as Arc<_>;
        let bridge = Arc::clone(self);
        let begin_bridge = Arc::clone(self);
        Ok(Handler::new(catalog::voice_handlers(role), [])?
            .with_request_middleware(move |_, _| {
                let initialized = bridge.initialized.load(Ordering::Acquire);
                async move {
                    if initialized {
                        Ok(())
                    } else {
                        Err(crate::Error::Handler(crate::WireError {
                            code: 500,
                            message: "session not initialized".to_owned(),
                            data: None,
                        }))
                    }
                }
            })
            .with_on_begin(move |context| {
                let bridge = Arc::clone(&begin_bridge);
                async move { bridge.begin(context).await }
            }))
    }

    /// Ask the application peer to terminate this initialized session.
    ///
    /// # Errors
    ///
    /// Returns initialization, timeout, validation, remote, or transport failures.
    pub async fn terminate(&self, reason: impl Into<String>) -> Result<(), crate::Error> {
        let context = mutex_lock(&self.context)
            .clone()
            .ok_or_else(|| crate::Error::SessionFailed("session not initialized".to_owned()))?;
        let request = catalog::SessionTerminateRequest {
            reason: reason.into(),
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            catalog::ApplicationPeer::new(context).session_terminate(request),
        )
        .await
        .map_err(|_| crate::Error::Timeout)??;
        Ok(())
    }

    async fn begin(self: Arc<Self>, context: HandlerContext) -> Result<(), crate::Error> {
        if self.initializing.swap(true, Ordering::AcqRel) {
            return Err(crate::Error::SessionFailed(
                "session already initialized".to_owned(),
            ));
        }
        self.config.audio_format.frame_bytes()?;
        let response = catalog::ApplicationPeer::new(context.clone())
            .session_initialize(catalog::SessionInitializeRequest {
                application: self.config.application.clone(),
                call: self.config.call.clone(),
                audio_codec_offerings: vec![audio_codec(&self.config.audio_format)],
                metadata: self.config.metadata.clone(),
            })
            .await?;
        let selected = media_format(
            response.audio_codec.as_ref(),
            self.config.audio_format.ptime,
        )?;
        if selected != self.config.audio_format {
            return Err(crate::Error::AudioFormatConflict);
        }
        context.accept_audio().await?;
        *mutex_lock(&self.context) = Some(context.clone());
        self.initialized.store(true, Ordering::Release);

        catalog::VoiceEvents::new(context.clone())
            .session_updated(catalog::SessionUpdatedEvent {
                audio_codec: response.audio_codec,
            })
            .await?;
        if let Some(interval) = *mutex_lock(&self.observation_interval) {
            Self::start_audio_observation(context.clone(), interval)?;
        }
        let audio_hook = Arc::clone(&read_lock(&self.audio_hook));
        audio_hook(context.clone()).await?;
        self.register_telephony_callbacks(context)?;
        Ok(())
    }

    fn register_telephony_callbacks(&self, context: HandlerContext) -> Result<(), crate::Error> {
        let dtmf_context = context.clone();
        let sequence = Arc::clone(&self.dtmf_sequence);
        self.telephony.on_dtmf(Arc::new(move |mut event| {
            event.seq = sequence.fetch_add(1, Ordering::Relaxed);
            let context = dtmf_context.clone();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = catalog::VoiceEvents::new(context).dtmf(event).await;
                });
            }
        }))?;

        self.telephony.on_hangup(Arc::new(move |event| {
            let context = context.clone();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = catalog::VoiceEvents::new(context.clone())
                        .call_hangup(event)
                        .await;
                    let _ = catalog::ApplicationPeer::new(context)
                        .session_terminate(catalog::SessionTerminateRequest {
                            reason: "hangup".to_owned(),
                        })
                        .await;
                });
            }
        }))
    }

    fn start_audio_observation(
        context: HandlerContext,
        interval: Duration,
    ) -> Result<(), crate::Error> {
        let audio = context.audio().ok_or(crate::Error::AudioUnavailable)?;
        let counters = Arc::new(AudioCounters::default());
        let read = Arc::clone(&counters);
        let write = Arc::clone(&counters);
        audio.observe(AudioObserver {
            on_read: Arc::new(move |count| {
                read.read.fetch_add(count_i64(count), Ordering::Relaxed);
            }),
            on_write: Arc::new(move |count| {
                write.write.fetch_add(count_i64(count), Ordering::Relaxed);
            }),
        });
        tokio::spawn(observe_audio(context, counters, interval));
        Ok(())
    }
}

#[derive(Default)]
struct AudioCounters {
    read: AtomicI64,
    read_total: AtomicI64,
    write: AtomicI64,
    write_total: AtomicI64,
}

#[allow(clippy::cast_precision_loss)]
async fn observe_audio(context: HandlerContext, counters: Arc<AudioCounters>, interval: Duration) {
    let mut timer = tokio::time::interval(interval);
    timer.tick().await;
    loop {
        timer.tick().await;
        if matches!(
            context.state(),
            Some(SessionState::Closed | SessionState::Failed) | None
        ) {
            return;
        }
        let read = counters.read.swap(0, Ordering::Relaxed);
        let write = counters.write.swap(0, Ordering::Relaxed);
        let read_total = counters.read_total.fetch_add(read, Ordering::Relaxed) + read;
        let write_total = counters.write_total.fetch_add(write, Ordering::Relaxed) + write;
        let seconds = interval.as_secs_f64();
        let event = catalog::AudioInfoEvent {
            read: catalog::AudioInfoItem {
                bytes: read,
                bytes_per_second: read as f64 / seconds,
                bytes_total: read_total,
            },
            write: catalog::AudioInfoItem {
                bytes: write,
                bytes_per_second: write as f64 / seconds,
                bytes_total: write_total,
            },
        };
        if catalog::VoiceEvents::new(context.clone())
            .audio_info(event)
            .await
            .is_err()
        {
            return;
        }
    }
}

#[async_trait]
impl catalog::VoiceHandler for VoiceBridge {
    async fn application_move(
        &self,
        _context: HandlerContext,
        request: catalog::ApplicationMoveRequest,
    ) -> Result<catalog::ApplicationMoveResponse, crate::Error> {
        self.telephony.application_move(request).await
    }

    async fn audio_buffer_clear(
        &self,
        context: HandlerContext,
        _request: catalog::AudioBufferClearRequest,
    ) -> Result<catalog::AudioBufferClearResponse, crate::Error> {
        let cleared = context
            .audio()
            .ok_or(crate::Error::AudioUnavailable)?
            .clear_read_buffer();
        Ok(catalog::AudioBufferClearResponse {
            len: i64::try_from(cleared).unwrap_or(i64::MAX),
        })
    }

    async fn call_hangup(
        &self,
        _context: HandlerContext,
        request: catalog::CallHangupRequest,
    ) -> Result<catalog::EmptyResponse, crate::Error> {
        self.telephony.hangup(request).await?;
        Ok(empty_response())
    }

    async fn ping(
        &self,
        context: HandlerContext,
        request: catalog::PingRequest,
    ) -> Result<catalog::PingResponse, crate::Error> {
        ping_response(&context, request)
    }

    async fn recording_start(
        &self,
        _context: HandlerContext,
        request: catalog::RecordingStartRequest,
    ) -> Result<catalog::RecordingStartResponse, crate::Error> {
        self.telephony.recording_start(request).await
    }

    async fn recording_stop(
        &self,
        _context: HandlerContext,
        request: catalog::RecordingStopRequest,
    ) -> Result<catalog::EmptyResponse, crate::Error> {
        self.telephony.recording_stop(request.id).await?;
        Ok(empty_response())
    }

    async fn session_get(
        &self,
        _context: HandlerContext,
        request: catalog::SessionGetRequest,
    ) -> Result<catalog::SessionGetResponse, crate::Error> {
        Ok(catalog::SessionGetResponse(
            self.telephony.session_variables_get(request).await?,
        ))
    }

    async fn session_set(
        &self,
        _context: HandlerContext,
        request: catalog::SessionSetRequest,
    ) -> Result<catalog::EmptyResponse, crate::Error> {
        self.telephony.session_variables_set(request).await?;
        Ok(empty_response())
    }
}

/// Default `L16/8000/1`, 20 ms media format.
#[must_use]
pub fn default_media_format() -> MediaFormat {
    MediaFormat {
        encoding: "L16".to_owned(),
        sample_rate: 8_000,
        bit_depth: 16,
        channels: 1,
        ptime: DEFAULT_PTIME,
    }
}

/// Convert a generated codec selection to the runtime media representation.
///
/// # Errors
///
/// Returns missing-codec, numeric-range, or unsupported-format failures.
pub fn media_format(
    codec: Option<&catalog::AudioCodec>,
    ptime: Duration,
) -> Result<MediaFormat, crate::Error> {
    let codec = codec
        .ok_or_else(|| crate::Error::InvalidMediaFormat("audio codec is required".to_owned()))?;
    let format = MediaFormat {
        encoding: codec.name.clone(),
        sample_rate: u32::try_from(codec.sample_rate).map_err(configuration_error)?,
        bit_depth: u16::try_from(codec.bit_depth).map_err(configuration_error)?,
        channels: u16::try_from(codec.channels).map_err(configuration_error)?,
        ptime: if ptime.is_zero() {
            DEFAULT_PTIME
        } else {
            ptime
        },
    };
    format.frame_bytes()?;
    Ok(format)
}

/// Create a catalog measurement ping using the current epoch time.
///
/// # Errors
///
/// Returns a system-clock range failure.
pub fn new_ping_request() -> Result<catalog::PingRequest, crate::Error> {
    Ok(catalog::PingRequest {
        t0: epoch_millis(SystemTime::now())?,
        rtt: None,
        data: None,
    })
}

/// Measure a catalog-level ping. This is distinct from WebSocket keepalive.
///
/// # Errors
///
/// Returns clock, validation, timeout, remote, or transport failures.
pub async fn ping(context: HandlerContext, last_rtt: Option<i64>) -> Result<i64, crate::Error> {
    let mut request = new_ping_request()?;
    request.rtt = last_rtt;
    let t0 = request.t0;
    tokio::time::timeout(
        Duration::from_secs(5),
        catalog::VoicePeer::new(context).ping(request),
    )
    .await
    .map_err(|_| crate::Error::Timeout)??;
    epoch_millis(SystemTime::now())?
        .checked_sub(t0)
        .ok_or_else(|| crate::Error::SessionFailed("ping clock moved backwards".to_owned()))
}

fn ping_response(
    context: &HandlerContext,
    request: catalog::PingRequest,
) -> Result<catalog::PingResponse, crate::Error> {
    let t1 = epoch_millis(
        context
            .received_at()
            .ok_or(crate::Error::NoRequestContext)?,
    )?;
    let t2 = epoch_millis(SystemTime::now())?;
    let owd = t2
        .checked_sub(request.t0)
        .ok_or_else(|| crate::Error::SessionFailed("ping clock moved backwards".to_owned()))?;
    Ok(catalog::PingResponse {
        t0: request.t0,
        t1,
        t2,
        owd,
        data: request.data,
    })
}

fn audio_codec(format: &MediaFormat) -> catalog::AudioCodec {
    catalog::AudioCodec {
        id: format!(
            "{}/{}/{}",
            format.encoding, format.sample_rate, format.channels
        ),
        name: format.encoding.clone(),
        sample_rate: i64::from(format.sample_rate),
        bit_depth: i64::from(format.bit_depth),
        channels: i64::from(format.channels),
    }
}

fn empty_response() -> catalog::EmptyResponse {
    catalog::EmptyResponse(Map::new())
}

fn epoch_millis(time: SystemTime) -> Result<i64, crate::Error> {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .map_err(configuration_error)?
        .as_millis();
    i64::try_from(millis).map_err(configuration_error)
}

fn count_i64(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

fn configuration_error(error: impl std::fmt::Display) -> crate::Error {
    crate::Error::Configuration(error.to_string())
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
