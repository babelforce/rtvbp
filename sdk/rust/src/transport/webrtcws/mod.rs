//! `rtvbp.webrtc.v1`: WebSocket control plus one duplex PCMU WebRTC audio stream.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Notify;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MIME_TYPE_PCMU, MediaEngine};
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;
use webrtc::rtp_transceiver::rtp_codec::{
    RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
};
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::track::track_local::TrackLocal;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

use super::ws;
use crate::{
    ControlChannel, Envelope, KeepalivePolicy, MediaChannel, MediaFormat, Transport,
    TransportFactory,
};

mod codec;
mod media;
mod signaling;

use media::WebRtcMedia;

/// WebSocket profile token for WebRTC media plus RTVBP control.
pub const SUBPROTOCOL: &str = crate::profile::PROFILE_RTVBP_WEBRTC_V1;
const PCMU_CLOCK_RATE: u32 = 8_000;
const PCMU_PTIME: Duration = Duration::from_millis(20);

/// WebRTC peer and SDK-boundary audio configuration.
#[derive(Clone)]
pub struct Config {
    pub peer_connection: RTCConfiguration,
    pub audio_format: Option<MediaFormat>,
    pub negotiation_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            peer_connection: RTCConfiguration::default(),
            audio_format: None,
            negotiation_timeout: Duration::from_secs(10),
        }
    }
}

impl Config {
    fn validate(&self) -> Result<(), crate::Error> {
        if let Some(format) = &self.audio_format {
            validate_format(format)?;
        }
        if self.negotiation_timeout.is_zero() {
            return Err(crate::Error::Configuration(
                "WebRTC negotiation timeout must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Session factory for an offering WebRTC client.
pub struct ClientFactory {
    websocket: ws::ClientConfig,
    config: Config,
}

impl ClientFactory {
    #[must_use]
    pub const fn new(websocket: ws::ClientConfig, config: Config) -> Self {
        Self { websocket, config }
    }
}

#[async_trait]
impl TransportFactory for ClientFactory {
    async fn connect(
        &self,
        envelope: Arc<dyn Envelope>,
    ) -> Result<Arc<dyn Transport>, crate::Error> {
        let mut websocket = self.websocket.clone();
        if websocket.subprotocols.is_none() {
            websocket.subprotocols = Some(vec![SUBPROTOCOL.to_owned()]);
        }
        let mut peer_config = self.config.clone();
        if peer_config.audio_format.is_none() {
            peer_config.audio_format = Some(websocket.audio_format.clone());
        }
        let base = ws::connect(websocket).await?;
        let mut construction = ConstructionGuard::new(Arc::clone(&base) as Arc<dyn Transport>);
        if base.wire_subprotocol() != SUBPROTOCOL {
            let selected = base.wire_subprotocol().to_owned();
            let _ = base.close().await;
            construction.disarm();
            return Err(crate::Error::UnsupportedSubprotocol(format!(
                "selected {selected:?}, want {SUBPROTOCOL:?}"
            )));
        }
        let transport = WebRtcTransport::new(base, peer_config).await?;
        construction.replace(Arc::clone(&transport) as Arc<dyn Transport>);
        let negotiation =
            signaling::negotiate_offer(transport.control(), envelope, transport.peer.as_ref());
        match tokio::time::timeout(self.config.negotiation_timeout, negotiation).await {
            Ok(Ok(())) => {
                construction.disarm();
                Ok(transport as Arc<dyn Transport>)
            }
            Ok(Err(error)) => {
                let _ = transport.close().await;
                construction.disarm();
                Err(error)
            }
            Err(_) => {
                let _ = transport.close().await;
                construction.disarm();
                Err(crate::Error::Timeout)
            }
        }
    }
}

/// Decorate an authenticated, upgraded server WebSocket selected as `rtvbp.webrtc.v1`.
///
/// # Errors
///
/// Returns profile, configuration, construction, timeout, or SDP negotiation failures.
pub async fn accept(
    base: Arc<ws::WsTransport>,
    envelope: Arc<dyn Envelope>,
    config: Config,
) -> Result<Arc<WebRtcTransport>, crate::Error> {
    let mut construction = ConstructionGuard::new(Arc::clone(&base) as Arc<dyn Transport>);
    if base.wire_subprotocol() != SUBPROTOCOL {
        return Err(crate::Error::UnsupportedSubprotocol(format!(
            "selected {:?}, want {SUBPROTOCOL:?}",
            base.wire_subprotocol()
        )));
    }
    let timeout = config.negotiation_timeout;
    let transport = WebRtcTransport::new(base, config).await?;
    construction.replace(Arc::clone(&transport) as Arc<dyn Transport>);
    let negotiation =
        signaling::negotiate_answer(transport.control(), envelope, transport.peer.as_ref());
    match tokio::time::timeout(timeout, negotiation).await {
        Ok(Ok(())) => {
            construction.disarm();
            Ok(transport)
        }
        Ok(Err(error)) => {
            let _ = transport.close().await;
            construction.disarm();
            Err(error)
        }
        Err(_) => {
            let _ = transport.close().await;
            construction.disarm();
            Err(crate::Error::Timeout)
        }
    }
}

/// Add the WebRTC token after any existing server profiles without removing classic fallback.
#[must_use]
pub fn add_to_server(mut config: ws::ServerConfig) -> ws::ServerConfig {
    let protocols = config
        .subprotocols
        .get_or_insert_with(|| vec![ws::DEFAULT_SUBPROTOCOL.to_owned()]);
    if !protocols.iter().any(|protocol| protocol == SUBPROTOCOL) {
        protocols.push(SUBPROTOCOL.to_owned());
    }
    config
}

/// Composite semantic transport with WebSocket control and WebRTC audio.
pub struct WebRtcTransport {
    base: Arc<ws::WsTransport>,
    peer: Arc<RTCPeerConnection>,
    media: Arc<WebRtcMedia>,
    connection: Mutex<Option<Result<(), String>>>,
    connection_changed: Notify,
    claimed: AtomicBool,
    closed: AtomicBool,
}

impl WebRtcTransport {
    async fn new(base: Arc<ws::WsTransport>, config: Config) -> Result<Arc<Self>, crate::Error> {
        config.validate()?;
        let mut media_engine = MediaEngine::default();
        media_engine
            .register_codec(
                RTCRtpCodecParameters {
                    capability: pcmu_capability(),
                    payload_type: 0,
                    ..Default::default()
                },
                RTPCodecType::Audio,
            )
            .map_err(transport_error)?;
        let registry = register_default_interceptors(Registry::new(), &mut media_engine)
            .map_err(transport_error)?;
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();
        let peer = Arc::new(
            api.new_peer_connection(config.peer_connection)
                .await
                .map_err(transport_error)?,
        );
        let track = Arc::new(TrackLocalStaticSample::new(
            pcmu_capability(),
            "audio".to_owned(),
            "rtvbp".to_owned(),
        ));
        let transceiver = peer
            .add_transceiver_from_track(
                Arc::clone(&track) as Arc<dyn TrackLocal + Send + Sync>,
                Some(RTCRtpTransceiverInit {
                    direction: RTCRtpTransceiverDirection::Sendrecv,
                    send_encodings: Vec::new(),
                }),
            )
            .await
            .map_err(transport_error)?;
        let sender = transceiver.sender().await;
        tokio::spawn(async move { while sender.read_rtcp().await.is_ok() {} });

        let media = Arc::new(WebRtcMedia::new(track, config.audio_format));
        let transport = Arc::new(Self {
            base,
            peer: Arc::clone(&peer),
            media: Arc::clone(&media),
            connection: Mutex::new(None),
            connection_changed: Notify::new(),
            claimed: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        });
        let weak = Arc::downgrade(&transport);
        peer.on_peer_connection_state_change(Box::new(move |state| {
            let weak = weak.clone();
            Box::pin(async move {
                if let Some(transport) = weak.upgrade() {
                    transport.handle_connection_state(state);
                }
            })
        }));
        peer.on_track(Box::new(move |track, _, _| {
            let media = Arc::clone(&media);
            Box::pin(async move {
                tokio::spawn(media.receive(track));
            })
        }));
        let lifetime = Arc::clone(&transport);
        tokio::spawn(async move {
            let _ = lifetime.base.wait_closed().await;
            if !lifetime.closed.swap(true, Ordering::AcqRel) {
                lifetime.media.finish_orderly();
                let _ = lifetime.peer.close().await;
            }
        });
        Ok(transport)
    }

    #[must_use]
    pub fn subprotocol(&self) -> &str {
        self.base.subprotocol()
    }

    #[must_use]
    pub fn wire_subprotocol(&self) -> &str {
        self.base.wire_subprotocol()
    }

    /// Return the negotiated remote SDP, when signaling completed.
    pub async fn remote_sdp(&self) -> Option<String> {
        self.peer
            .remote_description()
            .await
            .map(|description| description.sdp)
    }

    fn handle_connection_state(&self, state: RTCPeerConnectionState) {
        match state {
            RTCPeerConnectionState::Connected => self.set_connection(Ok(())),
            RTCPeerConnectionState::Failed => {
                self.set_connection(Err("WebRTC peer connection failed".to_owned()));
                self.media.fail("WebRTC peer connection failed");
            }
            RTCPeerConnectionState::Closed => {
                self.set_connection(Err("WebRTC peer connection closed".to_owned()));
                self.media.finish_orderly();
            }
            _ => {}
        }
    }

    fn set_connection(&self, result: Result<(), String>) {
        let mut state = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.is_none() {
            *state = Some(result);
            drop(state);
            self.connection_changed.notify_waiters();
        }
    }

    async fn wait_connected(&self) -> Result<(), crate::Error> {
        loop {
            let notified = self.connection_changed.notified();
            if let Some(result) = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
            {
                return result.map_err(crate::Error::Transport);
            }
            notified.await;
        }
    }

    fn claim(&self) -> Result<(), crate::Error> {
        self.claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| crate::Error::MediaAlreadyOpen)
    }

    fn unclaim(&self) {
        self.claimed.store(false, Ordering::Release);
    }
}

#[async_trait]
impl Transport for WebRtcTransport {
    fn control(&self) -> Arc<dyn ControlChannel> {
        self.base.control()
    }

    async fn accept_media(&self) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        self.claim()?;
        if self.media.selected_format().is_none() {
            self.unclaim();
            return Err(crate::Error::InvalidMediaFormat(
                "accepted WebRTC audio format is not configured".to_owned(),
            ));
        }
        if let Err(error) = self.wait_connected().await {
            self.unclaim();
            return Err(error);
        }
        Ok(Arc::clone(&self.media) as Arc<dyn MediaChannel>)
    }

    async fn open_media(
        &self,
        id: &str,
        format: MediaFormat,
    ) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        if id != "audio" {
            return Err(crate::Error::MediaUnsupported);
        }
        self.claim()?;
        if let Err(error) = self.media.configure(format) {
            self.unclaim();
            return Err(error);
        }
        if let Err(error) = self.wait_connected().await {
            self.unclaim();
            return Err(error);
        }
        Ok(Arc::clone(&self.media) as Arc<dyn MediaChannel>)
    }

    async fn close(&self) -> Result<(), crate::Error> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.media.finish_orderly();
        let peer = self.peer.close().await.map_err(transport_error);
        let base = self.base.close().await;
        match (peer, base) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(first), Err(second)) => Err(crate::Error::Transport(format!("{first}; {second}"))),
        }
    }

    fn supports_keepalive(&self) -> bool {
        true
    }

    async fn monitor_keepalive(&self, policy: KeepalivePolicy) -> Result<(), crate::Error> {
        self.base.monitor_keepalive(policy).await
    }
}

fn pcmu_capability() -> RTCRtpCodecCapability {
    RTCRtpCodecCapability {
        mime_type: MIME_TYPE_PCMU.to_owned(),
        clock_rate: PCMU_CLOCK_RATE,
        channels: 1,
        ..Default::default()
    }
}

fn validate_format(format: &MediaFormat) -> Result<(), crate::Error> {
    if format.encoding != "L16"
        || format.sample_rate != PCMU_CLOCK_RATE
        || format.bit_depth != 16
        || format.channels != 1
        || format.ptime != PCMU_PTIME
    {
        return Err(crate::Error::InvalidMediaFormat(format!(
            "unsupported WebRTC audio format {format:?}; want L16/8000/16-bit/mono/20ms"
        )));
    }
    format.frame_bytes().map(|_| ())
}

fn transport_error(error: impl std::fmt::Display) -> crate::Error {
    crate::Error::Transport(error.to_string())
}

struct ConstructionGuard {
    transport: Option<Arc<dyn Transport>>,
}

impl ConstructionGuard {
    fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport: Some(transport),
        }
    }

    fn replace(&mut self, transport: Arc<dyn Transport>) {
        self.transport = Some(transport);
    }

    fn disarm(&mut self) {
        self.transport = None;
    }
}

impl Drop for ConstructionGuard {
    fn drop(&mut self) {
        let Some(transport) = self.transport.take() else {
            return;
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = transport.close().await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_frozen_sdk_media_format_is_accepted() {
        let valid = MediaFormat {
            encoding: "L16".to_owned(),
            sample_rate: 8_000,
            bit_depth: 16,
            channels: 1,
            ptime: Duration::from_millis(20),
        };
        assert!(validate_format(&valid).is_ok());
        let mut changed = valid;
        changed.sample_rate = 16_000;
        assert!(validate_format(&changed).is_err());
    }
}
