use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PingRequest;

impl RequestExt for PingRequest {
    type Response = PingResponse;

    fn request_method_name() -> &'static str {
        "ping"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PingResponse;

impl ResponseExt for PingResponse {}

mod docs {
    use crate::v1::docs::Example;
    use crate::v1::op::ping::{PingRequest, PingResponse};

    impl Example for PingRequest {
        fn example() -> Self {
            Self {}
        }
    }

    impl Example for PingResponse {
        fn example() -> Self {
            Self {}
        }
    }
}
