mod agent;
mod client;
mod server;

use crate::client::{ClientArgs, client_run};
use crate::server::{ServerAgs, server_run};
use clap::Parser;

/// RTVBP demo
#[derive(Debug, clap::Parser)]
pub enum DemoArgs {
    /// Run a RTVBP client
    Client(ClientArgs),
    /// Run a RTVBP server
    Server(ServerAgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let demo = DemoArgs::parse();
    match demo {
        DemoArgs::Client(cmd) => client_run(cmd).await?,
        DemoArgs::Server(cmd) => server_run(cmd).await?,
    }

    Ok(())
}
