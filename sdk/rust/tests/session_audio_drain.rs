use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rtvbp::envelope::v1classic;
use rtvbp::transport::memory::{Config as MemoryConfig, MemoryTransport};
use rtvbp::{
    ControlChannel, Handler, MediaChannel, MediaFormat, MediaFrame, Session, SessionConfig,
    SessionState, Transport,
};

fn audio_format() -> MediaFormat {
    MediaFormat {
        encoding: "L16".to_owned(),
        sample_rate: 8_000,
        bit_depth: 16,
        channels: 1,
        ptime: Duration::from_millis(20),
    }
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

struct GatedMediaTransport {
    inner: Arc<dyn Transport>,
    gate: tokio::sync::watch::Receiver<bool>,
}

#[async_trait]
impl Transport for GatedMediaTransport {
    fn control(&self) -> Arc<dyn ControlChannel> {
        self.inner.control()
    }

    async fn accept_media(&self) -> Result<Arc<dyn MediaChannel>, rtvbp::Error> {
        let media = self.inner.accept_media().await?;
        Ok(Arc::new(GatedMediaChannel {
            inner: media,
            gate: self.gate.clone(),
        }))
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
}

struct GatedMediaChannel {
    inner: Arc<dyn MediaChannel>,
    gate: tokio::sync::watch::Receiver<bool>,
}

#[async_trait]
impl MediaChannel for GatedMediaChannel {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn format(&self) -> &MediaFormat {
        self.inner.format()
    }

    async fn write_frame(&self, frame: MediaFrame) -> Result<(), rtvbp::Error> {
        self.inner.write_frame(frame).await
    }

    async fn read_frame(&self) -> Result<MediaFrame, rtvbp::Error> {
        let mut gate = self.gate.clone();
        gate.wait_for(|open| *open).await.unwrap();
        self.inner.read_frame().await
    }

    async fn close(&self) -> Result<(), rtvbp::Error> {
        self.inner.close().await
    }
}

// The transport holds a frame when the session starts to close, because the media pump has not
// moved it yet (the gate keeps it there). The frame arrived before the close and must stay
// readable, then the stream must end.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_audio_queued_before_shutdown_is_still_delivered() {
    let (left, right) = MemoryTransport::pair(MemoryConfig { media: true });
    let (open_gate, gate) = tokio::sync::watch::channel(false);
    let peer_media = right.open_media("audio", audio_format()).await.unwrap();
    let local = Session::new(
        Arc::new(v1classic::Envelope),
        Handler::new([], [])
            .unwrap()
            .with_on_begin(|context| async move { context.accept_audio().await }),
        SessionConfig::with_transport(Arc::new(GatedMediaTransport { inner: left, gate })),
    );
    let local_task = tokio::spawn({
        let local = local.clone();
        async move { local.run().await }
    });
    wait_active(&local).await;

    let frame = vec![0x5a; 320];
    peer_media
        .write_frame(MediaFrame::untimed(frame.clone()))
        .await
        .unwrap();

    let closing = tokio::spawn({
        let local = local.clone();
        async move { local.close().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while local.audio().write(&[0]).await.is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the session never refused audio writes after close");
    open_gate.send(true).unwrap();

    let mut received = vec![0; 320];
    let mut offset = 0;
    while offset < received.len() {
        offset += tokio::time::timeout(
            Duration::from_secs(2),
            local.audio().read(&mut received[offset..]),
        )
        .await
        .expect("reading the queued frame timed out")
        .expect("the frame queued before close was dropped");
    }
    assert_eq!(received, frame);
    let mut next = [0; 1];
    assert!(matches!(
        local.audio().read(&mut next).await,
        Err(rtvbp::Error::Closed)
    ));
    closing.await.unwrap().unwrap();
    local_task.await.unwrap().unwrap();
}
