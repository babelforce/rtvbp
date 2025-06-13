mod agent;
mod client;
mod server;

use crate::client::{ClientCommand, client_run};
use crate::server::{ServerCommand, server_run};
use clap::Parser;
// TODO: make server a feature

#[derive(Debug, clap::Parser)]
enum Demo {
    Client(ClientCommand),
    Server(ServerCommand),
    AudioTest,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let demo = Demo::parse();
    match demo {
        Demo::Client(cmd) => client_run(cmd).await?,
        Demo::Server(cmd) => server_run(cmd).await?,
        Demo::AudioTest => {} //_ => unimplemented!()
    }

    Ok(())
}
