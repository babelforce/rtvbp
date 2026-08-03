#![forbid(unsafe_code)]

pub mod catalogs;
pub mod cli;
pub mod emit;
pub mod resolve;
pub mod write;

use thiserror::Error;

pub use emit::{GeneratedFile, Target, emit_docs, emit_go, emit_go_envelope, emit_manifest};
pub use resolve::{ResolveError, ResolvedCatalog, resolve};

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error(transparent)]
    Validation(#[from] catalogs::ValidationError),
    #[error(transparent)]
    EnvelopeValidation(#[from] catalogs::EnvelopeValidationError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Emit(#[from] emit::EmitError),
    #[error(transparent)]
    GoEmit(#[from] emit::GoEmitError),
    #[error(transparent)]
    DocsEmit(#[from] emit::DocsEmitError),
}

/// Run the side-effect-free load → validate → resolve → emit pipeline.
pub fn generate(target: Target) -> Result<Vec<GeneratedFile>, GenerateError> {
    let catalogs = catalogs::load();
    catalogs::validate(&catalogs)?;
    let envelopes = catalogs::load_envelopes();
    catalogs::validate_envelopes(&envelopes)?;

    let mut files = Vec::new();
    for catalog in catalogs {
        let resolved = resolve(catalog)?;
        match target {
            Target::Manifest => files.extend(emit_manifest(&resolved)?),
            Target::Go => files.extend(emit_go(&resolved)?),
            Target::Docs => files.extend(emit_docs(&resolved, &envelopes)?),
        }
    }
    if target == Target::Go {
        for envelope in &envelopes {
            files.extend(emit_go_envelope(envelope)?);
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}
