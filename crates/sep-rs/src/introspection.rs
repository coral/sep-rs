//! Machine-readable descriptions of the inputs accepted by `sep-rs`.
//!
//! The outer types are ordinary Rust data structures. The schema itself uses
//! JSON Schema Draft 2020-12 so it can be handed directly to form generators
//! and other language-neutral tooling.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ArtifactDialect, ArtifactKind, ModelProfileView, Protocol, SignalingMode, Transport,
    model_profiles,
};

/// Version of the `sep-rs` options document format.
pub const OPTIONS_SCHEMA_VERSION: u32 = 1;

/// A top-level input that can be described by the options API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionsTarget {
    Device,
    Defaults,
    Bundle,
    ArtifactValidation,
    BundleValidation,
}

impl OptionsTarget {
    /// Every target currently exposed by the options API.
    pub const ALL: [Self; 5] = [
        Self::Device,
        Self::Defaults,
        Self::Bundle,
        Self::ArtifactValidation,
        Self::BundleValidation,
    ];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Device => "Device generation",
            Self::Defaults => "Defaults generation",
            Self::Bundle => "Bundle generation",
            Self::ArtifactValidation => "Artifact validation",
            Self::BundleValidation => "Bundle validation",
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Device => "Generate the configuration artifact for one phone.",
            Self::Defaults => "Generate protocol defaults and model load declarations.",
            Self::Bundle => "Generate a complete multi-device bootstrap bundle.",
            Self::ArtifactValidation => "Validate one configuration artifact supplied as text.",
            Self::BundleValidation => "Validate an in-memory set of bootstrap files.",
        }
    }

    const fn definition(self) -> &'static str {
        match self {
            Self::Device => "DeviceSpec",
            Self::Defaults => "DefaultSpec",
            Self::Bundle => "BundleSpec",
            Self::ArtifactValidation => "ArtifactValidationRequest",
            Self::BundleValidation => "BundleValidationRequest",
        }
    }

    #[must_use]
    pub fn schema_ref(self) -> String {
        format!("#/$defs/{}", self.definition())
    }
}

impl fmt::Display for OptionsTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Device => "device",
            Self::Defaults => "defaults",
            Self::Bundle => "bundle",
            Self::ArtifactValidation => "artifact_validation",
            Self::BundleValidation => "bundle_validation",
        })
    }
}

impl FromStr for OptionsTarget {
    type Err = ParseOptionsTargetError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "device" => Ok(Self::Device),
            "defaults" => Ok(Self::Defaults),
            "bundle" => Ok(Self::Bundle),
            "artifact_validation" => Ok(Self::ArtifactValidation),
            "bundle_validation" => Ok(Self::BundleValidation),
            _ => Err(ParseOptionsTargetError {
                value: value.to_owned(),
            }),
        }
    }
}

/// Error returned when an unknown options target is requested.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "unknown options target `{value}`; expected device, defaults, bundle, artifact_validation, or bundle_validation"
)]
pub struct ParseOptionsTargetError {
    value: String,
}

/// A discoverable top-level operation and its location in [`OptionsCatalog::schema`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsTargetDefinition {
    pub target: OptionsTarget,
    pub title: String,
    pub description: String,
    pub schema_ref: String,
}

impl From<OptionsTarget> for OptionsTargetDefinition {
    fn from(target: OptionsTarget) -> Self {
        Self {
            target,
            title: target.title().to_owned(),
            description: target.description().to_owned(),
            schema_ref: target.schema_ref(),
        }
    }
}

/// Finite choices and suggestion catalogs useful to a generated UI.
///
/// `model_profiles` is intentionally a suggestion catalog rather than an
/// enum: generators accept unknown models using generic enterprise defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsChoices {
    pub model_profiles: Vec<ModelProfileView>,
    pub protocols: Vec<Protocol>,
    pub signaling_modes: Vec<SignalingMode>,
    pub transports: Vec<Transport>,
    pub artifact_kinds: Vec<ArtifactKind>,
    pub artifact_dialects: Vec<ArtifactDialect>,
    pub sip_button_features: Vec<String>,
}

/// Complete reflection document for `sep-rs` inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsCatalog {
    pub schema_version: u32,
    pub targets: Vec<OptionsTargetDefinition>,
    pub choices: OptionsChoices,
    /// JSON Schema Draft 2020-12 document containing all referenced inputs.
    pub schema: Value,
}

impl OptionsCatalog {
    /// Find one target description without inspecting string identifiers.
    #[must_use]
    pub fn target(&self, target: OptionsTarget) -> Option<&OptionsTargetDefinition> {
        self.targets
            .iter()
            .find(|candidate| candidate.target == target)
    }
}

/// Describe every supported input and every finite choice.
#[must_use]
pub fn options() -> OptionsCatalog {
    build_options(&OptionsTarget::ALL)
}

/// Describe one supported input while retaining the shared choices and schema
/// definitions needed to render it.
#[must_use]
pub fn options_for(target: OptionsTarget) -> OptionsCatalog {
    build_options(&[target])
}

fn build_options(targets: &[OptionsTarget]) -> OptionsCatalog {
    let target_definitions = targets
        .iter()
        .copied()
        .map(OptionsTargetDefinition::from)
        .collect::<Vec<_>>();
    let one_of = target_definitions
        .iter()
        .map(|target| {
            json!({
                "title": target.title,
                "description": target.description,
                "$ref": target.schema_ref,
            })
        })
        .collect::<Vec<_>>();

    OptionsCatalog {
        schema_version: OPTIONS_SCHEMA_VERSION,
        targets: target_definitions,
        choices: OptionsChoices {
            model_profiles: model_profiles(),
            protocols: vec![Protocol::Sccp, Protocol::Sip],
            signaling_modes: vec![
                SignalingMode::NonSecure,
                SignalingMode::Authenticated,
                SignalingMode::Encrypted,
            ],
            transports: vec![Transport::Udp, Transport::Tcp, Transport::Tls],
            artifact_kinds: vec![
                ArtifactKind::DeviceConfiguration,
                ArtifactKind::DefaultConfiguration,
                ArtifactKind::LoadDescriptor,
                ArtifactKind::Firmware,
                ArtifactKind::DialPlan,
                ArtifactKind::SoftKeyPolicy,
                ArtifactKind::Locale,
                ArtifactKind::TrustList,
                ArtifactKind::Other,
            ],
            artifact_dialects: vec![
                ArtifactDialect::EnterpriseXml,
                ArtifactDialect::LegacySipText,
                ArtifactDialect::CompiledBinary,
                ArtifactDialect::SignedXml,
                ArtifactDialect::EncryptedXml,
                ArtifactDialect::Mpp3pcc,
            ],
            sip_button_features: [
                "line",
                "speed_dial",
                "service_uri",
                "blf",
                "intercom",
                "raw",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        },
        schema: json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "sep-rs input options",
            "description": "Supported inputs for generation and validation. Unknown phone model identifiers remain valid and use generic enterprise assumptions.",
            "oneOf": one_of,
            "$defs": definitions(),
        }),
    }
}

#[allow(clippy::too_many_lines)]
fn definitions() -> Value {
    json!({
        "Host": {
            "title": "Host",
            "description": "An IP address or non-empty DNS hostname without whitespace or path separators.",
            "type": "string",
            "minLength": 1,
            "examples": ["call-control.example.net", "192.0.2.10"]
        },
        "PhoneModelId": {
            "title": "Phone model",
            "description": "A known profile ID or alias is recommended, but an unknown non-empty model identifier is accepted.",
            "type": "string",
            "minLength": 1,
            "x-sep-suggestions": "#/choices/model_profiles",
            "examples": ["CP-7965G", "7965"]
        },
        "CallControlEndpoint": {
            "title": "Call-control endpoint",
            "description": "Endpoint priorities must be unique within their containing list. Authenticated or encrypted signaling requires TLS.",
            "type": "object",
            "additionalProperties": false,
            "required": ["host", "port"],
            "properties": {
                "host": { "$ref": "#/$defs/Host" },
                "port": {
                    "title": "Port",
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535,
                    "examples": [2000, 5060, 2443, 5061]
                },
                "priority": {
                    "title": "Priority",
                    "description": "Lower values are preferred; priorities must be unique in one endpoint list.",
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 255,
                    "default": 0
                },
                "transport": {
                    "title": "Transport",
                    "type": "string",
                    "enum": ["udp", "tcp", "tls"],
                    "default": "tcp"
                }
            }
        },
        "SccpSpec": {
            "title": "SCCP settings",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "signaling": {
                    "title": "Signaling security",
                    "type": "string",
                    "enum": ["non_secure", "authenticated", "encrypted"],
                    "default": "non_secure"
                },
                "keepalive_seconds": {
                    "title": "Keepalive interval",
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 65535
                }
            }
        },
        "SipLine": {
            "title": "SIP line",
            "type": "object",
            "additionalProperties": false,
            "required": ["index", "directory_number"],
            "properties": {
                "index": {
                    "title": "Line index",
                    "description": "Indexes must be nonzero and unique within one SIP configuration.",
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 255
                },
                "directory_number": {
                    "title": "Directory number",
                    "type": "string",
                    "minLength": 1,
                    "examples": ["1001"]
                },
                "display_name": { "title": "Display name", "type": "string" },
                "auth_name": { "title": "Authentication username", "type": "string" },
                "auth_secret": {
                    "title": "Authentication secret",
                    "type": "string",
                    "writeOnly": true,
                    "x-sep-secret": true
                }
            }
        },
        "SipButton": {
            "title": "SIP button",
            "description": "Button positions must be nonzero and unique within one SIP configuration.",
            "oneOf": [
                {
                    "title": "Line",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["position", "feature", "line_index"],
                    "properties": {
                        "position": { "type": "integer", "minimum": 1, "maximum": 255 },
                        "feature": { "const": "line" },
                        "line_index": { "type": "integer", "minimum": 1, "maximum": 255 }
                    }
                },
                {
                    "title": "Speed dial",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["position", "feature", "label", "target"],
                    "properties": {
                        "position": { "type": "integer", "minimum": 1, "maximum": 255 },
                        "feature": { "const": "speed_dial" },
                        "label": { "type": "string" },
                        "target": { "type": "string" }
                    }
                },
                {
                    "title": "Service URI",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["position", "feature", "label", "uri"],
                    "properties": {
                        "position": { "type": "integer", "minimum": 1, "maximum": 255 },
                        "feature": { "const": "service_uri" },
                        "label": { "type": "string" },
                        "uri": { "type": "string" }
                    }
                },
                {
                    "title": "Busy lamp field",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["position", "feature", "label", "target"],
                    "properties": {
                        "position": { "type": "integer", "minimum": 1, "maximum": 255 },
                        "feature": { "const": "blf" },
                        "label": { "type": "string" },
                        "target": { "type": "string" }
                    }
                },
                {
                    "title": "Intercom",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["position", "feature", "line_index"],
                    "properties": {
                        "position": { "type": "integer", "minimum": 1, "maximum": 255 },
                        "feature": { "const": "intercom" },
                        "line_index": { "type": "integer", "minimum": 1, "maximum": 255 }
                    }
                },
                {
                    "title": "Raw vendor feature",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["position", "feature", "feature_id"],
                    "properties": {
                        "position": { "type": "integer", "minimum": 1, "maximum": 255 },
                        "feature": { "const": "raw" },
                        "feature_id": { "type": "integer", "minimum": 0, "maximum": 65535 },
                        "label": { "type": "string" },
                        "target": { "type": "string" }
                    }
                }
            ]
        },
        "SipTimers": {
            "title": "SIP timers",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "register_expires_seconds": { "type": "integer", "minimum": 0, "maximum": 4_294_967_295_u64 },
                "invite_expires_seconds": { "type": "integer", "minimum": 0, "maximum": 4_294_967_295_u64 },
                "keepalive_seconds": { "type": "integer", "minimum": 0, "maximum": 65535 }
            },
            "default": {}
        },
        "MediaPortRange": {
            "title": "Media port range",
            "description": "Both ports must be nonzero and start must not exceed end.",
            "type": "object",
            "additionalProperties": false,
            "required": ["start", "end"],
            "properties": {
                "start": { "type": "integer", "minimum": 1, "maximum": 65535, "default": 16384 },
                "end": { "type": "integer", "minimum": 1, "maximum": 65535, "default": 32766 }
            }
        },
        "SipSpec": {
            "title": "SIP settings",
            "type": "object",
            "additionalProperties": false,
            "required": ["lines"],
            "properties": {
                "signaling": {
                    "title": "Signaling security",
                    "type": "string",
                    "enum": ["non_secure", "authenticated", "encrypted"],
                    "default": "non_secure"
                },
                "lines": {
                    "title": "Lines",
                    "type": "array",
                    "minItems": 1,
                    "items": { "$ref": "#/$defs/SipLine" }
                },
                "buttons": {
                    "title": "Buttons",
                    "type": "array",
                    "items": { "$ref": "#/$defs/SipButton" },
                    "default": []
                },
                "timers": { "$ref": "#/$defs/SipTimers" },
                "media_ports": { "$ref": "#/$defs/MediaPortRange" },
                "outbound_proxy": { "$ref": "#/$defs/Host" }
            }
        },
        "ProtocolSpec": {
            "title": "Protocol configuration",
            "oneOf": [
                {
                    "title": "SCCP",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["sccp"],
                    "properties": { "sccp": { "$ref": "#/$defs/SccpSpec" } }
                },
                {
                    "title": "SIP",
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["sip"],
                    "properties": { "sip": { "$ref": "#/$defs/SipSpec" } }
                }
            ]
        },
        "ServiceUrls": {
            "title": "Phone service URLs",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "services": { "type": "string" },
                "directory": { "type": "string" },
                "messages": { "type": "string" },
                "information": { "type": "string" },
                "idle": { "type": "string" }
            },
            "default": {}
        },
        "DeviceSpec": {
            "title": "Device generation",
            "type": "object",
            "additionalProperties": false,
            "required": ["mac", "model", "protocol", "endpoints"],
            "properties": {
                "mac": {
                    "title": "MAC address",
                    "description": "Six octets in compact, colon-separated, hyphen-separated, SEP-prefixed, or SIP-prefixed form.",
                    "type": "string",
                    "format": "mac",
                    "examples": ["00:08:2F:B6:B4:AA", "SEP00082FB6B4AA"]
                },
                "model": { "$ref": "#/$defs/PhoneModelId" },
                "firmware": { "title": "Firmware load", "type": "string" },
                "protocol": { "$ref": "#/$defs/ProtocolSpec" },
                "endpoints": {
                    "title": "Call-control endpoints",
                    "type": "array",
                    "minItems": 1,
                    "items": { "$ref": "#/$defs/CallControlEndpoint" }
                },
                "phone_label": { "type": "string" },
                "time_zone": { "type": "string" },
                "date_template": { "type": "string" },
                "ntp_server": { "$ref": "#/$defs/Host" },
                "locale": { "type": "string" },
                "services": { "$ref": "#/$defs/ServiceUrls" }
            }
        },
        "ModelLoad": {
            "title": "Model firmware load",
            "type": "object",
            "additionalProperties": false,
            "required": ["model", "firmware"],
            "properties": {
                "model": { "$ref": "#/$defs/PhoneModelId" },
                "model_id": {
                    "description": "Numeric loadInformation suffix for an unlisted model.",
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 65535
                },
                "firmware": { "type": "string" }
            }
        },
        "DefaultSpec": {
            "title": "Defaults generation",
            "type": "object",
            "additionalProperties": false,
            "required": ["protocol", "endpoints"],
            "properties": {
                "protocol": { "type": "string", "enum": ["sccp", "sip"] },
                "firmware": { "type": "string" },
                "endpoints": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "$ref": "#/$defs/CallControlEndpoint" }
                },
                "model_loads": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/ModelLoad" },
                    "default": []
                },
                "time_zone": { "type": "string" },
                "date_template": { "type": "string" },
                "ntp_server": { "$ref": "#/$defs/Host" },
                "locale": { "type": "string" }
            }
        },
        "ExternalArtifact": {
            "title": "External artifact",
            "type": "object",
            "additionalProperties": false,
            "required": ["filename", "kind"],
            "properties": {
                "filename": { "type": "string", "minLength": 1 },
                "kind": {
                    "type": "string",
                    "enum": [
                        "device_configuration", "default_configuration", "load_descriptor",
                        "firmware", "dial_plan", "soft_key_policy", "locale", "trust_list", "other"
                    ]
                },
                "required": { "type": "boolean", "default": true },
                "description": { "type": "string" }
            }
        },
        "BundleSpec": {
            "title": "Bundle generation",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "devices": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/DeviceSpec" },
                    "default": []
                },
                "defaults": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/DefaultSpec" },
                    "default": []
                },
                "external_artifacts": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/ExternalArtifact" },
                    "default": []
                }
            },
            "default": {}
        },
        "ArtifactValidationRequest": {
            "title": "Artifact validation",
            "type": "object",
            "additionalProperties": false,
            "required": ["filename", "contents"],
            "properties": {
                "filename": { "type": "string", "minLength": 1 },
                "contents": { "type": "string", "contentMediaType": "text/plain", "x-sep-multiline": true },
                "model": { "$ref": "#/$defs/PhoneModelId" }
            }
        },
        "BundleFile": {
            "title": "Bundle file",
            "type": "object",
            "additionalProperties": false,
            "required": ["filename"],
            "properties": {
                "filename": { "type": "string", "minLength": 1 },
                "contents": { "type": "string", "contentMediaType": "text/plain", "x-sep-multiline": true }
            }
        },
        "BundleValidationRequest": {
            "title": "Bundle validation",
            "type": "object",
            "additionalProperties": false,
            "required": ["files"],
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "$ref": "#/$defs/BundleFile" }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_targets_are_discoverable_and_resolvable() {
        let catalog = options();

        assert_eq!(catalog.schema_version, OPTIONS_SCHEMA_VERSION);
        assert_eq!(catalog.targets.len(), OptionsTarget::ALL.len());
        for target in OptionsTarget::ALL {
            let definition = catalog.target(target).expect("target is present");
            assert!(
                catalog
                    .schema
                    .pointer(&definition.schema_ref[1..])
                    .is_some()
            );
        }
    }

    #[test]
    fn one_target_keeps_shared_schema_and_choices() {
        let catalog = options_for(OptionsTarget::Device);

        assert_eq!(catalog.targets.len(), 1);
        assert_eq!(catalog.targets[0].target, OptionsTarget::Device);
        assert!(!catalog.choices.model_profiles.is_empty());
        assert_eq!(
            catalog.schema.pointer("/$defs/ProtocolSpec/oneOf/1/title"),
            Some(&json!("SIP"))
        );
    }

    #[test]
    fn catalogs_include_open_models_and_all_finite_choices() {
        let catalog = options();

        assert_eq!(catalog.choices.model_profiles, model_profiles());
        assert_eq!(catalog.choices.protocols, [Protocol::Sccp, Protocol::Sip]);
        assert_eq!(
            catalog.choices.sip_button_features,
            [
                "line",
                "speed_dial",
                "service_uri",
                "blf",
                "intercom",
                "raw"
            ]
        );
        assert_eq!(
            catalog
                .schema
                .pointer("/$defs/PhoneModelId/x-sep-suggestions"),
            Some(&json!("#/choices/model_profiles"))
        );
    }

    #[test]
    fn target_strings_round_trip() {
        for target in OptionsTarget::ALL {
            assert_eq!(target.to_string().parse(), Ok(target));
        }
        assert!("wat".parse::<OptionsTarget>().is_err());
    }
}
