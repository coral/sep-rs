//! Platform-neutral request and response helpers shared by native, browser,
//! and edge hosts.

use serde::{Deserialize, Serialize};

use crate::artifact::{ArtifactDetection, detect_artifact, parse_artifact};
use crate::bundle::validate_bundle_files;
use crate::catalog::profiles;
use crate::diagnostic::{Diagnostic, Severity, code};
use crate::model::{ArtifactDialect, BundleFile, PhoneModelId, Protocol};
use crate::validation::validate;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactValidationRequest {
    pub filename: String,
    pub contents: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<PhoneModelId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BundleValidationRequest {
    pub files: Vec<BundleFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection: Option<ArtifactDetection>,
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelProfileView {
    pub id: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub model_id: u16,
    pub protocols: Vec<Protocol>,
    pub dialects: Vec<ArtifactDialect>,
    pub load_prefixes: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ServiceError {
    pub code: String,
    pub message: String,
}

impl ServiceError {
    #[must_use]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_request".to_owned(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn generation(message: impl Into<String>) -> Self {
        Self {
            code: "generation_failed".to_owned(),
            message: message.into(),
        }
    }
}

#[must_use]
pub fn model_profiles() -> Vec<ModelProfileView> {
    profiles()
        .iter()
        .map(|profile| ModelProfileView {
            id: profile.id.to_owned(),
            display_name: profile.display_name.to_owned(),
            aliases: profile
                .aliases
                .iter()
                .map(|alias| (*alias).to_owned())
                .collect(),
            model_id: profile.model_id,
            protocols: profile.protocols.to_vec(),
            dialects: profile.dialects.to_vec(),
            load_prefixes: profile
                .load_prefixes
                .iter()
                .map(|prefix| (*prefix).to_owned())
                .collect(),
        })
        .collect()
}

#[must_use]
pub fn validate_artifact_input(request: &ArtifactValidationRequest) -> ValidationResult {
    let detection = detect_artifact(&request.contents, Some(&request.filename)).ok();
    let diagnostics = match parse_artifact(&request.contents, Some(&request.filename)) {
        Ok(parsed) => validate(&parsed, request.model.as_ref()),
        Err(error) => {
            vec![Diagnostic::error(code::MALFORMED, error.to_string()).at(&request.filename)]
        }
    };
    ValidationResult {
        detection,
        valid: !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        diagnostics,
    }
}

/// # Errors
///
/// Returns a request error when the bundle inventory or supplied filename set
/// is malformed. Artifact-level problems are returned as diagnostics.
pub fn validate_bundle_input(
    request: &BundleValidationRequest,
) -> Result<ValidationResult, ServiceError> {
    let diagnostics = validate_bundle_files(&request.files)
        .map_err(|error| ServiceError::invalid_request(error.to_string()))?;
    Ok(ValidationResult {
        detection: None,
        valid: !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noncanonical_filename_warns_without_invalidating_parsed_xml() {
        let result = validate_artifact_input(&ArtifactValidationRequest {
            filename: "aids.xml".to_owned(),
            contents: concat!(
                "<device>",
                "<deviceProtocol>SCCP</deviceProtocol>",
                "<devicePool><callManagerGroup><members>",
                "<member priority=\"0\"><callManager>",
                "<ports><ethernetPhonePort>2000</ethernetPhonePort></ports>",
                "<processNodeName>pbx.example.net</processNodeName>",
                "</callManager></member>",
                "</members></callManagerGroup></devicePool>",
                "</device>",
            )
            .to_owned(),
            model: None,
        });

        assert!(result.valid);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == code::FILENAME_MAC_MISMATCH
                && diagnostic.severity == Severity::Warning
        }));
    }

    #[test]
    fn missing_nested_xml_field_becomes_a_located_diagnostic() {
        let result = validate_artifact_input(&ArtifactValidationRequest {
            filename: "SEP001122334455.cnf.xml".to_owned(),
            contents: concat!(
                "<device>",
                "<deviceProtocol>SCCP</deviceProtocol>",
                "<devicePool>",
                "<dateTimeSetting><ntps><ntp><ntpMode>Unicast</ntpMode></ntp></ntps></dateTimeSetting>",
                "<callManagerGroup><members>",
                "<member priority=\"0\"><callManager>",
                "<ports><ethernetPhonePort>2000</ethernetPhonePort></ports>",
                "<processNodeName>pbx.example.net</processNodeName>",
                "</callManager></member>",
                "</members></callManagerGroup>",
                "</devicePool>",
                "</device>",
            )
            .to_owned(),
            model: None,
        });

        assert_eq!(
            result
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.message == "NTP server name is required")
                .and_then(|diagnostic| diagnostic.path.as_deref()),
            Some("device.devicePool.dateTimeSetting.ntps.ntp[0].name")
        );
        assert!(result.detection.is_some());
    }
}
