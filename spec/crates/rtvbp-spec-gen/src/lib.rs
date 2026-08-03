#![forbid(unsafe_code)]

pub mod catalogs;
pub mod cli;
pub mod emit;
pub mod resolve;
pub mod write;

use thiserror::Error;

pub use emit::{GeneratedFile, Target, emit_manifest};
pub use resolve::{ResolveError, ResolvedCatalog, resolve};

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error(transparent)]
    Validation(#[from] catalogs::ValidationError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Emit(#[from] emit::EmitError),
}

/// Run the side-effect-free load → validate → resolve → emit pipeline.
pub fn generate(target: Target) -> Result<Vec<GeneratedFile>, GenerateError> {
    let catalogs = catalogs::load();
    catalogs::validate(&catalogs)?;

    let mut files = Vec::new();
    for catalog in catalogs {
        let resolved = resolve(catalog)?;
        match target {
            Target::Manifest => files.extend(emit_manifest(&resolved)?),
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}
