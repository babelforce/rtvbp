use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rtvbp::catalog::babelforcev1 as catalog;
use rtvbp::envelope::v1classic;
use rtvbp::transport::{webrtcws, ws};
use rtvbp::{
    Envelope, Handler, HandlerContext, MediaFormat, RequestRegistration, Session, SessionConfig,
    SessionState, TransportFactory,
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

fn pcm_frame(sample: i16) -> Vec<u8> {
    (0..160).flat_map(|_| sample.to_le_bytes()).collect()
}

fn go_helper(mode: &str, argument: Option<&str>, capture_stdout: bool) -> Child {
    let mut command = Command::new("go");
    command
        .args(["run", ".", mode])
        .current_dir(format!("{}/tests/go-interop", env!("CARGO_MANIFEST_DIR")))
        .stderr(Stdio::inherit());
    if let Some(argument) = argument {
        command.arg(argument);
    }
    if capture_stdout {
        command.stdout(Stdio::piped());
    } else {
        command.stdout(Stdio::inherit());
    }
    command.spawn().expect("start Go interoperability peer")
}

fn application_handler() -> Handler {
    let ping = RequestRegistration::typed::<catalog::PingRequest, catalog::PingResponse, _, _>(
        catalog::METHOD_PING,
        false,
        |context, request| async move { ping_response(&context, request) },
    );
    let terminate = RequestRegistration::typed::<
        catalog::SessionTerminateRequest,
        catalog::EmptyResponse,
        _,
        _,
    >(catalog::METHOD_SESSION_TERMINATE, true, |_, _| async move {
        Ok(catalog::EmptyResponse(serde_json::Map::new()))
    });
    Handler::new([ping, terminate], []).unwrap()
}

fn ping_response(
    context: &HandlerContext,
    request: catalog::PingRequest,
) -> Result<catalog::PingResponse, rtvbp::Error> {
    let t1 = millis(
        context
            .received_at()
            .ok_or(rtvbp::Error::NoRequestContext)?,
    );
    let t2 = millis(SystemTime::now());
    Ok(catalog::PingResponse {
        t0: request.t0,
        t1,
        t2,
        owd: t2 - request.t0,
        data: request.data,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rust_client_interoperates_with_current_go_server() {
    let mut go = go_helper("server", None, true);
    let stdout = go.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let url = lines.next().unwrap().unwrap();
    assert!(url.starts_with("ws://"), "Go server URL: {url:?}");

    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let factory: Arc<dyn TransportFactory> = Arc::new(webrtcws::ClientFactory::new(
        ws::ClientConfig::new(url),
        webrtcws::Config::default(),
    ));
    let handler = Handler::new([], [])
        .unwrap()
        .with_on_begin(|context| async move { context.accept_audio().await });
    let client = Session::new(Arc::clone(&envelope), handler, SessionConfig::new(factory));
    let client_task = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    wait_active(&client).await;

    let request = rtvbp::bridge::babelforcev1::new_ping_request().unwrap();
    let response = catalog::ApplicationPeer::new(client.clone())
        .ping(request.clone())
        .await
        .unwrap();
    assert_eq!(response.t0, request.t0);
    client.audio().write(&pcm_frame(1_200)).await.unwrap();
    let mut received = [0_u8; 320];
    tokio::time::timeout(Duration::from_secs(5), client.audio().read(&mut received))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(received, [0; 320]);

    catalog::ApplicationPeer::new(client.clone())
        .session_terminate(catalog::SessionTerminateRequest {
            reason: "interop complete".to_owned(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), client_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let status = tokio::task::spawn_blocking(move || go.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success(), "Go server exited with {status}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn current_go_client_interoperates_with_rust_server() {
    let server = ws::Server::bind(webrtcws::add_to_server(ws::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
    )))
    .await
    .unwrap();
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let server_envelope = Arc::clone(&envelope);
    let accepted_server = Arc::clone(&server);
    let (session_tx, session_rx) = tokio::sync::oneshot::channel();
    let server_task = tokio::spawn(async move {
        let base = accepted_server.accept().await.unwrap();
        let transport = webrtcws::accept(
            base,
            Arc::clone(&server_envelope),
            webrtcws::Config::default(),
        )
        .await
        .unwrap();
        let handler = application_handler()
            .with_on_begin(|context| async move { context.open_audio(audio_format()).await });
        let session = Session::new(
            server_envelope,
            handler,
            SessionConfig::with_transport(transport),
        );
        session_tx.send(session.clone()).ok();
        session.run().await
    });

    let mut go = go_helper("client", Some(&server.url()), false);
    let session = tokio::time::timeout(Duration::from_secs(10), session_rx)
        .await
        .unwrap()
        .unwrap();
    wait_active(&session).await;
    let mut received = [0_u8; 320];
    tokio::time::timeout(Duration::from_secs(5), session.audio().read(&mut received))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(received, [0; 320]);
    session.audio().write(&pcm_frame(-2_400)).await.unwrap();

    let status = tokio::task::spawn_blocking(move || go.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success(), "Go client exited with {status}");
    tokio::time::timeout(Duration::from_secs(5), server_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    server.shutdown().await.unwrap();
}

async fn wait_active(session: &Session) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while session.state() != SessionState::Active {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

fn millis(time: SystemTime) -> i64 {
    i64::try_from(time.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap()
}
