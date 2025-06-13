use crate::handler::MessageContext;
use async_trait::async_trait;
use nanoid::nanoid;
use rtvbp_spec::v1::Metadata;
use rtvbp_spec::v1::message::Message;
use rtvbp_spec::v1::op::session::{SessionCapabilities, SessionCreateRequest};
use rtvbp_spec::v1::request::RequestExt;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error};

#[async_trait]
pub trait SessionAcceptor: Sync + Send {
    async fn accept(&self) -> Option<Session>;
}

#[async_trait]
pub trait SessionTransport: Sync + Send {
    async fn receive(&self) -> Option<Message>;
    fn send(&self, message: Message) -> anyhow::Result<()>;
}

pub struct Session {
    id: String,
    transport: Arc<dyn SessionTransport>,
}

impl Session {
    pub fn new(transport: Arc<dyn SessionTransport>) -> Self {
        Self {
            id: nanoid!(),
            transport,
        }
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    /// Initialize the session
    pub async fn session_create(
        &self,
        capabilities: SessionCapabilities,
        metadata: Option<Metadata>,
    ) -> anyhow::Result<()> {
        // TODO: use a request instead
        self.send(
            SessionCreateRequest {
                session_id: self.id(),
                capabilities,
                metadata,
            }
            .message(),
        )?;

        Ok(())
    }

    pub fn send_binary(&self, data: Vec<u8>) -> anyhow::Result<()> {
        self.send(Message::Binary(data))
    }

    // TODO: request response

    pub fn send(&self, msg: impl Into<Message>) -> anyhow::Result<()> {
        let msg = msg.into();
        self.transport.send(msg)
    }

    pub fn handle<H, F>(&self, handler: H) -> JoinHandle<()>
    where
        H: for<'a> Fn(Arc<MessageContext>) -> F + Send + Sync + 'static,
        F: Future<Output = ()> + Send + 'static,
    {
        let transport = self.transport.clone();

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        // from handler to transport
        let transport_send = transport.clone();
        let task_send = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match transport_send.send(message) {
                    Err(err) => {
                        error!("failed to send message: {}", err);
                    }
                    Ok(_) => {}
                }
            }
        });

        // from transport to handler
        let tx_receive = tx.clone();
        let transport_receive = transport.clone();
        let task_receive = tokio::spawn(async move {
            while let Some(message) = transport_receive.receive().await {
                handler(Arc::new(MessageContext::new(tx_receive.clone(), message))).await;
            }
        });

        debug!("session handler started: {}", self.id);

        let sid = self.id.clone();
        tokio::spawn(async move {
            match tokio::try_join!(task_send, task_receive) {
                Ok(_) => {}
                Err(err) => {
                    error!("session handler ended: {}", err);
                }
            }
            debug!("session handler ended: {}", sid)
        })
    }
}
