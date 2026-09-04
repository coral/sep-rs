//! JavaScript bindings for browser and edge WebAssembly hosts.

use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

use crate::{
    ArtifactValidationRequest, BundleSpec, BundleValidationRequest, DeviceSpec, generate_bundle,
    generate_device, model_profiles, options, options_for, validate_artifact_input,
    validate_bundle_input,
};

/// Return the model compatibility profiles as a JavaScript value.
///
/// # Errors
///
/// Returns a JavaScript error if serialization fails.
#[wasm_bindgen(js_name = modelProfiles)]
pub fn model_profiles_js() -> Result<JsValue, JsValue> {
    to_js(&model_profiles())
}

/// Return the machine-readable input catalog used to build dynamic UIs.
///
/// Passing no target returns every supported input. Pass `device`, `defaults`,
/// `bundle`, `artifact_validation`, or `bundle_validation` to select one.
///
/// # Errors
///
/// Returns a JavaScript error if the target is unknown or serialization fails.
#[wasm_bindgen(js_name = options)]
pub fn options_js(target: Option<String>) -> Result<JsValue, JsValue> {
    let catalog = match target {
        Some(target) => options_for(target.parse().map_err(service_error)?),
        None => options(),
    };
    to_js(&catalog)
}

/// Validate one text artifact from a JavaScript request object.
///
/// # Errors
///
/// Returns a JavaScript error if the request cannot be decoded or the result
/// cannot be encoded.
#[wasm_bindgen(js_name = validateArtifact)]
pub fn validate_artifact_js(request: JsValue) -> Result<JsValue, JsValue> {
    let request = from_js::<ArtifactValidationRequest>(request)?;
    to_js(&validate_artifact_input(&request))
}

/// Validate an in-memory bundle from a JavaScript request object.
///
/// # Errors
///
/// Returns a JavaScript error for an invalid request shape or bundle inventory.
#[wasm_bindgen(js_name = validateBundle)]
pub fn validate_bundle_js(request: JsValue) -> Result<JsValue, JsValue> {
    let request = from_js::<BundleValidationRequest>(request)?;
    let result = validate_bundle_input(&request).map_err(service_error)?;
    to_js(&result)
}

/// Generate one device configuration from a JavaScript specification.
///
/// # Errors
///
/// Returns a JavaScript error for an invalid request or generation failure.
#[wasm_bindgen(js_name = generateDevice)]
pub fn generate_device_js(request: JsValue) -> Result<JsValue, JsValue> {
    let request = from_js::<DeviceSpec>(request)?;
    let result = generate_device(&request).map_err(service_error)?;
    to_js(&result)
}

/// Generate a bootstrap bundle from a JavaScript specification.
///
/// # Errors
///
/// Returns a JavaScript error for an invalid request or generation failure.
#[wasm_bindgen(js_name = generateBundle)]
pub fn generate_bundle_js(request: JsValue) -> Result<JsValue, JsValue> {
    let request = from_js::<BundleSpec>(request)?;
    let result = generate_bundle(&request).map_err(service_error)?;
    to_js(&result)
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(service_error)
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true))
        .map_err(service_error)
}

fn service_error(error: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}
