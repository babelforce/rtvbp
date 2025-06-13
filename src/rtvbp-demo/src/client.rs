use crate::agent::AgentCliArgs;
use codewandler_audio::{AudioPlayback, Buffer, BufferWriter, convert_pcm16_bytes_to_f32};
use openai_realtime::{ResponseCreateEvent, connect_realtime_agent};
use rtvbp_sdk::spec::v1::Message;
use rtvbp_sdk::{WebsocketConfig, websocket_connect};
use std::process::exit;
use tracing::info;
use url::Url;

#[derive(Debug, clap::Parser)]
pub struct ClientCommand {
    #[clap(short, long, default_value = "ws://127.0.0.1:8181")]
    url: Url,

    /// Authorization Bearer Token
    /// Is set as HTTP header on handshake: `Authorization: Bearer {token}`
    #[clap(short, long)]
    token: Option<String>,

    #[clap(flatten)]
    agent: Option<AgentCliArgs>,
}

pub async fn client_run(cmd: ClientCommand) -> anyhow::Result<()> {
    // playback
    let sample_rate = 24_000;
    let pb = AudioPlayback::new(sample_rate)?;
    let pb_client = pb.new_output(sample_rate);
    let pb_server = pb.new_output(sample_rate);

    let agent_settings = cmd.agent.unwrap_or_default();

    info!("agent settings: {:?}", agent_settings);

    let (my_agent, mut rx_agent) = connect_realtime_agent(agent_settings.clone().into())
        .await
        .expect("failed to connect agent");

    // init that agent
    if agent_settings.create_response {
        let a = my_agent.clone();
        tokio::spawn(async move {
            a.response_create(ResponseCreateEvent::default()).unwrap();
        });
    }

    // rtvbp client session
    let session = websocket_connect(WebsocketConfig::new(cmd.url)).await?;

    let agent_session = my_agent.clone();
    session.handle(move |ctx| {
        let agent_session = agent_session.clone();

        let playback = pb_server.clone();
        async move {
            let msg = ctx.msg();

            match msg {
                Message::Binary(data) => {
                    playback
                        .audio_write_buffer(&Buffer::new(convert_pcm16_bytes_to_f32(data.clone())))
                        .unwrap();
                    agent_session.audio_append(data.clone()).unwrap();
                }
                _ => {
                    // println!("client(rcv)> MSG {:?}", msg);
                }
            }
        }
    });

    // send agent audio to session
    tokio::spawn(async move {
        let s = session.clone();
        let pb = pb_client.clone();
        while let Some(data) = rx_agent.recv().await {
            pb.audio_write_buffer(&Buffer::new(convert_pcm16_bytes_to_f32(data.clone())))
                .unwrap();
            s.send_binary(data).unwrap()
        }
    });

    tokio::signal::ctrl_c().await?;
    exit(0);
}
