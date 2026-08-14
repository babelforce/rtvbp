use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Notify;
use webrtc::media::Sample;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_remote::TrackRemote;

use super::{PCMU_CLOCK_RATE, codec};
use crate::{MediaChannel, MediaFormat, MediaFrame};

#[derive(Clone, Debug)]
enum Terminal {
    Orderly,
    Failed(String),
}

pub(super) struct WebRtcMedia {
    track: Arc<TrackLocalStaticSample>,
    format: OnceLock<MediaFormat>,
    incoming: FrameInbox,
    closed: AtomicBool,
}

impl WebRtcMedia {
    pub(super) fn new(track: Arc<TrackLocalStaticSample>, format: Option<MediaFormat>) -> Self {
        let selected = OnceLock::new();
        if let Some(format) = format {
            let _ = selected.set(format);
        }
        Self {
            track,
            format: selected,
            incoming: FrameInbox::new(),
            closed: AtomicBool::new(false),
        }
    }

    pub(super) fn configure(&self, format: MediaFormat) -> Result<(), crate::Error> {
        super::validate_format(&format)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(crate::Error::Closed);
        }
        match self.format.get() {
            Some(existing) if existing == &format => Ok(()),
            Some(_) => Err(crate::Error::AudioFormatConflict),
            None => self
                .format
                .set(format)
                .map_err(|_| crate::Error::AudioFormatConflict),
        }
    }

    pub(super) fn selected_format(&self) -> Option<MediaFormat> {
        self.format.get().cloned()
    }

    pub(super) fn fail(&self, message: impl Into<String>) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.incoming.close(Terminal::Failed(message.into()));
        }
    }

    pub(super) fn finish_orderly(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.incoming.close(Terminal::Orderly);
        }
    }

    pub(super) async fn receive(self: Arc<Self>, track: Arc<TrackRemote>) {
        if track.kind() != RTPCodecType::Audio {
            self.fail("unexpected non-audio WebRTC track");
            return;
        }
        let mut first_timestamp = None;
        let mut codec_checked = false;
        loop {
            match track.read_rtp().await {
                Ok((packet, _)) => {
                    if !codec_checked {
                        let capability = track.codec().capability;
                        if packet.header.payload_type != 0
                            || (!capability.mime_type.is_empty()
                                && (!capability.mime_type.eq_ignore_ascii_case("audio/PCMU")
                                    || capability.clock_rate != PCMU_CLOCK_RATE
                                    || capability.channels != 1))
                        {
                            self.fail(format!(
                                "unexpected WebRTC codec payload={} {}/{}",
                                packet.header.payload_type,
                                capability.mime_type,
                                capability.clock_rate
                            ));
                            return;
                        }
                        codec_checked = true;
                    }
                    let first = *first_timestamp.get_or_insert(packet.header.timestamp);
                    let ticks = packet.header.timestamp.wrapping_sub(first);
                    let nanos =
                        u64::from(ticks).saturating_mul(1_000_000_000) / u64::from(PCMU_CLOCK_RATE);
                    if self
                        .incoming
                        .push(MediaFrame {
                            data: codec::decode_pcmu(&packet.payload),
                            pts: Some(Duration::from_nanos(nanos)),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    if self.closed.load(Ordering::Acquire) {
                        self.finish_orderly();
                    } else {
                        self.fail(format!("read WebRTC RTP audio: {error}"));
                    }
                    return;
                }
            }
        }
    }
}

#[async_trait]
impl MediaChannel for WebRtcMedia {
    fn id(&self) -> &'static str {
        "audio"
    }

    fn format(&self) -> &MediaFormat {
        self.format
            .get()
            .expect("WebRTC media must be configured before it is exposed")
    }

    async fn write_frame(&self, frame: MediaFrame) -> Result<(), crate::Error> {
        if self.closed.load(Ordering::Acquire) {
            return Err(crate::Error::Closed);
        }
        let format = self.selected_format().ok_or_else(|| {
            crate::Error::InvalidMediaFormat("audio is not configured".to_owned())
        })?;
        let wanted = format.frame_bytes()?;
        if frame.data.len() != wanted {
            return Err(crate::Error::InvalidMediaFormat(format!(
                "L16 frame has {} bytes, want {wanted}",
                frame.data.len()
            )));
        }
        self.track
            .write_sample(&Sample {
                data: codec::encode_pcmu(&frame.data).into(),
                duration: format.ptime,
                ..Default::default()
            })
            .await
            .map_err(|error| crate::Error::Transport(error.to_string()))
    }

    async fn read_frame(&self) -> Result<MediaFrame, crate::Error> {
        self.incoming.pop().await
    }

    async fn close(&self) -> Result<(), crate::Error> {
        self.finish_orderly();
        Ok(())
    }
}

struct FrameInbox {
    state: Mutex<FrameState>,
    ready: Notify,
}

struct FrameState {
    frames: VecDeque<MediaFrame>,
    terminal: Option<Terminal>,
}

impl FrameInbox {
    fn new() -> Self {
        Self {
            state: Mutex::new(FrameState {
                frames: VecDeque::new(),
                terminal: None,
            }),
            ready: Notify::new(),
        }
    }

    fn push(&self, frame: MediaFrame) -> Result<(), crate::Error> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.terminal.is_some() {
            return Err(crate::Error::Closed);
        }
        state.frames.push_back(frame);
        drop(state);
        self.ready.notify_one();
        Ok(())
    }

    async fn pop(&self) -> Result<MediaFrame, crate::Error> {
        loop {
            let notified = self.ready.notified();
            {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(frame) = state.frames.pop_front() {
                    return Ok(frame);
                }
                if let Some(terminal) = &state.terminal {
                    return match terminal {
                        Terminal::Orderly => Err(crate::Error::Closed),
                        Terminal::Failed(message) => Err(crate::Error::Transport(message.clone())),
                    };
                }
            }
            notified.await;
        }
    }

    fn close(&self, terminal: Terminal) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.terminal.is_none() {
            state.terminal = Some(terminal);
        }
        drop(state);
        self.ready.notify_waiters();
    }
}
