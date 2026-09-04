//! JavaScript bindings for browser and edge WebAssembly hosts.

use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

use crate::{
    ArtifactValidationRequest, BundleSpec, BundleValidationRequest, DeviceSpec, generate_bundle,
    generate_device, model_profiles, validate_artifact_input, validate_bundle_input,
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
    serde_wasm_bindgen::to_value(value).map_err(service_error)
}

fn service_error(error: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}
