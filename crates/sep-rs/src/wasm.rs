//! JavaScript bindings for browser and edge WebAssembly hosts.

use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

use crate::{
    ArtifactValidationRequest, BundleSpec, BundleValidationRequest, DeviceSpec, PhoneModelId,
    Protocol, SepSetting, generate_bundle, generate_device, model_profiles, options, options_for,
    phone_options, validate_artifact_input, validate_bundle_input, validate_phone_settings,
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
    match target {
        Some(target) => to_js(&options_for(target.parse().map_err(service_error)?)),
        None => to_js(options()),
    }
}

/// Return every setting applicable to one phone model and protocol.
///
/// # Errors
///
/// Returns a JavaScript error for an invalid model or protocol value.
#[wasm_bindgen(js_name = phoneOptions)]
pub fn phone_options_js(model: String, protocol: String) -> Result<JsValue, JsValue> {
    let model = model.parse::<PhoneModelId>().map_err(service_error)?;
    let protocol = parse_protocol(&protocol)?;
    to_js(&phone_options(&model, protocol))
}

/// Validate advanced SEP settings for one phone model and protocol.
///
/// # Errors
///
/// Returns a JavaScript error if arguments cannot be decoded.
#[wasm_bindgen(js_name = validatePhoneSettings)]
pub fn validate_phone_settings_js(
    model: String,
    protocol: String,
    settings: JsValue,
) -> Result<JsValue, JsValue> {
    let model = model.parse::<PhoneModelId>().map_err(service_error)?;
    let protocol = parse_protocol(&protocol)?;
    let settings = from_js::<Vec<SepSetting>>(settings)?;
    to_js(&validate_phone_settings(&model, protocol, &settings))
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

fn parse_protocol(value: &str) -> Result<Protocol, JsValue> {
    value.parse().map_err(service_error)
}
