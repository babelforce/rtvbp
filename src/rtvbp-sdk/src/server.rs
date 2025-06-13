use crate::handler::MessageContext;
use crate::session::{Session, SessionAcceptor};
use rtvbp_spec::v1::op::ping::PingRequest;
use rtvbp_spec::v1::op::session::SessionCapabilities;
use rtvbp_spec::v1::request::RequestExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::error;

pub struct Server {
    acceptor: Arc<dyn SessionAcceptor>,
}

impl Server {
    pub fn new(transport: Arc<dyn SessionAcceptor>) -> Arc<Self> {
        Arc::new(Self {
            acceptor: transport,
        })
    }

    pub fn run<F, HFut, H, Fut>(self: Arc<Self>, handler_factory: F) -> JoinHandle<()>
    where
        F: Fn(Arc<Session>) -> HFut + Send + Sync + 'static,
        HFut: Future<Output = H> + Send + 'static,
        H: Fn(Arc<MessageContext>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let srv = self.clone();
        let acceptor = srv.acceptor.clone();
        tokio::spawn(async move {
            // accept sessions
            while let Some(session) = acceptor.accept().await {
                let session = Arc::new(session);
                let handler = handler_factory(session.clone()).await;

                tokio::spawn(async move {
                    // initialize session
                    session
                        .session_create(SessionCapabilities::default(), None)
                        .await
                        .unwrap();

                    // ping loop
                    let session_ping = session.clone();
                    tokio::spawn(async move {
                        loop {
                            match session_ping.send(PingRequest::default().message()) {
                                Ok(_) => {}
                                Err(err) => {
                                    error!("ping failed: {}", err);
                                }
                            }

                            sleep(Duration::from_secs(10)).await;
                        }
                    });

                    match session.handle(handler).await {
                        Ok(_) => {}
                        Err(err) => {
                            error!("session failed: {}", err);
                        }
                    }
                });
            }
        })
    }
}
