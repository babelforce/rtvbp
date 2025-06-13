use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub trait ApiSupport: Sync + Send + Serialize + DeserializeOwned + JsonSchema + Sized {}

impl<T> ApiSupport for T where T: Sync + Send + Serialize + DeserializeOwned + JsonSchema + Sized {}
