#![forbid(unsafe_code)]

mod catalog;
mod envelope;
mod nullable;

pub use catalog::{
    Catalog, CatalogId, CatalogItemKind, CatalogValidationError, CatalogValidationErrors, Event,
    EventExample, ExampleSide, Operation, OperationExample, Role, TypeRef,
};
pub use envelope::{
    CodecError, ConstantField, ControlFrame, EnvelopeSpec, ErrorSpec, FieldSpec, FrameKind,
    FrameSpec, WireError, classic_v1,
};
pub use nullable::Nullable;
