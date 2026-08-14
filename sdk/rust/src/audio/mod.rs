//! Session-owned bounded duplex byte audio and timed-frame observation.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

use crate::{MediaFormat, MediaFrame};

/// Bounded duplex byte stream used by one session audio channel.
pub struct AudioStream {
    inbound: Arc<ByteBuffer>,
    outbound: Arc<ByteBuffer>,
    format: Mutex<Option<MediaFormat>>,
    timed: Arc<FrameBuffer>,
    observers: Mutex<Vec<AudioObserver>>,
}

/// Synchronous byte-count callbacks for application reads and writes.
#[derive(Clone)]
pub struct AudioObserver {
    pub on_read: Arc<dyn Fn(usize) + Send + Sync>,
    pub on_write: Arc<dyn Fn(usize) + Send + Sync>,
}

impl AudioStream {
    /// Construct an audio stream with the same capacity for each direction.
    ///
    /// # Panics
    ///
    /// Panics when `capacity` is zero.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "audio buffer capacity must be positive");
        Self {
            inbound: Arc::new(ByteBuffer::new(capacity)),
            outbound: Arc::new(ByteBuffer::new(capacity)),
            format: Mutex::new(None),
            timed: Arc::new(FrameBuffer::new()),
            observers: Mutex::new(Vec::new()),
        }
    }

    /// Return the immutable negotiated format, when media is bound.
    #[must_use]
    pub fn format(&self) -> Option<MediaFormat> {
        self.format
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Record the negotiated format. Repeating the same selection is idempotent.
    ///
    /// # Errors
    ///
    /// Returns an invalid-format or format-conflict error.
    pub fn set_format(&self, format: MediaFormat) -> Result<(), crate::Error> {
        format.frame_bytes()?;
        let mut selected = self
            .format
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match selected.as_ref() {
            Some(existing) if existing == &format => Ok(()),
            Some(existing) => Err(crate::Error::InvalidMediaFormat(format!(
                "format is already negotiated as {existing:?}"
            ))),
            None => {
                *selected = Some(format);
                Ok(())
            }
        }
    }

    /// Read inbound bytes, waiting until at least one byte or closure.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after buffered data drains.
    pub async fn read(&self, output: &mut [u8]) -> Result<usize, crate::Error> {
        let count = self.inbound.read(output).await?;
        let callbacks: Vec<_> = self
            .observers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|observer| Arc::clone(&observer.on_read))
            .collect();
        for callback in &callbacks {
            callback(count);
        }
        Ok(count)
    }

    /// Append outbound bytes, applying bounded backpressure.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after shutdown.
    pub async fn write(&self, input: &[u8]) -> Result<usize, crate::Error> {
        let count = self.outbound.write(input).await?;
        let callbacks: Vec<_> = self
            .observers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|observer| Arc::clone(&observer.on_write))
            .collect();
        for callback in &callbacks {
            callback(count);
        }
        Ok(count)
    }

    /// Register byte-count callbacks for subsequent application I/O.
    pub fn observe(&self, observer: AudioObserver) {
        self.observers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observer);
    }

    /// Remove every currently buffered inbound byte without affecting waiters.
    #[must_use]
    pub fn clear_read_buffer(&self) -> usize {
        self.inbound.clear()
    }

    /// Read exactly one outbound packetization frame.
    ///
    /// A partial final frame is discarded when the stream closes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after closure or a format error before negotiation.
    pub async fn read_outbound_frame(&self) -> Result<Vec<u8>, crate::Error> {
        let size = self
            .format()
            .ok_or_else(|| {
                crate::Error::InvalidMediaFormat("audio format is not negotiated".to_owned())
            })?
            .frame_bytes()?;
        self.outbound.read_exact_or_drop(size).await
    }

    /// Admit one inbound timed transport frame to both byte and frame views.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after shutdown.
    pub async fn push_inbound_frame(&self, frame: MediaFrame) -> Result<(), crate::Error> {
        self.inbound.write_all(&frame.data).await?;
        self.timed.push(frame)
    }

    /// Observe the next inbound media frame with its transport PTS.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Closed`] after frame drain.
    pub async fn read_timed_frame(&self) -> Result<MediaFrame, crate::Error> {
        self.timed.pop().await
    }

    /// Close both byte directions and the timed observer idempotently.
    pub fn close(&self) {
        self.inbound.close();
        self.outbound.close();
        self.timed.close();
    }
}

struct ByteBuffer {
    capacity: usize,
    state: Mutex<ByteState>,
    readable: Notify,
    writable: Notify,
}

struct ByteState {
    bytes: VecDeque<u8>,
    closed: bool,
}

impl ByteBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            state: Mutex::new(ByteState {
                bytes: VecDeque::with_capacity(capacity),
                closed: false,
            }),
            readable: Notify::new(),
            writable: Notify::new(),
        }
    }

    async fn write(&self, input: &[u8]) -> Result<usize, crate::Error> {
        let mut written = 0;
        while written < input.len() {
            let notified = self.writable.notified();
            {
                let mut state = self.state.lock().unwrap();
                if state.closed {
                    return if written == 0 {
                        Err(crate::Error::Closed)
                    } else {
                        Ok(written)
                    };
                }
                let available = self.capacity - state.bytes.len();
                if available > 0 {
                    let count = available.min(input.len() - written);
                    state.bytes.extend(&input[written..written + count]);
                    written += count;
                    drop(state);
                    self.readable.notify_waiters();
                    continue;
                }
            }
            notified.await;
        }
        Ok(written)
    }

    async fn write_all(&self, input: &[u8]) -> Result<(), crate::Error> {
        let written = self.write(input).await?;
        if written == input.len() {
            Ok(())
        } else {
            Err(crate::Error::Closed)
        }
    }

    async fn read(&self, output: &mut [u8]) -> Result<usize, crate::Error> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let notified = self.readable.notified();
            {
                let mut state = self.state.lock().unwrap();
                if !state.bytes.is_empty() {
                    let count = output.len().min(state.bytes.len());
                    for byte in output.iter_mut().take(count) {
                        *byte = state.bytes.pop_front().unwrap();
                    }
                    drop(state);
                    self.writable.notify_waiters();
                    return Ok(count);
                }
                if state.closed {
                    return Err(crate::Error::Closed);
                }
            }
            notified.await;
        }
    }

    async fn read_exact_or_drop(&self, size: usize) -> Result<Vec<u8>, crate::Error> {
        loop {
            let notified = self.readable.notified();
            {
                let mut state = self.state.lock().unwrap();
                if state.bytes.len() >= size {
                    let frame = state.bytes.drain(..size).collect();
                    drop(state);
                    self.writable.notify_waiters();
                    return Ok(frame);
                }
                if state.closed {
                    state.bytes.clear();
                    return Err(crate::Error::Closed);
                }
            }
            notified.await;
        }
    }

    fn clear(&self) -> usize {
        let mut state = self.state.lock().unwrap();
        let cleared = state.bytes.len();
        state.bytes.clear();
        drop(state);
        self.writable.notify_waiters();
        cleared
    }

    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.readable.notify_waiters();
        self.writable.notify_waiters();
    }
}

struct FrameBuffer {
    state: Mutex<FrameState>,
    readable: Notify,
}

struct FrameState {
    frames: VecDeque<MediaFrame>,
    closed: bool,
}

impl FrameBuffer {
    fn new() -> Self {
        Self {
            state: Mutex::new(FrameState {
                frames: VecDeque::new(),
                closed: false,
            }),
            readable: Notify::new(),
        }
    }

    fn push(&self, frame: MediaFrame) -> Result<(), crate::Error> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(crate::Error::Closed);
        }
        state.frames.push_back(frame);
        drop(state);
        self.readable.notify_one();
        Ok(())
    }

    async fn pop(&self) -> Result<MediaFrame, crate::Error> {
        loop {
            let notified = self.readable.notified();
            {
                let mut state = self.state.lock().unwrap();
                if let Some(frame) = state.frames.pop_front() {
                    return Ok(frame);
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
        self.readable.notify_waiters();
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
    async fn negotiated_format_is_immutable_and_outbound_is_exactly_chunked() {
        let stream = AudioStream::new(1_024);
        stream.set_format(format()).unwrap();
        stream.set_format(format()).unwrap();
        let mut changed = format();
        changed.ptime = Duration::from_millis(10);
        assert!(stream.set_format(changed).is_err());

        let first = vec![0x11; 320];
        let second = vec![0x22; 320];
        let partial = vec![0x33; 160];
        stream
            .write(&[first.clone(), second.clone(), partial].concat())
            .await
            .unwrap();
        assert_eq!(stream.read_outbound_frame().await.unwrap(), first);
        assert_eq!(stream.read_outbound_frame().await.unwrap(), second);
        stream.close();
        assert!(matches!(
            stream.read_outbound_frame().await,
            Err(crate::Error::Closed)
        ));
    }

    #[tokio::test]
    async fn inbound_bytes_concatenate_while_timed_frames_remain_observable() {
        let stream = AudioStream::new(1_024);
        let first = MediaFrame {
            data: vec![1, 2, 3],
            pts: Some(Duration::from_millis(20)),
        };
        let second = MediaFrame {
            data: vec![4, 5],
            pts: Some(Duration::from_millis(40)),
        };
        stream.push_inbound_frame(first.clone()).await.unwrap();
        stream.push_inbound_frame(second.clone()).await.unwrap();
        let mut bytes = [0; 5];
        assert_eq!(stream.read(&mut bytes).await.unwrap(), 5);
        assert_eq!(bytes, [1, 2, 3, 4, 5]);
        assert_eq!(stream.read_timed_frame().await.unwrap(), first);
        assert_eq!(stream.read_timed_frame().await.unwrap(), second);
    }

    #[tokio::test]
    async fn clearing_does_not_poison_a_blocked_reader_and_close_unblocks_it() {
        let stream = Arc::new(AudioStream::new(32));
        let reader = {
            let stream = Arc::clone(&stream);
            tokio::spawn(async move {
                let mut output = [0; 16];
                let read = stream.read(&mut output).await?;
                Ok::<_, crate::Error>((read, output))
            })
        };
        assert_eq!(stream.clear_read_buffer(), 0);
        stream
            .push_inbound_frame(MediaFrame::untimed(b"later".to_vec()))
            .await
            .unwrap();
        let (read, output) = reader.await.unwrap().unwrap();
        assert_eq!(&output[..read], b"later");

        stream
            .push_inbound_frame(MediaFrame::untimed(b"discard".to_vec()))
            .await
            .unwrap();
        assert_eq!(stream.clear_read_buffer(), 7);
        stream.close();
        let mut output = [0; 1];
        assert!(matches!(
            stream.read(&mut output).await,
            Err(crate::Error::Closed)
        ));
    }
}
