//! Cisco enterprise phone bootstrap configuration generation and validation.
//!
//! The crate deliberately separates semantic configuration from Cisco's
//! model- and firmware-dependent wire representations.

mod artifact;
mod bundle;
mod catalog;
mod diagnostic;
mod generation;
mod introspection;
mod legacy;
mod model;
mod service;
mod settings;
mod validation;
mod xml;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use artifact::{
    ArtifactDetection, ArtifactError, ParsedArtifact, detect_artifact, parse_artifact,
};
#[cfg(not(target_arch = "wasm32"))]
pub use bundle::validate_bundle;
pub use bundle::{BundleError, generate_bundle, validate_bundle_files};
pub use catalog::{ModelProfile, PhoneProfile, profiles, resolve_profile};
pub use diagnostic::{Diagnostic, Severity};
pub use generation::{GenerationError, generate_defaults, generate_device};
pub use introspection::{
    OPTIONS_SCHEMA_VERSION, OptionsCatalog, OptionsChoices, OptionsTarget, OptionsTargetDefinition,
    ParseOptionsTargetError, SchemaValue, options, options_for,
};
pub use model::*;
pub use service::{
    ArtifactValidationRequest, BundleValidationRequest, ModelProfileView, ServiceError,
    ValidationResult, model_profiles, validate_artifact_input, validate_bundle_input,
};
pub use settings::{
    PhoneOptionsCatalog, PhoneSettingOption, SepSettingDefinition, SepSettingsCatalog,
    SettingAllowedValue, SettingSelector, SettingValueKind, SettingVariant, phone_options,
    sep_settings, validate_phone_settings,
};
pub use validation::validate;
