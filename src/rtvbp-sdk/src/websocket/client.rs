use crate::session::{Session, SessionTransport};
use async_trait::async_trait;
use ezsockets::client::ClientCloseMode;
use ezsockets::{Client, ClientConfig};
use rtvbp_spec::v1::message::Message;
use rtvbp_spec::v1::op::session::SessionCapabilities;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::{Mutex, oneshot};
use tracing::debug;

struct ClientHandler {
    messages: UnboundedSender<Message>,
    on_connected: Option<oneshot::Sender<()>>,
}

#[async_trait]
impl ezsockets::ClientExt for ClientHandler {
    type Call = ();

    async fn on_text(&mut self, text: ezsockets::Utf8Bytes) -> Result<(), ezsockets::Error> {
        debug!("client: on_text: {}", text);
        let msg: Message = serde_json::from_str(&text)?;
        self.messages.send(msg)?;
        Ok(())
    }

    async fn on_binary(&mut self, bytes: ezsockets::Bytes) -> Result<(), ezsockets::Error> {
        self.messages.send(Message::Binary(bytes.to_vec()))?;
        Ok(())
    }

    async fn on_call(&mut self, _call: Self::Call) -> Result<(), ezsockets::Error> {
        Ok(())
    }

    async fn on_connect(&mut self) -> Result<(), ezsockets::Error> {
        debug!("client: on_connect");
        if let Some(on_connected) = self.on_connected.take() {
            on_connected.send(()).ok();
            debug!("client: on_connect (signal sent)");
        }
        Ok(())
    }

    async fn on_disconnect(&mut self) -> Result<ClientCloseMode, ezsockets::Error> {
        debug!("client: on_disconnect");
        Ok(ClientCloseMode::Reconnect)
    }
}

struct ClientMessageTransport {
    //ws: Client<ClientHandler>,
    tx_outgoing: UnboundedSender<Message>,
    rx_incoming: Mutex<UnboundedReceiver<Message>>,
}

impl ClientMessageTransport {
    pub fn new(ws: Client<ClientHandler>, rx_incoming: UnboundedReceiver<Message>) -> Self {
        let (tx_outgoing, mut rx_outgoing) = unbounded_channel();
        tokio::spawn(async move {
            let ws_write = ws.clone();
            while let Some(msg) = rx_outgoing.recv().await {
                match msg {
                    Message::Binary(data) => {
                        ws_write.binary(data).unwrap();
                    }
                    _ => {
                        ws_write.text(serde_json::to_string(&msg).unwrap()).unwrap();
                    }
                }
            }
        });
        Self {
            rx_incoming: Mutex::new(rx_incoming),
            tx_outgoing,
        }
    }
}

#[async_trait]
impl SessionTransport for ClientMessageTransport {
    async fn receive(&self) -> Option<Message> {
        if let Some(msg) = self.rx_incoming.lock().await.recv().await {
            return Some(msg);
        }
        None
    }

    fn send(&self, message: Message) -> anyhow::Result<()> {
        self.tx_outgoing.send(message)?;
        Ok(())
    }
}

pub async fn connect(config: ClientConfig) -> anyhow::Result<Arc<Session>> {
    debug!("websocket client connecting to {:?}", config);

    let (tx_incoming, rx_incoming) = unbounded_channel();
    let (tx_connected, rx_connected) = oneshot::channel::<()>();

    let (handle, _) = ezsockets::connect(
        move |_handle| ClientHandler {
            messages: tx_incoming.clone(),
            on_connected: Some(tx_connected),
        },
        config,
    )
    .await;

    // wait until connected
    rx_connected.await?;

    debug!("websocket client connected");

    let session = Session::new(Arc::new(ClientMessageTransport::new(handle, rx_incoming)));

    // initialize
    session
        .session_create(SessionCapabilities::default(), None)
        .await?;

    Ok(Arc::new(session))
}
