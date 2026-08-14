//! Drain-safe in-process transport pair.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use async_trait::async_trait;
use tokio::sync::Notify;

use super::{ControlChannel, MediaChannel, MediaFormat, MediaFrame, Received, Transport};

/// Configuration for an in-process pair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub media: bool,
}

/// One endpoint of an in-process transport pair.
pub struct MemoryTransport {
    side: usize,
    pair: Arc<Pair>,
    control: Arc<MemoryControl>,
}

impl MemoryTransport {
    #[must_use]
    pub fn pair(config: Config) -> (Arc<Self>, Arc<Self>) {
        let control = [Arc::new(Mailbox::new()), Arc::new(Mailbox::new())];
        let pair = Arc::new(Pair {
            state: Mutex::new(PairState {
                closed: false,
                media_opened: false,
                pending_media: [None, None],
                media: None,
            }),
            control: control.clone(),
            media_ready: [Notify::new(), Notify::new()],
            media_enabled: config.media,
        });
        let endpoint = |side| {
            Arc::new(Self {
                side,
                pair: Arc::clone(&pair),
                control: Arc::new(MemoryControl {
                    incoming: Arc::clone(&control[side]),
                    outgoing: Arc::clone(&control[1 - side]),
                }),
            })
        };
        (endpoint(0), endpoint(1))
    }
}

struct MemoryControl {
    incoming: Arc<Mailbox<Received>>,
    outgoing: Arc<Mailbox<Received>>,
}

#[async_trait]
impl ControlChannel for MemoryControl {
    async fn send(&self, data: Vec<u8>) -> Result<(), crate::Error> {
        self.outgoing.push(Received {
            data,
            received_at: SystemTime::now(),
        })
    }

    async fn recv(&self) -> Result<Received, crate::Error> {
        self.incoming.pop().await
    }
}

struct Pair {
    state: Mutex<PairState>,
    control: [Arc<Mailbox<Received>>; 2],
    media_ready: [Notify; 2],
    media_enabled: bool,
}

struct PairState {
    closed: bool,
    media_opened: bool,
    pending_media: [Option<Arc<MemoryMedia>>; 2],
    media: Option<Arc<MediaPair>>,
}

impl Pair {
    fn open_media(
        &self,
        side: usize,
        id: &str,
        format: &MediaFormat,
    ) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        if !self.media_enabled {
            return Err(crate::Error::MediaUnsupported);
        }
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(crate::Error::Closed);
        }
        if state.media_opened {
            return Err(crate::Error::MediaAlreadyOpen);
        }
        let pair = MediaPair::new(id, format);
        state.media_opened = true;
        state.pending_media[1 - side] = Some(Arc::clone(&pair.channels[1 - side]));
        state.media = Some(Arc::clone(&pair));
        self.media_ready[1 - side].notify_one();
        Ok(Arc::clone(&pair.channels[side]) as Arc<dyn MediaChannel>)
    }

    async fn accept_media(&self, side: usize) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        if !self.media_enabled {
            return Err(crate::Error::MediaUnsupported);
        }
        loop {
            let notified = self.media_ready[side].notified();
            {
                let mut state = self.state.lock().unwrap();
                if state.closed {
                    return Err(crate::Error::Closed);
                }
                if let Some(media) = state.pending_media[side].take() {
                    return Ok(media);
                }
            }
            notified.await;
        }
    }

    fn close(&self) {
        let media = {
            let mut state = self.state.lock().unwrap();
            if state.closed {
                return;
            }
            state.closed = true;
            state.media.clone()
        };
        self.control.iter().for_each(|mailbox| mailbox.close());
        self.media_ready.iter().for_each(Notify::notify_waiters);
        if let Some(media) = media {
            media.close();
        }
    }
}

#[async_trait]
impl Transport for MemoryTransport {
    fn control(&self) -> Arc<dyn ControlChannel> {
        Arc::clone(&self.control) as Arc<dyn ControlChannel>
    }

    async fn accept_media(&self) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        self.pair.accept_media(self.side).await
    }

    async fn open_media(
        &self,
        id: &str,
        format: MediaFormat,
    ) -> Result<Arc<dyn MediaChannel>, crate::Error> {
        self.pair.open_media(self.side, id, &format)
    }

    async fn close(&self) -> Result<(), crate::Error> {
        self.pair.close();
        Ok(())
    }
}

struct MediaPair {
    mailboxes: [Arc<Mailbox<MediaFrame>>; 2],
    channels: [Arc<MemoryMedia>; 2],
}

impl MediaPair {
    fn new(id: &str, format: &MediaFormat) -> Arc<Self> {
        Arc::new_cyclic(|weak| {
            let mailboxes = [Arc::new(Mailbox::new()), Arc::new(Mailbox::new())];
            let first = Arc::new(MemoryMedia {
                id: id.to_owned(),
                format: format.clone(),
                incoming: Arc::clone(&mailboxes[0]),
                outgoing: Arc::clone(&mailboxes[1]),
                pair: weak.clone(),
            });
            let second = Arc::new(MemoryMedia {
                id: id.to_owned(),
                format: format.clone(),
                incoming: Arc::clone(&mailboxes[1]),
                outgoing: Arc::clone(&mailboxes[0]),
                pair: weak.clone(),
            });
            Self {
                mailboxes,
                channels: [first, second],
            }
        })
    }

    fn close(&self) {
        self.mailboxes.iter().for_each(|mailbox| mailbox.close());
    }
}

struct MemoryMedia {
    id: String,
    format: MediaFormat,
    incoming: Arc<Mailbox<MediaFrame>>,
    outgoing: Arc<Mailbox<MediaFrame>>,
    pair: std::sync::Weak<MediaPair>,
}

#[async_trait]
impl MediaChannel for MemoryMedia {
    fn id(&self) -> &str {
        &self.id
    }

    fn format(&self) -> &MediaFormat {
        &self.format
    }

    async fn write_frame(&self, frame: MediaFrame) -> Result<(), crate::Error> {
        self.outgoing.push(frame)
    }

    async fn read_frame(&self) -> Result<MediaFrame, crate::Error> {
        self.incoming.pop().await
    }

    async fn close(&self) -> Result<(), crate::Error> {
        if let Some(pair) = self.pair.upgrade() {
            pair.close();
        }
        Ok(())
    }
}

struct Mailbox<T> {
    state: Mutex<MailboxState<T>>,
    ready: Notify,
}

struct MailboxState<T> {
    items: VecDeque<T>,
    closed: bool,
}

impl<T> Mailbox<T> {
    fn new() -> Self {
        Self {
            state: Mutex::new(MailboxState {
                items: VecDeque::new(),
                closed: false,
            }),
            ready: Notify::new(),
        }
    }

    fn push(&self, item: T) -> Result<(), crate::Error> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(crate::Error::Closed);
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
                let mut state = self.state.lock().unwrap();
                if let Some(item) = state.items.pop_front() {
                    return Ok(item);
                }
                if state.closed {
                    return Err(crate::Error::Closed);
                }
            }
            notified.await;
        }
    }

    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.ready.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn format() -> MediaFormat {
        MediaFormat {
            encoding: "L16".to_owned(),
            sample_rate: 8_000,
            bit_depth: 16,
            channels: 1,
            ptime: Duration::from_millis(20),
        }
    }

    #[tokio::test]
    async fn control_drains_admitted_messages_before_close() {
        let (left, right) = MemoryTransport::pair(Config::default());
        left.control().send(b"final".to_vec()).await.unwrap();
        left.close().await.unwrap();
        assert_eq!(right.control().recv().await.unwrap().data, b"final");
        assert!(matches!(
            right.control().recv().await,
            Err(crate::Error::Closed)
        ));
    }

    #[tokio::test]
    async fn optional_media_is_duplex_timed_and_single_open() {
        let (left, right) = MemoryTransport::pair(Config { media: true });
        let opened = left.open_media("audio", format()).await.unwrap();
        let accepted = right.accept_media().await.unwrap();
        assert_eq!(accepted.id(), "audio");
        assert_eq!(accepted.format(), &format());

        let frame = MediaFrame {
            data: vec![1, 2, 3],
            pts: Some(Duration::from_millis(40)),
        };
        opened.write_frame(frame.clone()).await.unwrap();
        assert_eq!(accepted.read_frame().await.unwrap(), frame);
        assert!(matches!(
            left.open_media("audio", format()).await,
            Err(crate::Error::MediaAlreadyOpen)
        ));
        accepted.close().await.unwrap();
        assert!(matches!(
            opened.read_frame().await,
            Err(crate::Error::Closed)
        ));
    }

    #[tokio::test]
    async fn disabled_media_and_closed_accept_fail_deterministically() {
        let (left, _) = MemoryTransport::pair(Config::default());
        assert!(matches!(
            left.accept_media().await,
            Err(crate::Error::MediaUnsupported)
        ));
        assert!(matches!(
            left.open_media("audio", format()).await,
            Err(crate::Error::MediaUnsupported)
        ));

        let (left, right) = MemoryTransport::pair(Config { media: true });
        left.close().await.unwrap();
        assert!(matches!(
            right.accept_media().await,
            Err(crate::Error::Closed)
        ));
    }
}
