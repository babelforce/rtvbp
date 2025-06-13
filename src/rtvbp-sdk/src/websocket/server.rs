use crate::server::Server as SdkServer;
use crate::session::{Session as SdkSession, SessionAcceptor, SessionTransport};
use async_trait::async_trait;
use ezsockets::{CloseFrame, Request, Server, Socket};
use rtvbp_spec::v1::message::Message;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, unbounded_channel};
use tracing::debug;

type SessionID = u16;
type Session = ezsockets::Session<SessionID, ()>;

struct ServerHandler {
    tx_accept: Sender<SdkSession>,
}

struct ServerSession {
    id: SessionID,

    handle: Session,

    // send out messages which have been received
    tx_in: Sender<Message>,

    rx_out: Option<UnboundedReceiver<Message>>,

    tx_accept: Sender<SdkSession>,
}

struct ServerSessionTransport {
    handle: Session,
    rx_out: Mutex<UnboundedReceiver<Message>>,
}

#[async_trait]
impl SessionTransport for ServerSessionTransport {
    async fn receive(&self) -> Option<Message> {
        if let Some(x) = self.rx_out.lock().await.recv().await {
            return Some(x);
        }
        None
    }

    fn send(&self, msg: Message) -> anyhow::Result<()> {
        debug!("sending message: {:?}", msg);
        self.handle.text(serde_json::to_string(&msg)?)?;
        Ok(())
    }
}

#[async_trait]
impl ezsockets::ServerExt for ServerHandler {
    type Session = ServerSession;
    type Call = ();

    async fn on_connect(
        &mut self,
        socket: Socket,
        request: Request,
        address: SocketAddr,
    ) -> Result<Session, Option<CloseFrame>> {
        debug!("client connected {address} {:?}", request);

        let (tx_in, mut rx_in) = tokio::sync::mpsc::channel::<Message>(10);
        let (tx_out, rx_out) = unbounded_channel();

        // from websocket session to sdk session
        let out = tx_out.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx_in.recv().await {
                match out.send(msg) {
                    Ok(_) => {}
                    Err(_) => {
                        tracing::error!("failed to send message");
                    }
                }
            }
        });

        // create a new session
        let session_id = address.port();
        let session = Session::create(
            |handle| ServerSession {
                tx_in,
                id: session_id,
                handle,
                rx_out: Some(rx_out),
                tx_accept: self.tx_accept.clone(),
            },
            session_id,
            socket,
        );
        session.call(()).unwrap();
        debug!("session created {}", session.id);

        Ok(session)
    }

    async fn on_disconnect(
        &mut self,
        id: <Self::Session as ezsockets::SessionExt>::ID,
        reason: Result<Option<CloseFrame>, ezsockets::Error>,
    ) -> Result<(), ezsockets::Error> {
        debug!("server on_disconnect: {id} {reason:?}");
        Ok(())
    }

    async fn on_call(&mut self, call: Self::Call) -> Result<(), ezsockets::Error> {
        debug!("server on_call: {call:?}");
        Ok(())
    }
}

#[async_trait]
impl ezsockets::SessionExt for ServerSession {
    type ID = SessionID;
    type Call = ();

    fn id(&self) -> &Self::ID {
        &self.id
    }

    async fn on_text(&mut self, text: ezsockets::Utf8Bytes) -> Result<(), ezsockets::Error> {
        self.tx_in.send(serde_json::from_str(&text)?).await?;
        Ok(())
    }

    async fn on_binary(&mut self, _bytes: ezsockets::Bytes) -> Result<(), ezsockets::Error> {
        self.tx_in.send(Message::Binary(_bytes.to_vec())).await?;
        Ok(())
    }

    async fn on_call(&mut self, _call: Self::Call) -> Result<(), ezsockets::Error> {
        let transport = Arc::new(ServerSessionTransport {
            handle: self.handle.clone(),
            rx_out: Mutex::new(self.rx_out.take().unwrap()),
        });

        let session = SdkSession::new(transport);

        self.tx_accept.send(session).await.unwrap();

        Ok(())
    }
}

struct ServerSessionAcceptor {
    incoming_sessions: Mutex<Receiver<SdkSession>>,
}

#[async_trait]
impl SessionAcceptor for ServerSessionAcceptor {
    async fn accept(&self) -> Option<SdkSession> {
        let mut sessions = self.incoming_sessions.lock().await;
        sessions.recv().await
    }
}

/// Listen on socket
pub async fn listen(addr: SocketAddr) -> anyhow::Result<Arc<SdkServer>> {
    tracing::info!("listening on {addr}");

    let (tx_accept, rx_accept) = tokio::sync::mpsc::channel(10);

    tokio::spawn(async move {
        let (server, _) = Server::create(|_server| ServerHandler { tx_accept });
        ezsockets::tungstenite::run(server, addr).await.unwrap();
    });

    Ok(SdkServer::new(Arc::new(ServerSessionAcceptor {
        incoming_sessions: Mutex::new(rx_accept),
    })))
}
