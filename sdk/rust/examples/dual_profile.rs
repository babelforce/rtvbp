use std::env;
use std::sync::Arc;
use std::time::Duration;

use rtvbp::bridge::babelforcev1::default_media_format;
use rtvbp::envelope::v1classic;
use rtvbp::transport::{webrtcws, ws};
use rtvbp::{Envelope, Error, Handler, Session, SessionConfig, SessionState, Transport};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let arguments: Vec<_> = env::args().skip(1).collect();
    match arguments.as_slice() {
        [mode, profile] if mode == "server" => serve(profile).await,
        [mode, profile, url] if mode == "client" => connect(profile, url).await,
        _ => Err(Error::Configuration(
            "usage: dual_profile server <websocket|webrtc> | client <websocket|webrtc> <url>"
                .to_owned(),
        )),
    }
}

async fn serve(profile: &str) -> Result<(), Error> {
    let mut config = ws::ServerConfig::new("127.0.0.1:0".parse().unwrap());
    config.subprotocols = Some(match profile {
        "websocket" => vec![ws::DEFAULT_SUBPROTOCOL.to_owned()],
        "webrtc" => vec![webrtcws::SUBPROTOCOL.to_owned()],
        other => return Err(unknown_profile(other)),
    });
    let server = ws::Server::bind(config).await?;
    println!("{}", server.url());
    let base = server.accept().await?;
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let transport: Arc<dyn Transport> = if profile == "webrtc" {
        webrtcws::accept(base, Arc::clone(&envelope), webrtc_config()).await?
    } else {
        base
    };
    let handler = Handler::new([], [])?
        .with_on_begin(|context| async move { context.open_audio(default_media_format()).await });
    let session = Session::new(envelope, handler, SessionConfig::with_transport(transport));
    let result = session.run().await;
    server.shutdown().await?;
    result
}

async fn connect(profile: &str, url: &str) -> Result<(), Error> {
    let mut websocket = ws::ClientConfig::new(url);
    if let Ok(authorization) = env::var("RTVBP_AUTHORIZATION") {
        websocket.authorization = Some(authorization);
    }
    let factory: Arc<dyn rtvbp::TransportFactory> = match profile {
        "websocket" => Arc::new(ws::ClientFactory::new(websocket)),
        "webrtc" => Arc::new(webrtcws::ClientFactory::new(websocket, webrtc_config())),
        other => return Err(unknown_profile(other)),
    };
    let handler =
        Handler::new([], [])?.with_on_begin(|context| async move { context.accept_audio().await });
    let session = Session::new(
        Arc::new(v1classic::Envelope),
        handler,
        SessionConfig::new(factory),
    );
    let run = tokio::spawn({
        let session = session.clone();
        async move { session.run().await }
    });
    tokio::time::timeout(Duration::from_secs(15), async {
        while session.state() != SessionState::Active {
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| Error::Timeout)?;
    session.audio().write(&vec![0x11; 320]).await?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    session.close().await?;
    run.await
        .map_err(|error| Error::SessionFailed(error.to_string()))?
}

fn webrtc_config() -> webrtcws::Config {
    let urls = env::var("RTVBP_ICE_SERVERS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ice_servers = if urls.is_empty() {
        Vec::new()
    } else {
        vec![RTCIceServer {
            urls,
            username: env::var("RTVBP_ICE_USERNAME").unwrap_or_default(),
            credential: env::var("RTVBP_ICE_CREDENTIAL").unwrap_or_default(),
        }]
    };
    webrtcws::Config {
        peer_connection: RTCConfiguration {
            ice_servers,
            ..Default::default()
        },
        audio_format: Some(default_media_format()),
        ..Default::default()
    }
}

fn unknown_profile(profile: &str) -> Error {
    Error::Configuration(format!(
        "unknown profile {profile:?}; use websocket or webrtc"
    ))
}
