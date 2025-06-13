use crate::agent::AgentCliArgs;
use openai_realtime::{AgentConfig, ResponseCreateEvent, Voice, connect_realtime_agent};
use rtvbp_sdk::spec::v1::Message;
use rtvbp_sdk::{MessageContext, websocket_listen};
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Clone, clap::Parser)]
pub struct ServerCommand {
    #[clap(short, long, default_value = "0.0.0.0:8181")]
    listen: SocketAddr,

    #[clap(flatten)]
    agent: Option<AgentCliArgs>,
}

pub async fn server_run(cmd: ServerCommand) -> anyhow::Result<()> {
    let server = websocket_listen(cmd.listen).await?;
    let mut config: AgentConfig = cmd.agent.clone().unwrap_or_default().into();
    config.voice = Voice::Ballad.into();
    info!("agent config: {:?}", config);
    server
        .run(move |session| {
            let config = config.clone();

            async move {
                // create open ai agent
                let (agent, mut rx) = connect_realtime_agent(config)
                    .await
                    .expect("failed to connect agent");

                agent
                    .response_create(ResponseCreateEvent::default())
                    .expect("failed to create initial response");

                // SEND: open-ai agent -> client websocket
                let session_audio = session.clone();
                tokio::spawn(async move {
                    while let Some(data) = rx.recv().await {
                        session_audio.send_binary(data).unwrap()
                    }
                });

                // message handler
                move |ctx: Arc<MessageContext>| {
                    let agent_handler = agent.clone();

                    async move {
                        //handle_ping(ctx.clone()).await;

                        // send audio from client to agent
                        let msg = ctx.msg();

                        match msg {
                            Message::Binary(data) => {
                                agent_handler.audio_append(data.clone()).unwrap();
                            }
                            _ => {}
                        }
                    }
                }
            }
        })
        .await?;

    tokio::signal::ctrl_c().await?;
    exit(0);
}
