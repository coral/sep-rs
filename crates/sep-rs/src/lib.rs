//! Cisco enterprise phone bootstrap configuration generation and validation.
//!
//! The crate deliberately separates semantic configuration from Cisco's
//! model- and firmware-dependent wire representations.

mod artifact;
mod bundle;
mod catalog;
mod diagnostic;
mod generation;
mod legacy;
mod model;
mod service;
mod validation;
mod xml;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use artifact::{
    ArtifactDetection, ArtifactError, ParsedArtifact, detect_artifact, parse_artifact,
};
pub use bundle::{BundleError, generate_bundle, validate_bundle, validate_bundle_files};
pub use catalog::{ModelProfile, PhoneProfile, profiles, resolve_profile};
pub use diagnostic::{Diagnostic, Severity};
pub use generation::{GenerationError, generate_defaults, generate_device};
pub use model::*;
pub use service::{
    ArtifactValidationRequest, BundleValidationRequest, ModelProfileView, ServiceError,
    ValidationResult, model_profiles, validate_artifact_input, validate_bundle_input,
};
pub use validation::validate;
