use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rtvbp::catalog::babelforcev1 as catalog;
use rtvbp::envelope::v1classic;
use rtvbp::transport::{Transport, webrtcws, ws};
use rtvbp::{
    Handler, HandlerContext, MediaFormat, RequestRegistration, Session, SessionConfig, SessionState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("server") => serve().await?,
        Some("browser-server") => {
            serve_browser(
                arguments
                    .get(2)
                    .ok_or("browser-server requires websocket or webrtc")?,
            )
            .await?
        }
        Some("client") => {
            client(arguments.get(2).ok_or("client requires a WebSocket URL")?).await?
        }
        _ => return Err("usage: typescript-interop server|client [url]".into()),
    }
    Ok(())
}

async fn serve_browser(binding: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ws::ServerConfig::new("127.0.0.1:0".parse()?);
    config.transport.audio_format = Some(audio_format());
    config = match binding {
        "websocket" => config,
        "webrtc" => webrtcws::add_to_server(config),
        _ => return Err(format!("unknown browser binding {binding:?}").into()),
    };
    let server = ws::Server::bind(config).await?;
    println!("{}", server.url());
    let base = server.accept().await?;
    let envelope: Arc<dyn rtvbp::Envelope> = Arc::new(v1classic::Envelope);
    let transport: Arc<dyn Transport> = if binding == "webrtc" {
        webrtcws::accept(
            base,
            Arc::clone(&envelope),
            webrtcws::Config {
                audio_format: Some(audio_format()),
                ..Default::default()
            },
        )
        .await?
    } else {
        base
    };
    let handler = application_handler()
        .with_on_begin(|context| async move { context.open_audio(audio_format()).await });
    let session = Session::new(envelope, handler, SessionConfig::with_transport(transport));
    let running = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    wait_active(&session).await?;

    let received = tokio::spawn({
        let session = session.clone();
        async move {
            for _ in 0..100 {
                let mut frame = [0_u8; 320];
                let mut offset = 0;
                while offset < frame.len() {
                    offset += session.audio().read(&mut frame[offset..]).await?;
                }
                if frame.iter().any(|value| *value != 0) {
                    return Ok::<_, rtvbp::Error>(true);
                }
            }
            Ok::<_, rtvbp::Error>(false)
        }
    });
    let mut sequence = 0;
    for _ in 0..8 {
        session.audio().write(&tone_frame(sequence)).await?;
        sequence += 1;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    for _ in 0..32 {
        session.audio().write(&tone_frame(sequence)).await?;
        sequence += 1;
    }
    catalog::VoicePeer::new(session.clone())
        .audio_buffer_clear(catalog::AudioBufferClearRequest(Default::default()))
        .await?;
    for _ in 0..50 {
        session.audio().write(&tone_frame(sequence)).await?;
        sequence += 1;
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    catalog::ApplicationEvents::new(session.clone())
        .audio_speech_started(catalog::AudioSpeechStartedEvent {
            origin: "sender".to_owned(),
        })
        .await?;
    let microphone_non_silent = tokio::time::timeout(Duration::from_secs(10), received).await???;
    if !microphone_non_silent {
        return Err("received only silent browser microphone audio".into());
    }
    running.await??;
    server.shutdown().await?;
    Ok(())
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ws::ServerConfig::new("127.0.0.1:0".parse()?);
    config.transport.audio_format = Some(audio_format());
    let server = ws::Server::bind(config).await?;
    println!("{}", server.url());
    let transport = server.accept().await?;
    let handler = application_handler()
        .with_on_begin(|context| async move { context.open_audio(audio_format()).await });
    let session = Session::new(
        Arc::new(v1classic::Envelope),
        handler,
        SessionConfig::with_transport(transport),
    );
    let running = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    wait_active(&session).await?;
    exchange_audio(&session, 1_200, -2_400).await?;
    running.await??;
    server.shutdown().await?;
    Ok(())
}

async fn client(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ws::ClientConfig::new(url);
    config.subprotocols = Some(Vec::new());
    let transport = ws::connect(config).await?;
    let handler =
        Handler::new([], [])?.with_on_begin(|context| async move { context.accept_audio().await });
    let session = Session::new(
        Arc::new(v1classic::Envelope),
        handler,
        SessionConfig::with_transport(transport),
    );
    let running = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    wait_active(&session).await?;

    let request = rtvbp::bridge::babelforcev1::new_ping_request()?;
    let response = catalog::ApplicationPeer::new(session.clone())
        .ping(request.clone())
        .await?;
    if response.t0 != request.t0 {
        return Err("typed ping did not round-trip".into());
    }
    exchange_audio(&session, 1_200, -2_400).await?;
    catalog::ApplicationPeer::new(session.clone())
        .session_terminate(catalog::SessionTerminateRequest {
            reason: "interop complete".to_owned(),
        })
        .await?;
    running.await??;
    Ok(())
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
        Ok(catalog::EmptyResponse(Default::default()))
    });
    Handler::new([ping, terminate], []).expect("valid interop handler")
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

async fn exchange_audio(
    session: &Session,
    sent: i16,
    expected: i16,
) -> Result<(), Box<dyn std::error::Error>> {
    session.audio().write(&pcm_frame(sent)).await?;
    let mut received = [0_u8; 320];
    let mut offset = 0;
    while offset < received.len() {
        offset += session.audio().read(&mut received[offset..]).await?;
    }
    if received.iter().all(|value| *value == 0) {
        return Err("received silent audio".into());
    }
    if i16::from_le_bytes([received[0], received[1]]) != expected {
        return Err("received unexpected audio sample".into());
    }
    Ok(())
}

async fn wait_active(session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while session.state() != SessionState::Active {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
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

fn pcm_frame(sample: i16) -> Vec<u8> {
    (0..160).flat_map(|_| sample.to_le_bytes()).collect()
}

fn tone_frame(sequence: usize) -> Vec<u8> {
    (0..160)
        .flat_map(|sample| {
            let position = sequence * 160 + sample;
            let value = ((2.0 * std::f64::consts::PI * 440.0 * position as f64 / 8_000.0).sin()
                * 8_000.0)
                .round() as i16;
            value.to_le_bytes()
        })
        .collect()
}

fn millis(time: SystemTime) -> i64 {
    i64::try_from(
        time.duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_millis(),
    )
    .expect("millisecond timestamp fits i64")
}
