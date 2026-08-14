use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rtvbp::bridge::babelforcev1::{
    DtmfCallback, HangupCallback, TelephonyAdapter, VoiceBridge, VoiceBridgeConfig,
    default_media_format,
};
use rtvbp::catalog::babelforcev1 as catalog;
use rtvbp::catalog::babelforcev1::{ApplicationEventHandler, ApplicationHandler};
use rtvbp::envelope::v1classic;
use rtvbp::transport::ws;
use rtvbp::{Error, Handler, HandlerContext, Session, SessionConfig, SessionState};
use serde_json::{Map, Value};
use tokio::sync::mpsc;

struct GoPeer(Child);

impl GoPeer {
    fn start(mode: &str, argument: Option<&str>, capture_stdout: bool) -> Self {
        let mut command = Command::new("go");
        command
            .args(["run", ".", mode])
            .current_dir(format!(
                "{}/tests/go-v037-interop",
                env!("CARGO_MANIFEST_DIR")
            ))
            .stderr(Stdio::inherit());
        if let Some(argument) = argument {
            command.arg(argument);
        }
        if capture_stdout {
            command.stdout(Stdio::piped());
        } else {
            command.stdout(Stdio::inherit());
        }
        Self(
            command
                .spawn()
                .expect("start published Go interoperability peer"),
        )
    }

    async fn wait(mut self) {
        let status = tokio::task::spawn_blocking(move || self.0.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success(), "published Go peer exited with {status}");
    }
}

impl Drop for GoPeer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[derive(Default)]
struct TestTelephony {
    dtmf: Mutex<Option<DtmfCallback>>,
    hangup: Mutex<Option<HangupCallback>>,
    ready: Mutex<Option<mpsc::UnboundedSender<()>>>,
}

impl TestTelephony {
    fn with_ready(ready: mpsc::UnboundedSender<()>) -> Self {
        Self {
            ready: Mutex::new(Some(ready)),
            ..Self::default()
        }
    }

    fn send_dtmf(&self, digit: &str) {
        let now = millis(SystemTime::now());
        self.dtmf.lock().unwrap().as_ref().unwrap()(catalog::DtmfEvent {
            seq: 0,
            pressed_at: now,
            released_at: now + 1,
            digit: digit.to_owned(),
        });
    }
}

#[async_trait]
impl TelephonyAdapter for TestTelephony {
    async fn application_move(
        &self,
        _: catalog::ApplicationMoveRequest,
    ) -> Result<catalog::ApplicationMoveResponse, Error> {
        Ok(catalog::ApplicationMoveResponse {
            next_application_id: None,
        })
    }

    async fn hangup(&self, _: catalog::CallHangupRequest) -> Result<(), Error> {
        Ok(())
    }

    async fn session_variables_set(&self, _: catalog::SessionSetRequest) -> Result<(), Error> {
        Ok(())
    }

    async fn session_variables_get(
        &self,
        _: catalog::SessionGetRequest,
    ) -> Result<Map<String, Value>, Error> {
        Ok(Map::new())
    }

    async fn recording_start(
        &self,
        _: catalog::RecordingStartRequest,
    ) -> Result<catalog::RecordingStartResponse, Error> {
        Ok(catalog::RecordingStartResponse {
            id: "recording".to_owned(),
        })
    }

    async fn recording_stop(&self, _: String) -> Result<(), Error> {
        Ok(())
    }

    fn on_dtmf(&self, callback: DtmfCallback) -> Result<(), Error> {
        *self.dtmf.lock().unwrap() = Some(callback);
        if let Some(ready) = self.ready.lock().unwrap().take() {
            ready.send(()).ok();
        }
        Ok(())
    }

    fn on_hangup(&self, callback: HangupCallback) -> Result<(), Error> {
        *self.hangup.lock().unwrap() = Some(callback);
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_voice_interoperates_with_published_go_application_headerless() {
    let mut go = GoPeer::start("server", None, true);
    let stdout = go.0.stdout.take().unwrap();
    let url = BufReader::new(stdout).lines().next().unwrap().unwrap();
    assert!(url.starts_with("ws://"));

    let (ready_tx, mut ready_rx) = mpsc::unbounded_channel();
    let telephony = Arc::new(TestTelephony::with_ready(ready_tx));
    let bridge = VoiceBridge::new(
        Arc::clone(&telephony) as Arc<dyn TelephonyAdapter>,
        VoiceBridgeConfig::new(
            catalog::CallInfo {
                id: "call".to_owned(),
                session_id: "session".to_owned(),
                from: "100".to_owned(),
                to: "200".to_owned(),
            },
            catalog::AppInfo {
                id: "application".to_owned(),
            },
        ),
    );
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel();
    bridge.set_audio_hook(move |context| {
        let audio_tx = audio_tx.clone();
        async move {
            let audio = context.audio().ok_or(Error::AudioUnavailable)?;
            let probe = pcm_frame(1_200);
            audio.write(&probe).await?;
            let received = read_exact(&audio, probe.len()).await?;
            if received != probe {
                return Err(Error::RequestFailed(
                    "published Go audio changed".to_owned(),
                ));
            }
            audio_tx.send(()).ok();
            Ok(())
        }
    });
    let mut websocket = ws::ClientConfig::new(url);
    websocket.subprotocols = Some(Vec::new());
    let factory = Arc::new(ws::ClientFactory::new(websocket));
    let session = Session::new(
        Arc::new(v1classic::Envelope),
        bridge.handler().unwrap(),
        SessionConfig::new(factory),
    );
    let run = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    wait_active(&session).await;
    timeout_recv(&mut ready_rx, "Rust DTMF registration").await;
    telephony.send_dtmf("5");
    timeout_recv(&mut audio_rx, "published Go audio echo").await;
    bridge.terminate("interop complete").await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    go.wait().await;
}

struct TestApplication {
    observed: mpsc::UnboundedSender<&'static str>,
}

impl TestApplication {
    fn signal(&self, name: &'static str) {
        self.observed.send(name).ok();
    }
}

#[async_trait]
impl ApplicationHandler for TestApplication {
    async fn ping(
        &self,
        context: HandlerContext,
        request: catalog::PingRequest,
    ) -> Result<catalog::PingResponse, Error> {
        self.signal("ping");
        let t1 = millis(context.received_at().ok_or(Error::NoRequestContext)?);
        let t2 = millis(SystemTime::now());
        Ok(catalog::PingResponse {
            t0: request.t0,
            t1,
            t2,
            owd: t2 - request.t0,
            data: request.data,
        })
    }

    async fn session_initialize(
        &self,
        context: HandlerContext,
        request: catalog::SessionInitializeRequest,
    ) -> Result<catalog::SessionInitializeResponse, Error> {
        let selected = request.audio_codec_offerings.first().cloned();
        context.open_audio(default_media_format()).await?;
        let audio = context.audio().ok_or(Error::AudioUnavailable)?;
        let observed = self.observed.clone();
        tokio::spawn(async move {
            let probe = read_exact(&audio, 320).await?;
            audio.write(&probe).await?;
            observed.send("audio").ok();
            Ok::<_, Error>(())
        });
        self.signal("initialize");
        Ok(catalog::SessionInitializeResponse {
            audio_codec: selected,
        })
    }

    async fn session_terminate(
        &self,
        _: HandlerContext,
        _: catalog::SessionTerminateRequest,
    ) -> Result<catalog::EmptyResponse, Error> {
        self.signal("terminate");
        Ok(catalog::EmptyResponse(Map::new()))
    }
}

#[async_trait]
impl ApplicationEventHandler for TestApplication {
    async fn audio_info(&self, _: HandlerContext, _: catalog::AudioInfoEvent) -> Result<(), Error> {
        Ok(())
    }

    async fn call_hangup(
        &self,
        _: HandlerContext,
        _: catalog::CallHangupEvent,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn dtmf(&self, _: HandlerContext, _: catalog::DtmfEvent) -> Result<(), Error> {
        self.signal("dtmf");
        Ok(())
    }

    async fn session_updated(
        &self,
        _: HandlerContext,
        _: catalog::SessionUpdatedEvent,
    ) -> Result<(), Error> {
        self.signal("updated");
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn published_go_voice_interoperates_with_rust_application_headerless() {
    let server = ws::Server::bind(ws::ServerConfig::new("127.0.0.1:0".parse().unwrap()))
        .await
        .unwrap();
    let go = GoPeer::start("client", Some(&server.url()), false);
    let transport = tokio::time::timeout(Duration::from_secs(5), server.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transport.wire_subprotocol(), "");
    assert_eq!(transport.subprotocol(), ws::DEFAULT_SUBPROTOCOL);

    let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
    let application = Arc::new(TestApplication {
        observed: observed_tx,
    });
    let handler = Handler::new(
        catalog::application_handlers(Arc::clone(&application) as Arc<dyn ApplicationHandler>),
        catalog::application_event_handlers(
            Arc::clone(&application) as Arc<dyn ApplicationEventHandler>
        ),
    )
    .unwrap();
    let session = Session::new(
        Arc::new(v1classic::Envelope),
        handler,
        SessionConfig::with_transport(transport),
    );
    let run = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    wait_active(&session).await;

    let required: HashSet<_> = [
        "initialize",
        "updated",
        "dtmf",
        "ping",
        "audio",
        "terminate",
    ]
    .into_iter()
    .collect();
    let mut observed = HashSet::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !required.is_subset(&observed) {
            observed.insert(observed_rx.recv().await.unwrap());
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    go.wait().await;
    server.shutdown().await.unwrap();
}

async fn read_exact(audio: &rtvbp::audio::AudioStream, size: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = vec![0; size];
    let mut offset = 0;
    while offset < size {
        offset += audio.read(&mut bytes[offset..]).await?;
    }
    Ok(bytes)
}

async fn timeout_recv(receiver: &mut mpsc::UnboundedReceiver<()>, name: &str) {
    tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {name}"))
        .unwrap();
}

async fn wait_active(session: &Session) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while session.state() != SessionState::Active {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

fn pcm_frame(sample: i16) -> Vec<u8> {
    (0..160).flat_map(|_| sample.to_le_bytes()).collect()
}

fn millis(time: SystemTime) -> i64 {
    i64::try_from(time.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap()
}
