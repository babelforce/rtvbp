mod client;
mod server;

pub use {client::connect, ezsockets::ClientConfig as WebsocketConfig, server::listen};
