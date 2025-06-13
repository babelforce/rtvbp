use chrono::Utc;
use nanoid::nanoid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Header {
    /// Message ID
    pub id: String,

    /// Timestamp
    #[serde(rename = "ts")]
    pub timestamp: i64,
}

impl Default for Header {
    fn default() -> Self {
        Header {
            id: nanoid!(),
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}

pub mod docs {
    use crate::v1::Header;
    use crate::v1::docs::Example;

    pub const EXAMPLE_TIMESTAMP: i64 = 1431648000000;
    pub const EXAMPLE_MESSAGE_ID: &str = "VPk_6IQStguK0vJrdJ4mT";

    impl Example for Header {
        fn example() -> Self {
            Header {
                id: EXAMPLE_MESSAGE_ID.to_string(),
                timestamp: EXAMPLE_TIMESTAMP,
            }
        }
    }
}
