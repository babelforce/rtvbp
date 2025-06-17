mod agent;
mod client;
mod server;

use crate::client::{ClientCommand, client_run};
use crate::server::{ServerCommand, server_run};
use clap::Parser;

#[derive(Debug, clap::Parser)]
pub enum DemoArgs {
    /// Run a RTVBP server
    #[clap(subcommand)]
    Client(ClientCommand),
    /// Run a RTVBP server
    Server(ServerCommand),
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
