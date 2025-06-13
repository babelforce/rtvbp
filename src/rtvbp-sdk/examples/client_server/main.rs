use ezsockets::ClientConfig;
use rtvbp_sdk::{MessageContext, Session, websocket_connect, websocket_listen};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::error;

async fn simulate_audio(c: Arc<Session>) {
    tokio::spawn(async move {
        let mut i = 0;
        loop {
            sleep(Duration::from_millis(1000)).await;
            match c.send_binary(vec![i]) {
                Ok(_) => {}
                Err(err) => {
                    error!("srv: send_binary error: {}", err);
                }
            };
            i = i + 1;
        }
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:8282".parse()?;

    // server
    let srv = websocket_listen(addr).await?;
    srv.run(async |c| {
        simulate_audio(c.clone()).await;
        async |c: Arc<MessageContext>| println!("srv: got msg {:?}", c.msg())
    });

    // client
    let client_session = websocket_connect(ClientConfig::new("ws://127.0.0.1:8282")).await?;
    simulate_audio(client_session.clone()).await;
    client_session.handle(|c| async move { println!("client: got msg {:?}", c.msg()) });

    // wait for some while
    sleep(Duration::from_secs(10)).await;

    Ok(())
}
