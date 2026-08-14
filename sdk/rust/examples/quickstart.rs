use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rtvbp::bridge::babelforcev1::{DEFAULT_PTIME, default_media_format, media_format};
use rtvbp::catalog::babelforcev1 as catalog;
use rtvbp::catalog::babelforcev1::ApplicationHandler;
use rtvbp::envelope::v1classic;
use rtvbp::transport::{webrtcws, ws};
use rtvbp::{Envelope, Error, Handler, HandlerContext, Session, SessionConfig, Transport};

struct Application;

#[async_trait]
impl ApplicationHandler for Application {
    async fn ping(
        &self,
        context: HandlerContext,
        request: catalog::PingRequest,
    ) -> Result<catalog::PingResponse, Error> {
        let t1 = epoch_millis(context.received_at().ok_or(Error::NoRequestContext)?)?;
        let t2 = epoch_millis(SystemTime::now())?;
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
        let selected = request
            .audio_codec_offerings
            .into_iter()
            .find(|codec| {
                media_format(Some(codec), DEFAULT_PTIME)
                    .is_ok_and(|format| format == default_media_format())
            })
            .ok_or_else(|| Error::InvalidMediaFormat("L16/8000/1 was not offered".to_owned()))?;
        context.open_audio(default_media_format()).await?;
        let audio = context.audio().ok_or(Error::AudioUnavailable)?;
        tokio::spawn(async move {
            let mut buffer = [0_u8; 320];
            while audio.read(&mut buffer).await.is_ok() {}
        });
        Ok(catalog::SessionInitializeResponse {
            audio_codec: Some(selected),
        })
    }

    async fn session_terminate(
        &self,
        _: HandlerContext,
        _: catalog::SessionTerminateRequest,
    ) -> Result<catalog::EmptyResponse, Error> {
        Ok(catalog::EmptyResponse(serde_json::Map::new()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let server = ws::Server::bind(webrtcws::add_to_server(ws::ServerConfig::new(
        "0.0.0.0:8080"
            .parse()
            .map_err(|error| Error::Configuration(format!("listen address: {error}")))?,
    )))
    .await?;
    println!("listening on {}", server.url());

    // This compact example serves one session. Production code normally accepts in a loop and
    // spawns one task per returned transport.
    let base = server.accept().await?;
    let envelope: Arc<dyn Envelope> = Arc::new(v1classic::Envelope);
    let transport: Arc<dyn Transport> = if base.wire_subprotocol() == webrtcws::SUBPROTOCOL {
        webrtcws::accept(
            base,
            Arc::clone(&envelope),
            webrtcws::Config {
                audio_format: Some(default_media_format()),
                ..Default::default()
            },
        )
        .await?
    } else {
        base
    };
    let application: Arc<dyn ApplicationHandler> = Arc::new(Application);
    let session = Session::new(
        envelope,
        Handler::new(catalog::application_handlers(application), [])?,
        SessionConfig::with_transport(transport),
    );
    let result = session.run().await;
    server.shutdown().await?;
    result
}

fn epoch_millis(time: SystemTime) -> Result<i64, Error> {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::RequestFailed(error.to_string()))?
        .as_millis();
    i64::try_from(millis).map_err(|error| Error::RequestFailed(error.to_string()))
}
