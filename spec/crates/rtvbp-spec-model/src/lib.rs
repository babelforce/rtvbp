#![forbid(unsafe_code)]

mod catalog;
mod envelope;
mod nullable;

pub use catalog::{
    Catalog, CatalogFixture, CatalogId, CatalogItemKind, CatalogValidationError,
    CatalogValidationErrors, Event, EventExample, ExampleSide, FixtureTarget, Operation,
    OperationExample, OperationRejection, RESERVED_TRANSPORT_METHOD_PREFIX, Role, TypeRef,
};
pub use envelope::{
    CodecError, ConstantField, ControlFrame, EnvelopeFixture, EnvelopeSpec,
    EnvelopeValidationError, EnvelopeValidationErrors, ErrorCodeSpec, ErrorSpec, FieldSpec,
    FrameKind, FrameSpec, WireError,
};
pub use nullable::Nullable;
