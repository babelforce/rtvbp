use crate::v1::request::RequestExt;
use crate::v1::response::ResponseExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationMoveRequest {
    Next,
    Application { id: String },
}

impl RequestExt for ApplicationMoveRequest {
    type Response = ApplicationMoveResponse;

    fn request_method_name() -> &'static str {
        "application_move"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApplicationMoveResponse;

impl ResponseExt for ApplicationMoveResponse {}

mod docs {

    use crate::v1::docs::{EXAMPLE_RESOURCE_ID, Example};
    use crate::v1::op::application::{ApplicationMoveRequest, ApplicationMoveResponse};

    impl Example for ApplicationMoveRequest {
        fn example() -> Self {
            Self::Application {
                id: EXAMPLE_RESOURCE_ID.to_string(),
            }
        }
    }

    impl Example for ApplicationMoveResponse {
        fn example() -> Self {
            Self {}
        }
    }
}
