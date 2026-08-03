#![forbid(unsafe_code)]

mod catalog;
mod envelope;
mod nullable;

pub use catalog::{
    Catalog, CatalogFixture, CatalogId, CatalogItemKind, CatalogValidationError,
    CatalogValidationErrors, Event, EventExample, ExampleSide, FixtureTarget, Operation,
    OperationExample, RESERVED_TRANSPORT_METHOD_PREFIX, Role, TypeRef,
};
pub use envelope::{
    CodecError, ConstantField, ControlFrame, EnvelopeSpec, ErrorCodeSpec, ErrorSpec, FieldSpec,
    FrameKind, FrameSpec, WireError, classic_v1,
};
pub use nullable::Nullable;
