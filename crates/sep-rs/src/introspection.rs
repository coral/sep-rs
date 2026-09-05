//! Machine-readable descriptions of the inputs accepted by `sep-rs`.
//!
//! The outer types are ordinary Rust data structures. The schema itself uses
//! JSON Schema Draft 2020-12 so it can be handed directly to form generators
//! and other language-neutral tooling.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactDialect, ArtifactKind, ModelProfileView, Protocol, SepSettingDefinition, SignalingMode,
    SipButtonFeature, Transport, model_profiles, sep_settings,
};

/// Version of the `sep-rs` options document format.
pub const OPTIONS_SCHEMA_VERSION: u32 = 2;

/// JSON-compatible value used for generated schemas without a runtime parser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaValue {
    Null,
    Boolean(bool),
    Integer(i64),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl SchemaValue {
    /// Resolve an RFC 6901 JSON Pointer within this value.
    #[must_use]
    pub fn pointer(&self, pointer: &str) -> Option<&Self> {
        if pointer.is_empty() {
            return Some(self);
        }
        let mut current = self;
        for encoded in pointer.strip_prefix('/')?.split('/') {
            let key = encoded.replace("~1", "/").replace("~0", "~");
            current = match current {
                Self::Object(values) => values.get(&key)?,
                Self::Array(values) => values.get(key.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        Some(current)
    }
}

impl From<&str> for SchemaValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for SchemaValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

fn schema_object<const N: usize>(values: [(&str, SchemaValue); N]) -> SchemaValue {
    SchemaValue::Object(
        values
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

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

    #[must_use]
    pub const fn schema_ref(self) -> &'static str {
        match self {
            Self::Device => "#/$defs/DeviceSpec",
            Self::Defaults => "#/$defs/DefaultSpec",
            Self::Bundle => "#/$defs/BundleSpec",
            Self::ArtifactValidation => "#/$defs/ArtifactValidationRequest",
            Self::BundleValidation => "#/$defs/BundleValidationRequest",
        }
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
            schema_ref: target.schema_ref().to_owned(),
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
    /// Every cataloged enterprise SEP XML setting, including model/protocol variants.
    pub sep_settings: Vec<SepSettingDefinition>,
}

/// Complete reflection document for `sep-rs` inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsCatalog {
    pub schema_version: u32,
    pub targets: Vec<OptionsTargetDefinition>,
    pub choices: OptionsChoices,
    /// JSON Schema Draft 2020-12 document containing all referenced inputs.
    pub schema: SchemaValue,
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
pub fn options() -> &'static OptionsCatalog {
    static OPTIONS: LazyLock<OptionsCatalog> = LazyLock::new(|| build_options(&OptionsTarget::ALL));
    &OPTIONS
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
            schema_object([
                ("title", target.title.clone().into()),
                ("description", target.description.clone().into()),
                ("$ref", target.schema_ref.clone().into()),
            ])
        })
        .collect::<Vec<_>>();

    OptionsCatalog {
        schema_version: OPTIONS_SCHEMA_VERSION,
        targets: target_definitions,
        choices: OptionsChoices {
            model_profiles: model_profiles(),
            protocols: Protocol::ALL.to_vec(),
            signaling_modes: SignalingMode::ALL.to_vec(),
            transports: Transport::ALL.to_vec(),
            artifact_kinds: ArtifactKind::ALL.to_vec(),
            artifact_dialects: ArtifactDialect::ALL.to_vec(),
            sip_button_features: Vec::from(SipButtonFeature::KINDS.map(str::to_owned)),
            sep_settings: sep_settings().settings.clone(),
        },
        schema: schema_object([
            (
                "$schema",
                "https://json-schema.org/draft/2020-12/schema".into(),
            ),
            ("title", "sep-rs input options".into()),
            (
                "description",
                "Supported inputs for generation and validation. Unknown phone model identifiers remain valid and use generic enterprise assumptions."
                    .into(),
            ),
            ("oneOf", SchemaValue::Array(one_of)),
            ("$defs", generated_schema_definitions()),
        ]),
    }
}

include!(concat!(env!("OUT_DIR"), "/options_schema.rs"));

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
            Some(&SchemaValue::from("SIP"))
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
            Some(&SchemaValue::from("#/choices/model_profiles"))
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
