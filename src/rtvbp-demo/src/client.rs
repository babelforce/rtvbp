use crate::agent::AgentCliArgs;
use codewandler_audio::{
    AudioPlayback, Buffer, BufferWriter, audio_capture, convert_f32_to_pcm16_bytes,
    convert_pcm16_bytes_to_f32,
};
use crossbeam_channel::{Receiver, Sender};
use fluxrpc_core::codec::json::JsonCodec;
use fluxrpc_core::{SessionState, TypedRpcHandler, WebsocketClientConfig, websocket_connect};
use openai_realtime::{RealtimeSession, connect_realtime_agent};
use std::collections::VecDeque;
use std::process::exit;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tracing::error;
use url::Url;

#[derive(Debug, clap::Subcommand)]
pub enum ClientCommand {
    /// Uses local audio for capture and playback
    Audio {
        /// Websocket URL to connect to
        #[clap(short, long, default_value = "ws://127.0.0.1:8181")]
        url: Url,
    },
    /// Use openAI to emulate a real person
    Agent(AgentCommand),
}

#[derive(Debug, clap::Args)]
pub struct AgentCommand {
    #[clap(short, long, default_value = "ws://127.0.0.1:8181")]
    url: Url,

    /// Authorization Bearer Token
    /// Is set as HTTP header on handshake: `Authorization: Bearer {token}`
    #[clap(short, long)]
    token: Option<String>,

    #[clap(flatten)]
    agent: Option<AgentCliArgs>,
}

struct ClientState {
    pb1: Sender<f32>,
    pb2: Sender<f32>,
    rx: Mutex<Option<Receiver<f32>>>,
    rx_data: Mutex<Option<Receiver<Vec<f32>>>>,
    tx_data: Sender<Vec<f32>>,
}

impl SessionState for ClientState {}

pub async fn client_run(cmd: ClientCommand) -> anyhow::Result<()> {
    match cmd {
        ClientCommand::Audio { url } => {
            client_audio_run(url).await?;
        }
        ClientCommand::Agent(cmd) => {
            client_agent_run(cmd).await?;
        }
    }

    exit(0);
}

async fn client_audio_run(url: Url) -> anyhow::Result<()> {
    // audio setup
    let sample_rate = 24_000;
    let pb = AudioPlayback::new(sample_rate)?;
    let pb1 = pb.new_output(sample_rate);
    let pb2 = pb.new_output(sample_rate);
    //let pb_server = pb.new_output(sample_rate);

    let cap = audio_capture(sample_rate)?;
    let mic_rx = cap.subscribe();

    let mut handler = TypedRpcHandler::<ClientState>::new();

    // from source to destination
    handler.with_open_handler(|ctx, s| async move {
        let state = ctx.state();
        let pb1 = state.pb1.clone();

        if let Some(rx_mic) = state.rx.lock().await.take() {
            let ctx_rcv = ctx.clone();

            let (tx_a, mut rx_a) = unbounded_channel::<Vec<u8>>();
            tokio::spawn(async move {
                while let Some(data) = rx_a.recv().await {
                    ctx_rcv.send_binary(data).await.unwrap();
                }
            });

            thread::spawn(move || {
                let pb = pb1.clone();
                let mut buf = VecDeque::new();
                while let Ok(data) = rx_mic.recv() {
                    buf.push_back(data);
                    if buf.len() > 1024 {
                        let all = buf.drain(..).collect::<Vec<_>>();

                        // to playback
                        pb.audio_write_buffer(&Buffer::new(all.clone())).unwrap();

                        // to websocket
                        tx_a.send(convert_f32_to_pcm16_bytes(all)).unwrap();
                    }
                }
            });

            // TODO: start another thread for playback here!
        }

        let pb2 = state.pb2.clone();
        if let Some(rx_ws) = state.rx_data.lock().await.take() {
            thread::spawn(move || {
                while let Ok(s) = rx_ws.recv() {
                    pb2.audio_write_buffer(&Buffer::new(s)).unwrap();
                }
            });
        }

        Ok(())
    });

    handler.register_data_handler(|s, data| async move {
        let pcm16_bytes = convert_pcm16_bytes_to_f32(data);
        s.state().tx_data.clone().send(pcm16_bytes)?;
        Ok(())
    });

    let (tx_data, rx_data) = crossbeam_channel::unbounded();

    // rtvbp client session
    let _ = websocket_connect(
        WebsocketClientConfig::new(url),
        JsonCodec::new(),
        Arc::new(handler),
        ClientState {
            pb1,
            pb2,
            rx: Mutex::new(Some(mic_rx)),
            tx_data,
            rx_data: Mutex::new(Some(rx_data)),
        },
    )
    .await?;

    tokio::signal::ctrl_c().await?;

    Ok(())
}

struct AgentState {
    pb_server: Sender<f32>,
    pb_client: Sender<f32>,
    agent: Arc<RealtimeSession>,
    rx: Mutex<Option<UnboundedReceiver<Vec<u8>>>>,
}

impl SessionState for AgentState {}

async fn client_agent_run(cmd: AgentCommand) -> anyhow::Result<()> {
    // audio setup
    let sample_rate = 24_000;
    let pb = AudioPlayback::new(sample_rate)?;
    let pb_client = pb.new_output(sample_rate);
    let pb_server = pb.new_output(sample_rate);

    // create agent
    let agent_settings = cmd.agent.unwrap_or_default();
    let (openai_agent_session, rx_agent) = connect_realtime_agent(agent_settings.clone().into())
        .await
        .expect("failed to connect agent");

    let mut handler = TypedRpcHandler::<AgentState>::new();

    handler.with_open_handler(|ctx, _| async move {
        let state = ctx.state();
        let pb = state.pb_client.clone();

        if let Some(mut rx) = state.rx.lock().await.take() {
            let ctx_rcv = ctx.clone();

            tokio::spawn(async move {
                while let Some(data) = rx.recv().await {
                    pb.audio_write_buffer(&Buffer::new(convert_pcm16_bytes_to_f32(data.clone())))
                        .unwrap();
                    ctx_rcv.send_binary(data).await.unwrap();
                }
            });
        }

        Ok(())
    });

    handler.register_data_handler(|s, data| {
        let audio_out = s.state().pb_server.clone();
        let agent = s.state().agent.clone();
        async move {
            if let Err(err) =
                audio_out.audio_write_buffer(&Buffer::new(convert_pcm16_bytes_to_f32(data.clone())))
            {
                error!("audio playback write error: {}", err);
            }

            if let Err(err) = agent.audio_append(data.clone()) {
                error!("agent audio append error: {}", err);
            }

            Ok(())
        }
    });

    // rtvbp client session
    let _ = websocket_connect(
        WebsocketClientConfig::new(cmd.url),
        JsonCodec::new(),
        Arc::new(handler),
        AgentState {
            pb_client: pb_client.clone(),
            pb_server: pb_server.clone(),
            agent: openai_agent_session.clone(),
            rx: Mutex::new(Some(rx_agent)),
        },
    )
    .await?;

    tokio::signal::ctrl_c().await?;

    Ok(())
}
