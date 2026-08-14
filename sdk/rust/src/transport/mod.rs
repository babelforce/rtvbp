//! Transport-neutral control and timed-media contracts.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;

pub mod memory;
pub mod webrtcws;
pub mod ws;

/// Transport-native liveness policy. The all-zero value disables monitoring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeepalivePolicy {
    pub interval: Duration,
    pub timeout: Duration,
    pub max_misses: usize,
}

impl KeepalivePolicy {
    #[must_use]
    pub fn enabled(self) -> bool {
        self != Self::default()
    }

    /// Validate an enabled policy.
    ///
    /// # Errors
    ///
    /// Returns a configuration error unless every enabled field is positive.
    pub fn validate(self) -> Result<(), crate::Error> {
        if !self.enabled() {
            return Ok(());
        }
        if self.interval.is_zero() {
            return Err(crate::Error::Configuration(
                "keepalive interval must be positive".to_owned(),
            ));
        }
        if self.timeout.is_zero() {
            return Err(crate::Error::Configuration(
                "keepalive timeout must be positive".to_owned(),
            ));
        }
        if self.max_misses == 0 {
            return Err(crate::Error::Configuration(
                "keepalive max misses must be positive".to_owned(),
            ));
        }
        Ok(())
    }
}

/// One opaque control message and the instant at which the transport received it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Received {
    pub data: Vec<u8>,
    pub received_at: SystemTime,
}

/// Fixed-width audio format carried at the SDK boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaFormat {
    pub encoding: String,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub channels: u16,
    pub ptime: Duration,
}

impl MediaFormat {
    /// Return the number of PCM bytes in one packetization interval.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidMediaFormat`] unless the format is fixed-width L16 with a
    /// positive, integral sample count and a representable byte length.
    pub fn frame_bytes(&self) -> Result<usize, crate::Error> {
        if self.encoding != "L16" {
            return Err(crate::Error::InvalidMediaFormat(format!(
                "unsupported byte-audio encoding {:?}",
                self.encoding
            )));
        }
        if self.sample_rate == 0 {
            return Err(crate::Error::InvalidMediaFormat(
                "sample rate must be positive".to_owned(),
            ));
        }
        if self.bit_depth != 16 {
            return Err(crate::Error::InvalidMediaFormat(format!(
                "L16 bit depth must be 16, got {}",
                self.bit_depth
            )));
        }
        if self.channels == 0 {
            return Err(crate::Error::InvalidMediaFormat(
                "channel count must be positive".to_owned(),
            ));
        }
        if self.ptime.is_zero() {
            return Err(crate::Error::InvalidMediaFormat(
                "packetization time must be positive".to_owned(),
            ));
        }

        let sample_nanos = u128::from(self.sample_rate)
            .checked_mul(self.ptime.as_nanos())
            .ok_or_else(|| {
                crate::Error::InvalidMediaFormat("frame sample count overflows".to_owned())
            })?;
        let second_nanos = Duration::from_secs(1).as_nanos();
        if sample_nanos % second_nanos != 0 {
            return Err(crate::Error::InvalidMediaFormat(
                "packetization time does not contain a whole number of samples".to_owned(),
            ));
        }
        let samples = sample_nanos / second_nanos;
        let bytes = samples
            .checked_mul(u128::from(self.channels))
            .and_then(|value| value.checked_mul(u128::from(self.bit_depth / 8)))
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value != 0)
            .ok_or_else(|| {
                crate::Error::InvalidMediaFormat("frame byte count is out of range".to_owned())
            })?;
        Ok(bytes)
    }
}

/// One transport media frame. `pts` is absent when the transport has no media clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaFrame {
    pub data: Vec<u8>,
    pub pts: Option<Duration>,
}

impl MediaFrame {
    #[must_use]
    pub const fn untimed(data: Vec<u8>) -> Self {
        Self { data, pts: None }
    }
}

/// Opaque envelope-byte channel.
#[async_trait]
pub trait ControlChannel: Send + Sync {
    /// Admit one complete control message.
    ///
    /// # Errors
    ///
    /// Returns cancellation or channel-closure failures.
    async fn send(&self, data: Vec<u8>) -> Result<(), crate::Error>;

    /// Receive one complete control message.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after orderly drain.
    async fn recv(&self) -> Result<Received, crate::Error>;
}

/// One named duplex media stream.
#[async_trait]
pub trait MediaChannel: Send + Sync {
    fn id(&self) -> &str;
    fn format(&self) -> &MediaFormat;

    /// Send one complete media frame.
    ///
    /// # Errors
    ///
    /// Returns a channel-closure or transport failure.
    async fn write_frame(&self, frame: MediaFrame) -> Result<(), crate::Error>;

    /// Receive one complete media frame.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after orderly drain.
    async fn read_frame(&self) -> Result<MediaFrame, crate::Error>;

    /// Close the media stream idempotently.
    ///
    /// # Errors
    ///
    /// Returns a transport close failure.
    async fn close(&self) -> Result<(), crate::Error>;
}

/// One control channel plus zero or more media channels.
#[async_trait]
pub trait Transport: Send + Sync {
    fn control(&self) -> Arc<dyn ControlChannel>;

    /// Wait for a peer-opened media stream.
    ///
    /// # Errors
    ///
    /// Returns cancellation, unsupported-media, duplicate, or transport failures.
    async fn accept_media(&self) -> Result<Arc<dyn MediaChannel>, crate::Error>;

    /// Open a named media stream.
    ///
    /// # Errors
    ///
    /// Returns cancellation, unsupported-media, duplicate, or transport failures.
    async fn open_media(
        &self,
        id: &str,
        format: MediaFormat,
    ) -> Result<Arc<dyn MediaChannel>, crate::Error>;

    /// Stop admission, drain admitted control messages, and close idempotently.
    ///
    /// # Errors
    ///
    /// Returns a transport close failure.
    async fn close(&self) -> Result<(), crate::Error>;

    /// Whether this transport supplies native liveness monitoring.
    fn supports_keepalive(&self) -> bool {
        false
    }

    /// Monitor native transport liveness until closure or failure.
    ///
    /// # Errors
    ///
    /// Returns a keepalive, transport, or configuration failure.
    async fn monitor_keepalive(&self, _policy: KeepalivePolicy) -> Result<(), crate::Error> {
        Err(crate::Error::Configuration(
            "transport does not support keepalive".to_owned(),
        ))
    }
}

/// Async constructor for one transport instance.
#[async_trait]
pub trait TransportFactory: Send + Sync {
    /// Create one transport for the selected envelope.
    ///
    /// # Errors
    ///
    /// Returns construction or cancellation failures.
    async fn connect(
        &self,
        envelope: Arc<dyn crate::Envelope>,
    ) -> Result<Arc<dyn Transport>, crate::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_width_l16_frame_sizes_match_go() {
        let cases = [
            (8_000, 1, Duration::from_millis(20), 320),
            (16_000, 1, Duration::from_millis(20), 640),
            (8_000, 2, Duration::from_millis(10), 320),
        ];
        for (sample_rate, channels, ptime, expected) in cases {
            let format = MediaFormat {
                encoding: "L16".to_owned(),
                sample_rate,
                bit_depth: 16,
                channels,
                ptime,
            };
            assert_eq!(format.frame_bytes().unwrap(), expected);
        }
    }

    #[test]
    fn invalid_fixed_width_formats_are_rejected() {
        let valid = MediaFormat {
            encoding: "L16".to_owned(),
            sample_rate: 8_000,
            bit_depth: 16,
            channels: 1,
            ptime: Duration::from_millis(20),
        };
        let invalid = vec![
            MediaFormat {
                encoding: "opus".to_owned(),
                ..valid.clone()
            },
            MediaFormat {
                sample_rate: 0,
                ..valid.clone()
            },
            MediaFormat {
                bit_depth: 8,
                ..valid.clone()
            },
            MediaFormat {
                channels: 0,
                ..valid.clone()
            },
            MediaFormat {
                ptime: Duration::ZERO,
                ..valid.clone()
            },
            MediaFormat {
                ptime: Duration::from_nanos(1),
                ..valid
            },
        ];
        assert!(
            invalid
                .into_iter()
                .all(|format| format.frame_bytes().is_err())
        );
    }
}
