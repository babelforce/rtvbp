use rtvbp_spec_model::{Catalog, CatalogValidationErrors, EnvelopeSpec, EnvelopeValidationErrors};
use thiserror::Error;

/// Link all authored catalogs into the generator process.
#[must_use]
pub fn load() -> Vec<Catalog> {
    vec![
        rtvbp_spec_babelforce_v1::catalog(),
        rtvbp_spec_demo_v1::catalog(),
    ]
}

/// Link all authored envelope descriptions into the generator process.
#[must_use]
pub fn load_envelopes() -> Vec<EnvelopeSpec> {
    vec![rtvbp_spec_babelforce_v1::envelope()]
}

#[derive(Debug, Error)]
#[error("catalog {catalog} is invalid: {source}")]
pub struct ValidationError {
    pub catalog: String,
    #[source]
    pub source: CatalogValidationErrors,
}

/// Validate loaded catalogs before resolving away incomplete source state.
pub fn validate(catalogs: &[Catalog]) -> Result<(), ValidationError> {
    for catalog in catalogs {
        catalog.validate().map_err(|source| ValidationError {
            catalog: catalog.id.to_string(),
            source,
        })?;
    }
    Ok(())
}

#[derive(Debug, Error)]
#[error("envelope {envelope:?} is invalid: {source}")]
pub struct EnvelopeValidationError {
    pub envelope: String,
    #[source]
    pub source: EnvelopeValidationErrors,
}

/// Validate loaded envelopes before target-specific emission.
pub fn validate_envelopes(envelopes: &[EnvelopeSpec]) -> Result<(), EnvelopeValidationError> {
    for envelope in envelopes {
        envelope
            .validate()
            .map_err(|source| EnvelopeValidationError {
                envelope: envelope.id.clone(),
                source,
            })?;
    }
    Ok(())
}
