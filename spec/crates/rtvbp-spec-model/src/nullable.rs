use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A present JSON field whose empty value is encoded as `null`.
///
/// Unlike `Option<T>`, this wrapper is not an optional field in generated JSON
/// Schema. Its schema carries an explicit presence marker for emitters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nullable<T>(Option<T>);

impl<T> Nullable<T> {
    #[must_use]
    pub const fn none() -> Self {
        Self(None)
    }

    #[must_use]
    pub const fn some(value: T) -> Self {
        Self(Some(value))
    }

    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.0.is_none()
    }

    #[must_use]
    pub const fn is_some(&self) -> bool {
        self.0.is_some()
    }

    #[must_use]
    pub const fn as_ref(&self) -> Nullable<&T> {
        Nullable(self.0.as_ref())
    }

    #[must_use]
    pub fn as_mut(&mut self) -> Nullable<&mut T> {
        Nullable(self.0.as_mut())
    }

    #[must_use]
    pub fn into_option(self) -> Option<T> {
        self.0
    }
}

impl<T> From<T> for Nullable<T> {
    fn from(value: T) -> Self {
        Self::some(value)
    }
}

impl<T> From<Option<T>> for Nullable<T> {
    fn from(value: Option<T>) -> Self {
        Self(value)
    }
}

impl<T> From<Nullable<T>> for Option<T> {
    fn from(value: Nullable<T>) -> Self {
        value.0
    }
}

impl<T: JsonSchema> JsonSchema for Nullable<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Nullable_{}", T::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("rtvbp::Nullable<{}>", T::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = <Option<T>>::json_schema(generator);
        schema.insert(
            "x-rtvbp-presence".to_owned(),
            Value::String("nullable".to_owned()),
        );
        schema
    }
}
