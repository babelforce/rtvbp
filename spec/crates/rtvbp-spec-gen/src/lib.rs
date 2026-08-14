#![forbid(unsafe_code)]

pub mod catalogs;
pub mod cli;
pub mod emit;
pub mod resolve;
pub mod write;

use thiserror::Error;

pub use emit::{
    GeneratedFile, Target, emit_docs, emit_go, emit_go_envelope, emit_go_profiles, emit_manifest,
    emit_profile_docs, emit_profile_manifest, emit_profile_vectors, emit_rust, emit_rust_envelope,
    emit_rust_profiles, emit_typescript, emit_typescript_envelope, emit_typescript_index,
    emit_typescript_profiles, emit_vectors,
};
pub use resolve::{ResolveError, ResolvedCatalog, resolve};

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error(transparent)]
    Validation(#[from] catalogs::ValidationError),
    #[error(transparent)]
    EnvelopeValidation(#[from] catalogs::EnvelopeValidationError),
    #[error(transparent)]
    ProfileValidation(#[from] catalogs::ProfileValidationError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Emit(#[from] emit::EmitError),
    #[error(transparent)]
    GoEmit(#[from] emit::GoEmitError),
    #[error(transparent)]
    RustEmit(#[from] emit::RustEmitError),
    #[error(transparent)]
    DocsEmit(#[from] emit::DocsEmitError),
    #[error(transparent)]
    VectorEmit(#[from] emit::VectorEmitError),
    #[error(transparent)]
    ProfileEmit(#[from] emit::ProfileEmitError),
    #[error(transparent)]
    TypeScriptEmit(#[from] emit::TypeScriptEmitError),
}

/// Run the side-effect-free load → validate → resolve → emit pipeline.
pub fn generate(target: Target) -> Result<Vec<GeneratedFile>, GenerateError> {
    let catalogs = catalogs::load();
    catalogs::validate(&catalogs)?;
    let envelopes = catalogs::load_envelopes();
    catalogs::validate_envelopes(&envelopes)?;
    let profiles = catalogs::load_profiles();
    catalogs::validate_profiles(&profiles, &catalogs, &envelopes)?;

    let catalog_ids = catalogs
        .iter()
        .map(|catalog| catalog.id.clone())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    for catalog in catalogs {
        let resolved = resolve(catalog)?;
        match target {
            Target::Manifest => files.extend(emit_manifest(&resolved)?),
            Target::Go => files.extend(emit_go(&resolved)?),
            Target::Rust => files.extend(emit_rust(&resolved)?),
            Target::Docs => files.extend(emit_docs(&resolved, &envelopes)?),
            Target::Vectors => files.extend(emit_vectors(&resolved, &envelopes)?),
            Target::TypeScript => files.extend(emit_typescript(&resolved)?),
        }
    }
    if matches!(target, Target::Go | Target::Rust | Target::TypeScript) {
        for envelope in &envelopes {
            match target {
                Target::Go => files.extend(emit_go_envelope(envelope)?),
                Target::Rust => files.extend(emit_rust_envelope(envelope)?),
                Target::TypeScript => files.extend(emit_typescript_envelope(envelope)?),
                _ => unreachable!(),
            }
        }
    }
    match target {
        Target::Manifest => files.extend(emit_profile_manifest(&profiles)?),
        Target::Go => files.extend(emit_go_profiles(&profiles)?),
        Target::Rust => files.extend(emit_rust_profiles(&profiles)?),
        Target::TypeScript => {
            files.extend(emit_typescript_profiles(&profiles)?);
            files.extend(emit_typescript_index(&catalog_ids, &envelopes));
        }
        Target::Docs => files.extend(emit_profile_docs(&profiles)?),
        Target::Vectors => files.extend(emit_profile_vectors(&profiles)?),
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}
