//! Model-aware catalog and validation for the enterprise SEP setting surface.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::LazyLock;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::catalog::resolve_profile;
use crate::diagnostic::{Diagnostic, code};
use crate::model::{PhoneModelId, Protocol, SepSetting, SepSettingValue};

include!(concat!(env!("OUT_DIR"), "/sep_settings_catalog.rs"));

static CATALOG: LazyLock<SepSettingsCatalog> = LazyLock::new(generated_sep_settings_catalog);
static PATTERNS: LazyLock<HashMap<String, Regex>> = LazyLock::new(|| {
    CATALOG
        .settings
        .iter()
        .flat_map(|definition| &definition.variants)
        .filter_map(|variant| variant.pattern.as_deref())
        .map(|pattern| {
            let expression = Regex::new(&format!("^(?:{pattern})$"))
                .expect("catalog patterns are validated by build.rs");
            (pattern.to_owned(), expression)
        })
        .collect()
});

/// Data type accepted by one SEP XML setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingValueKind {
    Boolean,
    Integer,
    String,
}

impl fmt::Display for SettingValueKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::String => "string",
        })
    }
}

/// One enum/list choice, including the phone-admin label when available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingAllowedValue {
    pub value: SepSettingValue,
    pub label: String,
}

/// A phone and protocol pair to which a constraint applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingSelector {
    /// Canonical model ID, or `*` for any built-in model.
    pub model: String,
    pub protocol: Protocol,
}

/// Validation rule for one model/protocol subset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingVariant {
    pub value_kind: SettingValueKind,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<SepSettingValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<SettingAllowedValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_characters: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub multiple: bool,
    /// Empty means common to enterprise phones. Otherwise the selector pair
    /// must match; this is deliberately not two cross-product lists.
    #[serde(default)]
    pub selectors: Vec<SettingSelector>,
}

/// All known model-specific variants for one normalized SEP XML path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SepSettingDefinition {
    pub path: String,
    pub name: String,
    pub title: String,
    pub section: String,
    pub variants: Vec<SettingVariant>,
}

/// Complete, unfiltered setting catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SepSettingsCatalog {
    pub schema_version: u32,
    pub settings: Vec<SepSettingDefinition>,
}

/// One setting after resolving its model/protocol-specific rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoneSettingOption {
    pub path: String,
    pub name: String,
    pub title: String,
    pub section: String,
    #[serde(flatten)]
    pub constraint: SettingVariant,
}

/// Setting catalog resolved for one concrete phone and protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhoneOptionsCatalog {
    pub schema_version: u32,
    pub requested_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub protocol: Protocol,
    pub supported: bool,
    /// Dialect described by `settings`. Legacy SIP text has a separate grammar.
    pub settings_dialect: crate::model::ArtifactDialect,
    pub dialects: Vec<crate::model::ArtifactDialect>,
    pub settings: Vec<PhoneSettingOption>,
}

/// Return every cataloged setting and all model-specific rule variants.
#[must_use]
pub fn sep_settings() -> &'static SepSettingsCatalog {
    &CATALOG
}

/// Resolve all applicable settings for one phone and protocol.
#[must_use]
pub fn phone_options(model: &PhoneModelId, protocol: Protocol) -> PhoneOptionsCatalog {
    let profile = resolve_profile(model);
    let canonical_model = profile.map(|profile| profile.id);
    let known_but_unsupported = profile.is_some_and(|profile| !profile.supports_protocol(protocol));
    let settings = if known_but_unsupported {
        Vec::new()
    } else {
        CATALOG
            .settings
            .iter()
            .filter_map(|definition| {
                selected_variant(definition, canonical_model, protocol).map(|variant| {
                    let mut constraint = variant.clone();
                    constraint.selectors.clear();
                    PhoneSettingOption {
                        path: definition.path.clone(),
                        name: definition.name.clone(),
                        title: definition.title.clone(),
                        section: definition.section.clone(),
                        constraint,
                    }
                })
            })
            .collect()
    };
    PhoneOptionsCatalog {
        schema_version: CATALOG.schema_version,
        requested_model: model.to_string(),
        model: canonical_model.map(str::to_owned),
        protocol,
        supported: profile.is_some_and(|profile| profile.supports_protocol(protocol)),
        settings_dialect: crate::model::ArtifactDialect::EnterpriseXml,
        dialects: profile.map_or_else(Vec::new, |profile| profile.dialects.to_vec()),
        settings,
    }
}

/// Validate path/value settings against the catalog for a phone and protocol.
///
/// Unknown paths are errors. Generation has a separate explicit
/// `allow_unknown_settings` escape hatch for newer firmware fields.
#[must_use]
pub fn validate_phone_settings(
    model: &PhoneModelId,
    protocol: Protocol,
    settings: &[SepSetting],
) -> Vec<Diagnostic> {
    validate_phone_settings_inner(model, protocol, settings, false)
}

pub(crate) fn validate_phone_settings_inner(
    model: &PhoneModelId,
    protocol: Protocol,
    settings: &[SepSetting],
    allow_unknown: bool,
) -> Vec<Diagnostic> {
    let profile = resolve_profile(model);
    let canonical_model = profile.map(|profile| profile.id);
    let mut diagnostics = Vec::new();
    let mut seen = HashSet::new();

    if let Some(profile) = profile
        && !profile.supports_protocol(protocol)
    {
        diagnostics.push(Diagnostic::error(
            code::MODEL_PROTOCOL_MISMATCH,
            format!("{} does not support {protocol}", profile.id),
        ));
    }

    for (index, setting) in settings.iter().enumerate() {
        let location = format!("settings[{index}]");
        let normalized = setting.path.normalized();
        if !seen.insert(setting.path.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    code::DUPLICATE_SETTING,
                    format!("setting path `{}` occurs more than once", setting.path),
                )
                .at(location),
            );
            continue;
        }
        let Some(definition) = CATALOG
            .settings
            .iter()
            .find(|definition| definition.path == normalized)
        else {
            if !allow_unknown {
                diagnostics.push(
                    Diagnostic::error(
                        code::UNKNOWN_FIELD,
                        format!("`{normalized}` is not in the SEP setting catalog"),
                    )
                    .at(location),
                );
            }
            continue;
        };
        let Some(variant) = selected_variant(definition, canonical_model, protocol) else {
            diagnostics.push(
                Diagnostic::error(
                    code::SETTING_NOT_APPLICABLE,
                    format!(
                        "`{normalized}` is known but not applicable to {} {protocol}",
                        canonical_model.unwrap_or_else(|| model.as_str())
                    ),
                )
                .at(location),
            );
            continue;
        };
        validate_value(&setting.value, variant, &location, &mut diagnostics);
    }
    diagnostics
}

pub(crate) fn setting_contains_secret(
    model: &PhoneModelId,
    protocol: Protocol,
    setting: &SepSetting,
) -> bool {
    let normalized = setting.path.normalized();
    let catalog_secret = CATALOG.settings.iter().any(|definition| {
        definition.path == normalized
            && selected_variant(
                definition,
                resolve_profile(model).map(|item| item.id),
                protocol,
            )
            .is_some_and(|variant| variant.secret)
    });
    let lower = setting.path.as_str().to_ascii_lowercase();
    catalog_secret || lower.contains("password") || lower.contains("secret")
}

pub(crate) fn validate_xml_settings(
    source: &str,
    model: Option<&PhoneModelId>,
    protocol: Protocol,
) -> Vec<Diagnostic> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<XmlFrame>::new();
    let mut diagnostics = Vec::new();
    let mut seen_paths = HashSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if let Some(parent) = stack.last_mut() {
                    parent.has_children = true;
                }
                let path = xml_child_path(
                    stack.last().map(|frame| frame.path.as_str()),
                    start.name().as_ref(),
                );
                validate_xml_attributes(
                    &start,
                    &path,
                    model,
                    protocol,
                    &mut seen_paths,
                    &mut diagnostics,
                );
                stack.push(XmlFrame {
                    path,
                    text: String::new(),
                    text_is_valid: true,
                    has_children: false,
                });
            }
            Ok(Event::Empty(start)) => {
                if let Some(parent) = stack.last_mut() {
                    parent.has_children = true;
                }
                let path = xml_child_path(
                    stack.last().map(|frame| frame.path.as_str()),
                    start.name().as_ref(),
                );
                validate_xml_attributes(
                    &start,
                    &path,
                    model,
                    protocol,
                    &mut seen_paths,
                    &mut diagnostics,
                );
                validate_raw_xml_value(
                    &path,
                    "",
                    model,
                    protocol,
                    &mut seen_paths,
                    &mut diagnostics,
                );
            }
            Ok(Event::Text(text)) => {
                if let Some(frame) = stack.last_mut() {
                    frame
                        .text
                        .push_str(&text.xml_content(XmlVersion::default()));
                }
            }
            Ok(Event::CData(text)) => {
                if let Some(frame) = stack.last_mut() {
                    frame
                        .text
                        .push_str(&text.xml_content(XmlVersion::default()));
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                append_xml_reference(stack.last_mut(), reference.as_ref(), &mut diagnostics);
            }
            Ok(Event::End(_)) => {
                if let Some(frame) = stack.pop()
                    && !frame.has_children
                    && frame.text_is_valid
                {
                    validate_raw_xml_value(
                        &frame.path,
                        frame.text.trim(),
                        model,
                        protocol,
                        &mut seen_paths,
                        &mut diagnostics,
                    );
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    code::MALFORMED,
                    format!("could not inspect SEP XML settings: {error}"),
                ));
                break;
            }
        }
    }
    diagnostics
}

struct XmlFrame {
    path: String,
    text: String,
    text_is_valid: bool,
    has_children: bool,
}

fn append_xml_reference(
    frame: Option<&mut XmlFrame>,
    reference: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(frame) = frame else {
        return;
    };
    match quick_xml::escape::unescape(&format!("&{reference};")) {
        Ok(value) => frame.text.push_str(&value),
        Err(error) => {
            frame.text_is_valid = false;
            diagnostics.push(
                Diagnostic::error(code::MALFORMED, format!("invalid XML text: {error}"))
                    .at(&frame.path),
            );
        }
    }
}

fn xml_child_path(parent: Option<&str>, name: &str) -> String {
    parent.map_or_else(|| format!("/{name}"), |parent| format!("{parent}/{name}"))
}

fn validate_xml_attributes(
    start: &BytesStart<'_>,
    path: &str,
    model: Option<&PhoneModelId>,
    protocol: Protocol,
    seen_paths: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for attribute in start.attributes() {
        let attribute = match attribute {
            Ok(attribute) => attribute,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::error(code::MALFORMED, format!("invalid XML attribute: {error}"))
                        .at(path),
                );
                continue;
            }
        };
        let name = attribute.key.as_ref();
        if name == "xmlns" || name.starts_with("xmlns:") || name == "xsi:schemaLocation" {
            continue;
        }
        match attribute.normalized_value(XmlVersion::default()) {
            Ok(value) => validate_raw_xml_value(
                &format!("{path}/@{name}"),
                &value,
                model,
                protocol,
                seen_paths,
                diagnostics,
            ),
            Err(error) => diagnostics.push(
                Diagnostic::error(code::MALFORMED, format!("invalid XML attribute: {error}"))
                    .at(format!("{path}/@{name}")),
            ),
        }
    }
}

fn validate_raw_xml_value(
    actual_path: &str,
    raw: &str,
    model: Option<&PhoneModelId>,
    protocol: Protocol,
    seen_paths: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = definition_for_actual_path(actual_path) else {
        if catalog_has_descendants(actual_path) {
            return;
        }
        diagnostics.push(
            Diagnostic::error(
                code::UNKNOWN_FIELD,
                "field is not in the SEP setting catalog",
            )
            .at(actual_path),
        );
        return;
    };
    if !definition.path.contains("[*]") && !seen_paths.insert(definition.path.clone()) {
        diagnostics.push(
            Diagnostic::error(
                code::DUPLICATE_SETTING,
                "non-repeating SEP setting occurs more than once",
            )
            .at(actual_path),
        );
        return;
    }
    let canonical_model = model.and_then(resolve_profile).map(|profile| profile.id);
    let candidates = if model.is_some() {
        selected_variant(definition, canonical_model, protocol)
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        definition
            .variants
            .iter()
            .filter(|variant| {
                variant.selectors.is_empty()
                    || variant
                        .selectors
                        .iter()
                        .any(|selector| selector.protocol == protocol)
            })
            .collect::<Vec<_>>()
    };
    if candidates.is_empty() {
        diagnostics.push(
            Diagnostic::error(
                code::SETTING_NOT_APPLICABLE,
                "field is not applicable to the selected phone and protocol",
            )
            .at(actual_path),
        );
        return;
    }
    let mut best_errors = None;
    for variant in candidates {
        let value = raw_setting_value(raw, variant);
        let mut errors = Vec::new();
        validate_value(&value, variant, actual_path, &mut errors);
        if errors.is_empty() {
            return;
        }
        if best_errors
            .as_ref()
            .is_none_or(|current: &Vec<Diagnostic>| errors.len() < current.len())
        {
            best_errors = Some(errors);
        }
    }
    diagnostics.extend(
        best_errors
            .unwrap_or_default()
            .into_iter()
            .map(|mut diagnostic| {
                diagnostic.path = Some(actual_path.to_owned());
                diagnostic
            }),
    );
}

fn raw_setting_value(raw: &str, variant: &SettingVariant) -> SepSettingValue {
    if raw.is_empty() && variant.nullable {
        return SepSettingValue::Null;
    }
    if variant.multiple {
        return SepSettingValue::List(
            raw.split(',')
                .filter(|value| !value.trim().is_empty())
                .map(|value| raw_scalar_value(value.trim(), variant.value_kind))
                .collect(),
        );
    }
    raw_scalar_value(raw, variant.value_kind)
}

fn raw_scalar_value(raw: &str, kind: SettingValueKind) -> SepSettingValue {
    match kind {
        SettingValueKind::Boolean => match raw.to_ascii_lowercase().as_str() {
            "true" => SepSettingValue::Boolean(true),
            "false" => SepSettingValue::Boolean(false),
            _ => SepSettingValue::String(raw.to_owned()),
        },
        SettingValueKind::Integer => raw.parse::<i64>().map_or_else(
            |_| SepSettingValue::String(raw.to_owned()),
            SepSettingValue::Integer,
        ),
        SettingValueKind::String => SepSettingValue::String(raw.to_owned()),
    }
}

fn definition_for_actual_path(path: &str) -> Option<&'static SepSettingDefinition> {
    let stripped = strip_indexes(path);
    CATALOG
        .settings
        .iter()
        .find(|definition| strip_indexes(&definition.path) == stripped)
}

fn catalog_has_descendants(path: &str) -> bool {
    let stripped = strip_indexes(path);
    CATALOG.settings.iter().any(|definition| {
        let candidate = strip_indexes(&definition.path);
        candidate
            .strip_prefix(&stripped)
            .is_some_and(|suffix| suffix.starts_with('/'))
    })
}

fn strip_indexes(path: &str) -> String {
    path.split('/')
        .map(|segment| segment.split_once('[').map_or(segment, |(name, _)| name))
        .collect::<Vec<_>>()
        .join("/")
}

fn selected_variant<'a>(
    definition: &'a SepSettingDefinition,
    model: Option<&str>,
    protocol: Protocol,
) -> Option<&'a SettingVariant> {
    definition
        .variants
        .iter()
        .filter_map(|variant| {
            if variant.selectors.is_empty() {
                return Some((0_u8, variant));
            }
            let score = variant
                .selectors
                .iter()
                .filter(|selector| selector.protocol == protocol)
                .filter_map(|selector| {
                    if model == Some(selector.model.as_str()) {
                        Some(2)
                    } else if selector.model == "*" {
                        Some(1)
                    } else {
                        None
                    }
                })
                .max()?;
            Some((score, variant))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, variant)| variant)
}

fn validate_value(
    value: &SepSettingValue,
    variant: &SettingVariant,
    location: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if matches!(value, SepSettingValue::Null) {
        if !variant.nullable {
            diagnostics.push(
                Diagnostic::error(code::INVALID_SETTING, "setting may not be null")
                    .at(format!("{location}.value")),
            );
        }
        return;
    }
    if variant.multiple {
        let SepSettingValue::List(items) = value else {
            diagnostics.push(
                Diagnostic::error(code::INVALID_SETTING, "setting requires an array value")
                    .at(format!("{location}.value")),
            );
            return;
        };
        for item in items {
            validate_scalar(item, variant, location, diagnostics);
        }
    } else {
        validate_scalar(value, variant, location, diagnostics);
    }
}

fn validate_scalar(
    value: &SepSettingValue,
    variant: &SettingVariant,
    location: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind_matches = matches!(
        (variant.value_kind, value),
        (SettingValueKind::Boolean, SepSettingValue::Boolean(_))
            | (SettingValueKind::Integer, SepSettingValue::Integer(_))
            | (SettingValueKind::String, SepSettingValue::String(_))
    );
    if !kind_matches {
        diagnostics.push(
            Diagnostic::error(
                code::INVALID_SETTING,
                format!("setting requires a {} value", variant.value_kind),
            )
            .at(format!("{location}.value")),
        );
        return;
    }
    if !variant.allowed_values.is_empty()
        && !variant
            .allowed_values
            .iter()
            .any(|allowed| allowed.value == *value)
    {
        diagnostics.push(
            Diagnostic::error(
                code::INVALID_SETTING,
                "value is not one of the allowed choices",
            )
            .at(format!("{location}.value")),
        );
    }
    if let SepSettingValue::Integer(number) = value
        && (variant.minimum.is_some_and(|minimum| *number < minimum)
            || variant.maximum.is_some_and(|maximum| *number > maximum))
    {
        diagnostics.push(
            Diagnostic::error(
                code::INVALID_SETTING,
                "integer is outside the allowed range",
            )
            .at(format!("{location}.value")),
        );
    }
    if let SepSettingValue::String(text) = value {
        if variant
            .maximum_characters
            .is_some_and(|maximum| text.chars().count() > maximum)
        {
            diagnostics.push(
                Diagnostic::error(code::INVALID_SETTING, "string exceeds the maximum length")
                    .at(format!("{location}.value")),
            );
        }
        if let Some(pattern) = &variant.pattern
            && !PATTERNS
                .get(pattern)
                .expect("catalog patterns are present in the compiled pattern index")
                .is_match(text)
        {
            diagnostics.push(
                Diagnostic::error(
                    code::INVALID_SETTING,
                    "string does not match the required format",
                )
                .at(format!("{location}.value")),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_exposes_complete_and_model_specific_setting_sets() {
        assert_eq!(CATALOG.settings.len(), 481);
        assert_eq!(
            phone_options(&"8841".parse().unwrap(), Protocol::Sip)
                .settings
                .len(),
            391
        );
        assert_eq!(
            phone_options(&"8821".parse().unwrap(), Protocol::Sip)
                .settings
                .len(),
            385
        );
    }

    #[test]
    fn catalog_titles_are_ui_ready() {
        for (path, expected) in [
            (
                "/device/WiFiHotspotProfile/hsProfileLocked",
                "Wi-Fi Hotspot Profile Locked",
            ),
            ("/device/@ctiid", "CTI ID"),
            ("/device/sipProfile/sipStack/timerT1", "Timer T1"),
            (
                "/device/vendorConfig/AutoSaveVolDuringCall",
                "Automatically Save Volume During Call",
            ),
        ] {
            let definition = CATALOG
                .settings
                .iter()
                .find(|definition| definition.path == path)
                .expect("catalog path exists");
            assert_eq!(definition.title, expected);
        }
    }

    #[test]
    fn model_specific_constraints_reject_bad_values() {
        let settings = [SepSetting::new(
            "/device/vendorConfig/recordingToneLocalVolume"
                .parse()
                .expect("valid setting path"),
            SepSettingValue::Integer(101),
        )];
        let diagnostics =
            validate_phone_settings(&"CP-8841".parse().unwrap(), Protocol::Sip, &settings);
        assert!(diagnostics.iter().any(Diagnostic::is_error));
    }

    #[test]
    fn indexed_paths_normalize_to_catalog_wildcards() {
        let path = "/device/sipProfile/sipLines/line[3]/featureID"
            .parse::<crate::model::SepSettingPath>()
            .expect("valid indexed setting path");
        assert_eq!(
            path.normalized(),
            "/device/sipProfile/sipLines/line[*]/featureID"
        );
        assert!(
            "/device/vendorConfig/x><evil"
                .parse::<crate::model::SepSettingPath>()
                .is_err()
        );
    }

    #[test]
    fn xml_setting_validation_accepts_cdata_and_reports_invalid_entities() {
        let cdata = validate_xml_settings(
            "<device><deviceProtocol><![CDATA[SCCP]]></deviceProtocol></device>",
            None,
            Protocol::Sccp,
        );
        assert!(cdata.is_empty());

        let escaped_reference = validate_xml_settings(
            "<device><authenticationURL>https://example.test/?a=1&amp;b=2</authenticationURL></device>",
            None,
            Protocol::Sccp,
        );
        assert!(escaped_reference.is_empty());

        let invalid_entity = validate_xml_settings(
            "<device><deviceProtocol>&not-an-entity;</deviceProtocol></device>",
            None,
            Protocol::Sccp,
        );
        assert!(
            invalid_entity
                .iter()
                .any(|diagnostic| diagnostic.code == code::MALFORMED)
        );
    }

    #[test]
    fn repeated_indexes_are_distinct_but_duplicate_singletons_fail() {
        let model = "8841".parse().unwrap();
        let settings = [
            SepSetting::new(
                "/device/sipProfile/sipLines/line[1]/featureID"
                    .parse()
                    .expect("valid first repeated setting path"),
                SepSettingValue::Integer(9),
            ),
            SepSetting::new(
                "/device/sipProfile/sipLines/line[2]/featureID"
                    .parse()
                    .expect("valid second repeated setting path"),
                SepSettingValue::Integer(9),
            ),
        ];
        assert!(validate_phone_settings(&model, Protocol::Sip, &settings).is_empty());

        let diagnostics = validate_xml_settings(
            "<device><deviceProtocol>SIP</deviceProtocol><deviceProtocol>SIP</deviceProtocol></device>",
            Some(&model),
            Protocol::Sip,
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code::DUPLICATE_SETTING)
        );
    }

    #[test]
    fn unknown_escape_hatch_does_not_allow_known_inapplicable_settings() {
        let model = "7945".parse().unwrap();
        let known_sip_only = [SepSetting::new(
            "/device/sipOAuthMode"
                .parse()
                .expect("valid known setting path"),
            SepSettingValue::Integer(1),
        )];
        let diagnostics =
            validate_phone_settings_inner(&model, Protocol::Sccp, &known_sip_only, true);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code::SETTING_NOT_APPLICABLE)
        );

        let unknown = [SepSetting::new(
            "/device/futureField"
                .parse()
                .expect("valid unknown setting path"),
            SepSettingValue::String("value".to_owned()),
        )];
        assert!(validate_phone_settings_inner(&model, Protocol::Sccp, &unknown, true).is_empty());
    }

    #[test]
    fn known_unsupported_protocol_has_no_phone_options() {
        let options = phone_options(&"8841".parse().unwrap(), Protocol::Sccp);
        assert!(!options.supported);
        assert!(options.settings.is_empty());
    }
}
