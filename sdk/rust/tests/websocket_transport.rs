use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rtvbp::catalog::demov1;
use rtvbp::envelope::v1classic;
use rtvbp::transport::ws::{
    self, AuthRejection, ClientConfig, DEFAULT_SUBPROTOCOL, ServerConfig, TransportConfig,
};
use rtvbp::{
    Error, Handler, HandlerContext, KeepalivePolicy, MediaFormat, MediaFrame, Session,
    SessionConfig, SessionState, Transport,
};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const DEMO_SUBPROTOCOL: &str = "rtvbp.demo.v1";

struct DemoApplication;

#[async_trait]
impl demov1::ApplicationHandler for DemoApplication {
    async fn demo_echo(
        &self,
        context: HandlerContext,
        request: demov1::DemoEchoRequest,
    ) -> Result<demov1::DemoEchoResponse, Error> {
        demov1::ApplicationEvents::new(context)
            .demo_observed(demov1::DemoObservedEvent {
                message: request.message.clone(),
            })
            .await?;
        Ok(demov1::DemoEchoResponse {
            message: request.message,
        })
    }
}

struct DemoEvents(mpsc::UnboundedSender<String>);

#[async_trait]
impl demov1::VoiceEventHandler for DemoEvents {
    async fn demo_observed(
        &self,
        _: HandlerContext,
        event: demov1::DemoObservedEvent,
    ) -> Result<(), Error> {
        self.0
            .send(event.message)
            .map_err(|error| Error::RequestFailed(error.to_string()))
    }
}

fn audio_format() -> MediaFormat {
    MediaFormat {
        encoding: "L16".to_owned(),
        sample_rate: 8_000,
        bit_depth: 16,
        channels: 1,
        ptime: Duration::from_millis(20),
    }
}

async fn listener_url() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

#[tokio::test]
async fn authentication_precedes_upgrade_and_rejection_starts_no_transport() {
    let (listener, url) = listener_url().await;
    let authentication_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&authentication_calls);
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        ws::accept(stream, None, TransportConfig::default(), move |request| {
            calls.fetch_add(1, Ordering::Relaxed);
            if request.headers().get("authorization") == Some(&"Bearer good".parse().unwrap()) {
                Ok(())
            } else {
                Err(AuthRejection::unauthorized("unauthorized"))
            }
        })
        .await
    });

    let mut client = ClientConfig::new(url);
    client.authorization = Some("Bearer bad".to_owned());
    assert!(ws::connect(client).await.is_err());
    assert!(server.await.unwrap().is_err());
    assert_eq!(authentication_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn explicit_and_headerless_profiles_have_go_parity() {
    let (listener, url) = listener_url().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        ws::accept(
            stream,
            Some(vec!["other.v1".to_owned(), DEFAULT_SUBPROTOCOL.to_owned()]),
            TransportConfig {
                audio_format: Some(audio_format()),
            },
            |_| Ok(()),
        )
        .await
        .unwrap()
    });
    let client = ws::connect(ClientConfig::new(url)).await.unwrap();
    let server = server.await.unwrap();
    assert_eq!(client.wire_subprotocol(), DEFAULT_SUBPROTOCOL);
    assert_eq!(server.wire_subprotocol(), DEFAULT_SUBPROTOCOL);
    client.close().await.unwrap();

    let (listener, url) = listener_url().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        ws::accept(stream, None, TransportConfig::default(), |_| Ok(()))
            .await
            .unwrap()
    });
    let mut config = ClientConfig::new(url);
    config.subprotocols = Some(Vec::new());
    let client = ws::connect(config).await.unwrap();
    let server = server.await.unwrap();
    assert_eq!(client.wire_subprotocol(), "");
    assert_eq!(server.wire_subprotocol(), "");
    assert_eq!(client.subprotocol(), DEFAULT_SUBPROTOCOL);
    assert_eq!(server.subprotocol(), DEFAULT_SUBPROTOCOL);
    client.close().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn negotiated_second_catalog_runs_generated_typed_exchange() {
    let mut server_config = ServerConfig::new("127.0.0.1:0".parse().unwrap());
    server_config.subprotocols = Some(vec![DEMO_SUBPROTOCOL.to_owned()]);
    let server = ws::Server::bind(server_config).await.unwrap();
    let mut client_config = ClientConfig::new(server.url());
    client_config.subprotocols = Some(vec![DEMO_SUBPROTOCOL.to_owned()]);
    let client_transport = ws::connect(client_config).await.unwrap();
    let server_transport = server.accept().await.unwrap();
    assert_eq!(client_transport.subprotocol(), DEMO_SUBPROTOCOL);
    assert_eq!(server_transport.subprotocol(), DEMO_SUBPROTOCOL);

    let server_handler =
        Handler::new(demov1::application_handlers(Arc::new(DemoApplication)), []).unwrap();
    let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
    let client_handler = Handler::new(
        [],
        demov1::voice_event_handlers(Arc::new(DemoEvents(observed_tx))),
    )
    .unwrap();
    let server_session = Session::new(
        Arc::new(v1classic::Envelope),
        server_handler,
        SessionConfig::with_transport(server_transport),
    );
    let client_session = Session::new(
        Arc::new(v1classic::Envelope),
        client_handler,
        SessionConfig::with_transport(client_transport),
    );
    let server_run = tokio::spawn({
        let session = server_session.clone();
        async move { session.run().await }
    });
    let client_run = tokio::spawn({
        let session = client_session.clone();
        async move { session.run().await }
    });
    wait_active(&server_session).await;
    wait_active(&client_session).await;

    let response = demov1::ApplicationPeer::new(client_session.clone())
        .demo_echo(demov1::DemoEchoRequest {
            message: "hello".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(response.message, "hello");
    assert_eq!(observed_rx.recv().await.unwrap(), "hello");

    client_session.close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), client_run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), server_run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn text_and_binary_route_semantically_and_close_drains_admitted_control() {
    let (listener, url) = listener_url().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        ws::accept(
            stream,
            None,
            TransportConfig {
                audio_format: Some(audio_format()),
            },
            |_| Ok(()),
        )
        .await
        .unwrap()
    });
    let client = ws::connect(ClientConfig::new(url)).await.unwrap();
    let server = server.await.unwrap();

    let client_media = client.open_media("audio", audio_format()).await.unwrap();
    let server_media = server.accept_media().await.unwrap();
    client.control().send(b"first".to_vec()).await.unwrap();
    client.control().send(b"final".to_vec()).await.unwrap();
    client_media
        .write_frame(MediaFrame::untimed(vec![1, 2, 3, 4]))
        .await
        .unwrap();
    client.close().await.unwrap();

    assert_eq!(server.control().recv().await.unwrap().data, b"first");
    assert_eq!(server.control().recv().await.unwrap().data, b"final");
    assert!(matches!(
        server.control().recv().await,
        Err(rtvbp::Error::Closed)
    ));
    assert_eq!(
        server_media.read_frame().await.unwrap(),
        MediaFrame::untimed(vec![1, 2, 3, 4])
    );
    assert!(matches!(
        server_media.read_frame().await,
        Err(rtvbp::Error::Closed)
    ));
}

#[tokio::test]
async fn native_ping_pong_keepalive_survives_a_healthy_peer() {
    let (listener, url) = listener_url().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        ws::accept(stream, None, TransportConfig::default(), |_| Ok(()))
            .await
            .unwrap()
    });
    let client = ws::connect(ClientConfig::new(url)).await.unwrap();
    let server = server.await.unwrap();
    let monitored = Arc::clone(&client);
    let monitor = tokio::spawn(async move {
        monitored
            .monitor_keepalive(KeepalivePolicy {
                interval: Duration::from_millis(10),
                timeout: Duration::from_millis(100),
                max_misses: 1,
            })
            .await
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    server.close().await.unwrap();
    let result = monitor.await.unwrap();
    assert!(result.is_ok(), "keepalive result: {result:?}");
}

#[tokio::test]
async fn native_keepalive_fails_when_upgraded_peer_never_reads_ping() {
    let (listener, url) = listener_url().await;
    let silent_server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        drop(socket);
    });
    let mut config = ClientConfig::new(url);
    config.subprotocols = Some(Vec::new());
    let client = ws::connect(config).await.unwrap();
    let result = client
        .monitor_keepalive(KeepalivePolicy {
            interval: Duration::from_millis(5),
            timeout: Duration::from_millis(15),
            max_misses: 2,
        })
        .await;
    assert!(matches!(result, Err(rtvbp::Error::KeepaliveTimeout)));
    silent_server.abort();
}

#[tokio::test]
async fn server_shutdown_stops_admission_and_closes_active_transports() {
    let server = ws::Server::bind(ServerConfig::new("127.0.0.1:0".parse().unwrap()))
        .await
        .unwrap();
    let client = ws::connect(ClientConfig::new(server.url())).await.unwrap();
    let accepted = server.accept().await.unwrap();
    assert_eq!(server.active_count(), 1);
    client.control().send(b"admitted".to_vec()).await.unwrap();

    server.shutdown().await.unwrap();
    assert_eq!(accepted.control().recv().await.unwrap().data, b"admitted");
    assert!(matches!(
        accepted.control().recv().await,
        Err(rtvbp::Error::Closed)
    ));
    assert!(matches!(
        client.control().recv().await,
        Err(rtvbp::Error::Closed)
    ));
    assert_eq!(server.active_count(), 0);
    assert!(ws::connect(ClientConfig::new(server.url())).await.is_err());
    assert!(matches!(server.accept().await, Err(rtvbp::Error::Closed)));
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
