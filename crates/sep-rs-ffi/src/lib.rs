//! JSON-shaped UniFFI bridge for generated language bindings.
//!
//! JSON keeps the foreign interface stable while the native Rust API evolves
//! richer domain types. Errors retain their category and human-readable
//! message in generated bindings.

#![allow(clippy::large_const_arrays)] // Emitted by UniFFI 0.31 scaffolding.

use sep_rs::{
    ArtifactValidationRequest, BundleSpec, BundleValidationRequest, DeviceSpec, OptionsTarget,
    PhoneModelId, Protocol, SepSetting, generate_bundle, generate_device, model_profiles, options,
    options_for, phone_options, validate_artifact_input, validate_bundle_input,
    validate_phone_settings,
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

fn options_json(target: Option<String>) -> Result<String, SepToolsError> {
    match target {
        Some(target) => encode(&options_for(target.parse::<OptionsTarget>().map_err(
            |error| SepToolsError::InvalidRequest {
                message: error.to_string(),
            },
        )?)),
        None => encode(options()),
    }
}

fn phone_options_json(model: String, protocol: String) -> Result<String, SepToolsError> {
    let model = model
        .parse::<PhoneModelId>()
        .map_err(|error| SepToolsError::InvalidRequest {
            message: error.to_string(),
        })?;
    encode(&phone_options(&model, parse_protocol(&protocol)?))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PhoneSettingsValidationRequest {
    model: PhoneModelId,
    protocol: Protocol,
    settings: Vec<SepSetting>,
}

fn validate_phone_settings_json(request_json: String) -> Result<String, SepToolsError> {
    let request = decode::<PhoneSettingsValidationRequest>(&request_json)?;
    encode(&validate_phone_settings(
        &request.model,
        request.protocol,
        &request.settings,
    ))
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

fn parse_protocol(value: &str) -> Result<Protocol, SepToolsError> {
    value.parse().map_err(
        |error: sep_rs::ParseProtocolError| SepToolsError::InvalidRequest {
            message: error.to_string(),
        },
    )
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
    fn options_can_be_filtered_at_the_json_boundary() {
        let value = options_json(Some("device".to_owned())).expect("serialize options");
        let value: serde_json::Value = serde_json::from_str(&value).expect("valid JSON");

        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["targets"][0]["target"], "device");
        assert_eq!(value["targets"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn phone_options_are_model_specific() {
        let value = phone_options_json("8841".to_owned(), "sip".to_owned())
            .expect("serialize phone options");
        let value: serde_json::Value = serde_json::from_str(&value).expect("valid JSON");
        assert_eq!(value["model"], "CP-8841");
        assert_eq!(value["settings"].as_array().map(Vec::len), Some(391));
    }

    #[test]
    fn unknown_options_target_is_a_categorized_error() {
        let error = options_json(Some("wat".to_owned())).expect_err("invalid target");
        assert!(matches!(error, SepToolsError::InvalidRequest { .. }));
    }

    #[test]
    fn malformed_json_is_a_categorized_error() {
        let error = validate_artifact_json("{".to_owned()).expect_err("invalid request");
        assert!(matches!(error, SepToolsError::InvalidRequest { .. }));
    }
}
