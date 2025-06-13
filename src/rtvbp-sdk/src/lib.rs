#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

mod handler;
mod server;
mod session;
mod websocket;

pub use {
    handler::MessageContext,
    session::Session,
    websocket::{WebsocketConfig, connect as websocket_connect, listen as websocket_listen},
};

// re-export specs
pub mod spec {
    pub use rtvbp_spec::v1;
}
