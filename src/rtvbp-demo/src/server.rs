use crate::agent::AgentArgs;
use fluxrpc_core::codec::json::JsonCodec;
use fluxrpc_core::{SessionState, TypedRpcHandler, websocket_listen};
use openai_realtime::{
    AgentConfig, RealtimeSession, ResponseCreateEvent, Voice, connect_realtime_agent,
};
use rtvbp_spec::v1::op::session::SessionUpdatedEvent;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, info};

#[derive(Debug, Clone, clap::Parser)]
pub struct ServerAgs {
    #[clap(short, long, default_value = "0.0.0.0:8181")]
    listen: SocketAddr,

    #[clap(flatten)]
    agent: Option<AgentArgs>,
}

struct ServerState {
    agent: Arc<RealtimeSession>,
    rx: Mutex<Option<UnboundedReceiver<Vec<u8>>>>,
}

impl SessionState for ServerState {}

impl ServerState {
    async fn create(config: AgentConfig) -> anyhow::Result<Self> {
        let (agent, rx) = connect_realtime_agent(config.clone()).await?;

        Ok(Self {
            agent,
            rx: Mutex::new(Some(rx)),
        })
    }
}

pub async fn server_run(cmd: ServerAgs) -> anyhow::Result<()> {
    let mut handler = TypedRpcHandler::<ServerState>::new();

    // when audio data is being received, send to agent
    handler.register_data_handler(|s, data| async move {
        s.state().agent.audio_append(data)?;
        Ok(())
    });

    handler.register_event_handler(
        "session.updated",
        |ctx, evt: SessionUpdatedEvent| async move {
            info!("session.updated: {:?}", evt);
            Ok(())
        },
    );

    handler.with_open_handler(|ctx, ()| async move {
        let state = ctx.state();

        if let Some(mut rx) = state.rx.lock().await.take() {
            let ctx_rcv = ctx.clone();

            tokio::spawn(async move {
                while let Some(data) = rx.recv().await {
                    if let Err(err) = ctx_rcv.send_binary(data).await {
                        error!("failed to send binary data: {}", err)
                    }
                }
            });
        }

        state
            .agent
            .response_create(ResponseCreateEvent::default())?;

        Ok(())
    });

    let _ = websocket_listen(cmd.listen, JsonCodec::new(), Arc::new(handler), move || {
        let mut config: AgentConfig = cmd.agent.clone().unwrap_or_default().into();
        config.voice = Voice::Ballad.into();
        async move { Ok(ServerState::create(config).await?) }
    })
    .await?;

    tokio::signal::ctrl_c().await?;
    exit(0);
}
