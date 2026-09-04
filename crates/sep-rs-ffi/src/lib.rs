//! JSON-shaped UniFFI bridge for generated language bindings.
//!
//! JSON keeps the foreign interface stable while the native Rust API evolves
//! richer domain types. Errors retain their category and human-readable
//! message in generated bindings.

#![allow(clippy::large_const_arrays)] // Emitted by UniFFI 0.31 scaffolding.

use sep_rs::{
    ArtifactValidationRequest, BundleSpec, BundleValidationRequest, DeviceSpec, generate_bundle,
    generate_device, model_profiles, validate_artifact_input, validate_bundle_input,
};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Debug, thiserror::Error)]
pub enum SepToolsError {
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("operation failed: {message}")]
    OperationFailed { message: String },
}

fn model_profiles_json() -> Result<String, SepToolsError> {
    encode(&model_profiles())
}

fn validate_artifact_json(request_json: String) -> Result<String, SepToolsError> {
    let request = decode::<ArtifactValidationRequest>(&request_json)?;
    encode(&validate_artifact_input(&request))
}

fn validate_bundle_json(request_json: String) -> Result<String, SepToolsError> {
    let request = decode::<BundleValidationRequest>(&request_json)?;
    let result =
        validate_bundle_input(&request).map_err(|error| SepToolsError::InvalidRequest {
            message: error.to_string(),
        })?;
    encode(&result)
}

fn generate_device_json(request_json: String) -> Result<String, SepToolsError> {
    let request = decode::<DeviceSpec>(&request_json)?;
    let artifact = generate_device(&request).map_err(|error| SepToolsError::OperationFailed {
        message: error.to_string(),
    })?;
    encode(&artifact)
}

fn generate_bundle_json(request_json: String) -> Result<String, SepToolsError> {
    let request = decode::<BundleSpec>(&request_json)?;
    let bundle = generate_bundle(&request).map_err(|error| SepToolsError::OperationFailed {
        message: error.to_string(),
    })?;
    encode(&bundle)
}

fn decode<T: DeserializeOwned>(json: &str) -> Result<T, SepToolsError> {
    serde_json::from_str(json).map_err(|error| SepToolsError::InvalidRequest {
        message: error.to_string(),
    })
}

fn encode<T: Serialize>(value: &T) -> Result<String, SepToolsError> {
    serde_json::to_string(value).map_err(|error| SepToolsError::OperationFailed {
        message: error.to_string(),
    })
}

uniffi::include_scaffolding!("sep_tools");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_cross_the_json_boundary() {
        let profiles = model_profiles_json().expect("serialize profiles");
        let profiles: serde_json::Value = serde_json::from_str(&profiles).expect("valid JSON");
        assert!(
            profiles
                .as_array()
                .is_some_and(|profiles| !profiles.is_empty())
        );
    }

    #[test]
    fn malformed_json_is_a_categorized_error() {
        let error = validate_artifact_json("{".to_owned()).expect_err("invalid request");
        assert!(matches!(error, SepToolsError::InvalidRequest { .. }));
    }
}
