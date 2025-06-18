use crate::agent::AgentArgs;
use codewandler_audio::{
    AudioPlayback, Buffer, BufferWriter, audio_capture, convert_f32_to_pcm16_bytes,
    convert_pcm16_bytes_to_f32,
};
use crossbeam_channel::{Receiver, Sender};
use fluxrpc_core::codec::json::JsonCodec;
use fluxrpc_core::{
    Event, Request, SessionContext, SessionState, TypedRpcHandler, WebsocketClientConfig,
    websocket_connect,
};
use openai_realtime::{RealtimeSession, connect_realtime_agent};
use rtvbp_spec::v1::Metadata;
use rtvbp_spec::v1::op::application::{ApplicationMoveRequest, ApplicationMoveResponse};
use rtvbp_spec::v1::op::session::{AudioConfig, AudioStreamConfig, SessionTerminateRequest, SessionUpdatedEvent};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::process::exit;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tracing::{error, info};
use url::Url;

#[derive(Debug, Clone, clap::Args)]
pub struct ClientArgs {
    /// Websocket URL to connect to
    #[clap(short, long, default_value = "ws://127.0.0.1:8181")]
    url: Url,

    /// Authorization Bearer Token which is set for websocket upgrade: `Authorization: Bearer {token}`
    #[clap(short, long)]
    token: Option<String>,

    #[clap(subcommand)]
    pub command: ClientCommand,
}

#[derive(Debug, Clone, clap::Args)]
struct ClientAudioConfig {
    #[clap(long, default_value = "false")]
    monitor: bool,

    #[clap(long, default_value = "24000")]
    input_sample_rate: u32,

    #[clap(long, default_value = "48000")]
    playback_sample_rate: u32,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ClientCommand {
    /// Uses local audio for capture and playback
    Audio(ClientAudioConfig),
    /// Use openAI to emulate a real person
    Agent(AgentArgs),
}

struct ClientState {
    pb1: Sender<f32>,
    pb2: Sender<f32>,
    rx_audio_in: Mutex<Option<Receiver<f32>>>,
    rx_data: Mutex<Option<Receiver<Vec<f32>>>>,
    tx_data: Sender<Vec<f32>>,
}

impl SessionState for ClientState {}

pub async fn client_run(client_args: ClientArgs) -> anyhow::Result<()> {
    match client_args.command.clone() {
        ClientCommand::Audio(audio_config) => {
            client_audio_run(client_args.url, client_args.token, audio_config).await?;
        }
        ClientCommand::Agent(cmd) => {
            client_agent_run(client_args, cmd).await?;
        }
    }

    exit(0);
}

async fn on_open_session_update<S>(
    ctx: Arc<dyn SessionContext<State = S>>,
    audio_config: Option<AudioConfig>,
) -> anyhow::Result<()>
where
    S: SessionState,
{
    info!("publish session.update: {:?}", audio_config);
    ctx.notify(&Event::new(
        "session.updated",
        SessionUpdatedEvent {
            audio: audio_config,
            metadata: Metadata::from([
                (
                    "call".to_string(),
                    json!({
                        "id": "call-12341234",
                        "from": "493010001000",
                        "to": "493050005000",
                        "type": "inbound"
                    }),
                ),
                ("recording_consent".to_string(), Value::from(true)),
            ])
            .into(),
        }
        .into(),
    ))
    .await?;

    Ok(())
}

async fn client_audio_run(
    url: Url,
    bearer_token: Option<String>,
    audio_config: ClientAudioConfig,
) -> anyhow::Result<()> {
    // audio setup
    let pb = AudioPlayback::new(audio_config.playback_sample_rate)?;
    let pb_mic = pb.new_output(audio_config.input_sample_rate);
    let pb_remote = pb.new_output(24_000);
    // TODO: pb_remote should be set to 8000 actually for testing, then our server would have to downsample
    // TODO: assuming the remote will send this to us might be bad!

    let cap = audio_capture(audio_config.input_sample_rate)?;
    let mic_rx = cap.subscribe();

    let mut handler = TypedRpcHandler::<ClientState>::new();

    // req: application.move
    handler.register_request_handler(
        "application.move",
        |ctx, req: ApplicationMoveRequest| async move {
            info!("req: application.move: {:?}", req);

            // emulate session.terminate as reaction to application.move
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Err(err) = ctx
                    .request(
                        &Request::new(
                            "session.terminate",
                            serde_json::to_value(SessionTerminateRequest {})
                                .unwrap()
                                .into(),
                        ),
                        Duration::from_millis(5000).into(),
                    )
                    .await
                {
                    error!("failed to terminate session: {}", err);
                }
                // TODO: close websocket connection
            });

            Result::<_, fluxrpc_core::ErrorBody>::Ok(ApplicationMoveResponse {})
        },
    );

    // from source to destination
    let audio_monitor = audio_config.monitor;
    handler.with_open_handler(move |ctx, s| async move {
        let state = ctx.state();
        let pb1 = state.pb1.clone();

        // read from mic
        if let Some(rx_mic) = state.rx_audio_in.lock().await.take() {
            let ctx_rcv = ctx.clone();

            // stream audio data to websocket
            let (tx_a, mut rx_a) = unbounded_channel::<Vec<u8>>();
            tokio::spawn(async move {
                while let Some(data) = rx_a.recv().await {
                    ctx_rcv.send_binary(data).await.unwrap();
                }
            });

            let monitor = audio_monitor.clone();

            // consume microphone
            thread::spawn(move || {
                let pb = pb1.clone();
                let mut buf = VecDeque::new();
                while let Ok(data) = rx_mic.recv() {
                    buf.push_back(data);
                    if buf.len() > 1024 {
                        let all = buf.drain(..).collect::<Vec<_>>();

                        // to playback
                        if monitor {
                            pb.audio_write_buffer(&Buffer::new(all.clone())).unwrap();
                        }

                        // to websocket
                        tx_a.send(convert_f32_to_pcm16_bytes(all)).unwrap();
                    }
                }
            });
        }

        let pb2 = state.pb2.clone();
        if let Some(rx_ws) = state.rx_data.lock().await.take() {
            thread::spawn(move || {
                while let Ok(s) = rx_ws.recv() {
                    pb2.audio_write_buffer(&Buffer::new(s)).unwrap();
                }
            });
        }
        
        on_open_session_update(ctx.clone(), AudioConfig {
            output: Some(AudioStreamConfig{
                channels: 1,
                sample_rate: audio_config.input_sample_rate,
                codec: "pcm16".into()
            }),
            input: Some(AudioStreamConfig{
                channels: 1,
                sample_rate: 24_000,
                codec: "pcm16".into()
            }),
        }.into()).await?;

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
        WebsocketClientConfig::new(url).bearer(bearer_token.unwrap_or_default()),
        JsonCodec::new(),
        Arc::new(handler),
        ClientState {
            pb1: pb_mic,
            pb2: pb_remote,
            rx_audio_in: Mutex::new(Some(mic_rx)),
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

async fn client_agent_run(client_args: ClientArgs, agent_args: AgentArgs) -> anyhow::Result<()> {
    // audio setup
    let sample_rate = 24_000;
    let pb = AudioPlayback::new(sample_rate)?;
    let pb_client = pb.new_output(sample_rate);
    let pb_server = pb.new_output(sample_rate);

    // create agent
    let (openai_agent_session, rx_agent) = connect_realtime_agent(agent_args.clone().into())
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

        on_open_session_update(ctx.clone(), None).await?;

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
        WebsocketClientConfig::new(client_args.url).bearer(client_args.token.unwrap_or_default()),
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
