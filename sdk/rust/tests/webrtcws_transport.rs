use std::sync::Arc;
use std::time::Duration;

use rtvbp::envelope::v1classic;
use rtvbp::transport::{webrtcws, ws};
use rtvbp::{
    ControlFrame, Envelope, FrameKind, Handler, MediaFormat, MediaFrame, RequestRegistration,
    Session, SessionConfig, SessionState, Transport, TransportFactory,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn reserved_signaling_leaves_control_clean_and_carries_timed_duplex_audio() {
    let server = ws::Server::bind(webrtcws::add_to_server(ws::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
    )))
    .await
    .unwrap();
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let server_envelope = Arc::clone(&envelope);
    let accepted_server = Arc::clone(&server);
    let server_task = tokio::spawn(async move {
        let base = accepted_server.accept().await.unwrap();
        let retained_base = Arc::clone(&base);
        let transport = webrtcws::accept(
            base,
            server_envelope,
            webrtcws::Config {
                audio_format: Some(audio_format()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        (transport, retained_base)
    });

    let client_factory = webrtcws::ClientFactory::new(
        ws::ClientConfig::new(server.url()),
        webrtcws::Config::default(),
    );
    let client = client_factory.connect(Arc::clone(&envelope)).await.unwrap();
    let (server_transport, server_base) = server_task.await.unwrap();
    assert_eq!(server_transport.wire_subprotocol(), webrtcws::SUBPROTOCOL);
    assert!(
        server_transport
            .remote_sdp()
            .await
            .unwrap()
            .contains("PCMU/8000")
    );

    let encoded = envelope
        .encode(&ControlFrame::request(
            "post-sdp",
            "ping",
            Some(serde_json::json!({"t0": 1})),
        ))
        .unwrap();
    client.control().send(encoded).await.unwrap();
    let received = server_transport.control().recv().await.unwrap();
    let frame = envelope.decode(&received.data).unwrap();
    assert_eq!(frame.kind, FrameKind::Request);
    assert_eq!(frame.id, "post-sdp");
    assert_eq!(frame.method, "ping");

    let (client_media, server_media) = tokio::join!(
        client.open_media("audio", audio_format()),
        server_transport.accept_media()
    );
    let client_media = client_media.unwrap();
    let server_media = server_media.unwrap();
    assert!(matches!(
        client.open_media("audio", audio_format()).await,
        Err(rtvbp::Error::MediaAlreadyOpen)
    ));
    assert!(matches!(
        client.open_media("video", audio_format()).await,
        Err(rtvbp::Error::MediaUnsupported)
    ));

    client_media
        .write_frame(MediaFrame::untimed(pcm_frame(1_000)))
        .await
        .unwrap();
    client_media
        .write_frame(MediaFrame::untimed(pcm_frame(-1_000)))
        .await
        .unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), server_media.read_frame())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(5), server_media.read_frame())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.pts, Some(Duration::ZERO));
    assert_eq!(second.pts, Some(Duration::from_millis(20)));
    assert_eq!(first.data.len(), 320);
    assert_eq!(i16::from_le_bytes([first.data[0], first.data[1]]), 988);
    assert_eq!(i16::from_le_bytes([second.data[0], second.data[1]]), -988);

    server_media
        .write_frame(MediaFrame::untimed(pcm_frame(3_000)))
        .await
        .unwrap();
    let reverse = tokio::time::timeout(Duration::from_secs(5), client_media.read_frame())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reverse.pts, Some(Duration::ZERO));
    assert_ne!(reverse.data, vec![0; 320]);

    let legacy_binary = server_base.accept_media().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), legacy_binary.read_frame())
            .await
            .is_err(),
        "WebRTC media must not traverse WebSocket binary frames"
    );

    client.close().await.unwrap();
    client.close().await.unwrap();
    server_transport.close().await.unwrap();
    server_transport.close().await.unwrap();
    server.shutdown().await.unwrap();
}

#[test]
fn adding_webrtc_preserves_classic_server_preference_and_is_idempotent() {
    let config = webrtcws::add_to_server(ws::ServerConfig::new("127.0.0.1:0".parse().unwrap()));
    assert_eq!(
        config.subprotocols.as_deref(),
        Some(
            &[
                ws::DEFAULT_SUBPROTOCOL.to_owned(),
                webrtcws::SUBPROTOCOL.to_owned()
            ][..]
        )
    );
    let config = webrtcws::add_to_server(config);
    assert_eq!(
        config
            .subprotocols
            .unwrap()
            .iter()
            .filter(|protocol| protocol.as_str() == webrtcws::SUBPROTOCOL)
            .count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn sessions_exchange_typed_control_and_non_silent_audio_over_webrtc_profile() {
    use rtvbp::catalog::babelforcev1 as catalog;

    let server = ws::Server::bind(webrtcws::add_to_server(ws::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
    )))
    .await
    .unwrap();
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let server_envelope = Arc::clone(&envelope);
    let accepted_server = Arc::clone(&server);
    let (server_session_tx, server_session_rx) = tokio::sync::oneshot::channel();
    let server_task = tokio::spawn(async move {
        let base = accepted_server.accept().await.unwrap();
        let transport = webrtcws::accept(
            base,
            Arc::clone(&server_envelope),
            webrtcws::Config::default(),
        )
        .await
        .unwrap();
        let ping = RequestRegistration::typed::<catalog::PingRequest, catalog::PingResponse, _, _>(
            catalog::METHOD_PING,
            false,
            |context, request| async move {
                let received = context
                    .received_at()
                    .unwrap()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let t1 = i64::try_from(received).unwrap();
                let t2 = i64::try_from(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                )
                .unwrap();
                Ok(catalog::PingResponse {
                    t0: request.t0,
                    t1,
                    t2,
                    owd: t2 - request.t0,
                    data: request.data,
                })
            },
        );
        let terminate = RequestRegistration::typed::<
            catalog::SessionTerminateRequest,
            catalog::EmptyResponse,
            _,
            _,
        >(catalog::METHOD_SESSION_TERMINATE, true, |_, _| async move {
            Ok(catalog::EmptyResponse(serde_json::Map::new()))
        });
        let handler = Handler::new([ping, terminate], [])
            .unwrap()
            .with_on_begin(|context| async move { context.open_audio(audio_format()).await });
        let session = Session::new(
            server_envelope,
            handler,
            SessionConfig::with_transport(transport),
        );
        server_session_tx.send(session.clone()).ok();
        session.run().await
    });

    let client_factory: Arc<dyn TransportFactory> = Arc::new(webrtcws::ClientFactory::new(
        ws::ClientConfig::new(server.url()),
        webrtcws::Config::default(),
    ));
    let client_handler = Handler::new([], [])
        .unwrap()
        .with_on_begin(|context| async move { context.accept_audio().await });
    let client = Session::new(
        Arc::clone(&envelope),
        client_handler,
        SessionConfig::new(client_factory),
    );
    let client_task = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    let server_session = tokio::time::timeout(Duration::from_secs(5), server_session_rx)
        .await
        .unwrap()
        .unwrap();
    wait_active(&client).await;
    wait_active(&server_session).await;

    let request = rtvbp::bridge::babelforcev1::new_ping_request().unwrap();
    let response = catalog::ApplicationPeer::new(client.clone())
        .ping(request.clone())
        .await
        .unwrap();
    assert_eq!(response.t0, request.t0);

    client.audio().write(&pcm_frame(1_200)).await.unwrap();
    let mut server_audio = [0_u8; 320];
    tokio::time::timeout(
        Duration::from_secs(5),
        server_session.audio().read(&mut server_audio),
    )
    .await
    .unwrap()
    .unwrap();
    assert_ne!(server_audio, [0; 320]);

    server_session
        .audio()
        .write(&pcm_frame(-2_400))
        .await
        .unwrap();
    let mut client_audio = [0_u8; 320];
    tokio::time::timeout(
        Duration::from_secs(5),
        client.audio().read(&mut client_audio),
    )
    .await
    .unwrap()
    .unwrap();
    assert_ne!(client_audio, [0; 320]);

    catalog::ApplicationPeer::new(client.clone())
        .session_terminate(catalog::SessionTerminateRequest {
            reason: "test complete".to_owned(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), client_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), server_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn negotiation_timeout_closes_the_partial_websocket() {
    let server = ws::Server::bind(webrtcws::add_to_server(ws::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
    )))
    .await
    .unwrap();
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let factory = webrtcws::ClientFactory::new(
        ws::ClientConfig::new(server.url()),
        webrtcws::Config {
            negotiation_timeout: Duration::from_millis(500),
            ..Default::default()
        },
    );
    let client = tokio::spawn({
        let envelope = Arc::clone(&envelope);
        async move { factory.connect(envelope).await }
    });
    let base = server.accept().await.unwrap();
    let offered = base.control().recv().await.unwrap();
    assert_eq!(
        envelope.decode(&offered.data).unwrap().method,
        "transport.webrtc.offer"
    );
    assert!(matches!(client.await.unwrap(), Err(rtvbp::Error::Timeout)));
    tokio::time::timeout(Duration::from_secs(2), base.wait_closed())
        .await
        .unwrap()
        .unwrap();
    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canceled_negotiation_closes_the_partial_websocket() {
    let server = ws::Server::bind(webrtcws::add_to_server(ws::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
    )))
    .await
    .unwrap();
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let factory = webrtcws::ClientFactory::new(
        ws::ClientConfig::new(server.url()),
        webrtcws::Config::default(),
    );
    let client = tokio::spawn({
        let envelope = Arc::clone(&envelope);
        async move { factory.connect(envelope).await }
    });
    let base = server.accept().await.unwrap();
    let offered = base.control().recv().await.unwrap();
    assert_eq!(
        envelope.decode(&offered.data).unwrap().method,
        "transport.webrtc.offer"
    );
    client.abort();
    assert!(matches!(client.await, Err(error) if error.is_cancelled()));
    tokio::time::timeout(Duration::from_secs(2), base.wait_closed())
        .await
        .unwrap()
        .unwrap();
    server.shutdown().await.unwrap();
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
