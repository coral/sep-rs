//! Artifact detection and parsing.

use std::path::Path;

use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::legacy::{LegacyParseError, LegacySipConfig, parse_legacy};
use crate::model::{ArtifactDialect, ArtifactKind};
use crate::xml::{DefaultDocument, DeviceDocument};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDetection {
    pub dialect: ArtifactDialect,
    pub kind: ArtifactKind,
}

#[derive(Clone, Debug)]
pub struct ParsedArtifact {
    detection: ArtifactDetection,
    source: String,
    filename_hint: Option<String>,
    ignored_paths: Vec<String>,
    pub(crate) document: ParsedDocument,
}

impl ParsedArtifact {
    #[must_use]
    pub const fn dialect(&self) -> ArtifactDialect {
        self.detection.dialect
    }

    #[must_use]
    pub const fn kind(&self) -> ArtifactKind {
        self.detection.kind
    }

    #[must_use]
    pub fn original_source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn filename_hint(&self) -> Option<&str> {
        self.filename_hint.as_deref()
    }

    #[must_use]
    pub fn ignored_paths(&self) -> &[String] {
        &self.ignored_paths
    }

    #[must_use]
    pub fn protocol_name(&self) -> Option<&str> {
        match &self.document {
            ParsedDocument::Device(document) => document.device_protocol.as_deref(),
            ParsedDocument::LegacySip(_) => Some("SIP"),
            ParsedDocument::Default(_) => None,
        }
    }

    #[must_use]
    pub(crate) const fn document(&self) -> &ParsedDocument {
        &self.document
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ParsedDocument {
    Device(Box<DeviceDocument>),
    Default(Box<DefaultDocument>),
    LegacySip(LegacySipConfig),
}

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact is empty")]
    Empty,
    #[error("could not recognize the artifact format")]
    UnknownFormat,
    #[error("unsupported XML root element <{0}>")]
    UnsupportedXmlRoot(String),
    #[error("{dialect:?} artifacts are recognized but cannot be parsed in this release")]
    Unsupported { dialect: ArtifactDialect },
    #[error("malformed XML: {0}")]
    XmlRead(#[from] quick_xml::Error),
    #[error("XML does not match the Cisco bootstrap structure: {0}")]
    XmlDecode(#[from] quick_xml::DeError),
    #[error(transparent)]
    Legacy(#[from] LegacyParseError),
}

/// Detect a Cisco bootstrap artifact from its contents and optional basename.
///
/// # Errors
///
/// Returns an error for empty input, malformed XML, or an unrecognized root or
/// text format.
pub fn detect_artifact(
    source: &str,
    filename_hint: Option<&str>,
) -> Result<ArtifactDetection, ArtifactError> {
    let filename = filename_hint
        .and_then(|hint| Path::new(hint).file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let filename_lower = filename.to_ascii_lowercase();

    let extension_is_sgn = Path::new(&filename_lower)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sgn"));
    if extension_is_sgn {
        return Ok(ArtifactDetection {
            dialect: if filename_lower.ends_with(".enc.sgn") {
                ArtifactDialect::EncryptedXml
            } else {
                ArtifactDialect::SignedXml
            },
            kind: infer_kind(filename),
        });
    }
    if filename_lower == "sepdefault.cnf" || is_compiled_legacy_filename(&filename_lower) {
        return Ok(ArtifactDetection {
            dialect: ArtifactDialect::CompiledBinary,
            kind: infer_kind(filename),
        });
    }
    if filename_lower.contains("dialplan") && source.contains("<DIALTEMPLATE") {
        return Ok(ArtifactDetection {
            dialect: ArtifactDialect::EnterpriseXml,
            kind: ArtifactKind::DialPlan,
        });
    }

    let source = source.trim_start_matches('\u{feff}').trim_start();
    if source.is_empty() {
        return Err(ArtifactError::Empty);
    }
    if source.starts_with('<') {
        let root = xml_root(source)?;
        return match root.as_str() {
            "device" => Ok(ArtifactDetection {
                dialect: ArtifactDialect::EnterpriseXml,
                kind: ArtifactKind::DeviceConfiguration,
            }),
            "Default" => Ok(ArtifactDetection {
                dialect: ArtifactDialect::EnterpriseXml,
                kind: ArtifactKind::DefaultConfiguration,
            }),
            "flat-profile" | "flatProfile" => Ok(ArtifactDetection {
                dialect: ArtifactDialect::Mpp3pcc,
                kind: ArtifactKind::DeviceConfiguration,
            }),
            _ => Err(ArtifactError::UnsupportedXmlRoot(root)),
        };
    }

    if filename_lower == "sipdefault.cnf"
        || (filename_lower.starts_with("sip")
            && Path::new(&filename_lower)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("cnf")))
        || looks_like_legacy_sip(source)
    {
        return Ok(ArtifactDetection {
            dialect: ArtifactDialect::LegacySipText,
            kind: if filename_lower == "sipdefault.cnf" {
                ArtifactKind::DefaultConfiguration
            } else {
                ArtifactKind::DeviceConfiguration
            },
        });
    }

    Err(ArtifactError::UnknownFormat)
}

/// Parse a supported artifact while retaining its exact text and all paths not
/// covered by the typed XML subset.
///
/// # Errors
///
/// Returns an error when detection fails, a supported syntax is malformed, or
/// the artifact belongs to a recognized but unsupported dialect.
pub fn parse_artifact(
    source: &str,
    filename_hint: Option<&str>,
) -> Result<ParsedArtifact, ArtifactError> {
    let detection = detect_artifact(source, filename_hint)?;
    let mut ignored_paths = Vec::new();
    let document = match (detection.dialect, detection.kind) {
        (ArtifactDialect::EnterpriseXml, ArtifactKind::DeviceConfiguration) => {
            let mut deserializer = quick_xml::de::Deserializer::from_str(source);
            let document = serde_ignored::deserialize(&mut deserializer, |path| {
                ignored_paths.push(path.to_string());
            })?;
            ParsedDocument::Device(Box::new(document))
        }
        (ArtifactDialect::EnterpriseXml, ArtifactKind::DefaultConfiguration) => {
            let mut deserializer = quick_xml::de::Deserializer::from_str(source);
            let document = serde_ignored::deserialize(&mut deserializer, |path| {
                ignored_paths.push(path.to_string());
            })?;
            ParsedDocument::Default(Box::new(document))
        }
        (ArtifactDialect::LegacySipText, _) => ParsedDocument::LegacySip(parse_legacy(source)?),
        (dialect, _) => return Err(ArtifactError::Unsupported { dialect }),
    };

    Ok(ParsedArtifact {
        detection,
        source: source.to_owned(),
        filename_hint: filename_hint.map(ToOwned::to_owned),
        ignored_paths,
        document,
    })
}

fn xml_root(source: &str) -> Result<String, ArtifactError> {
    let mut reader = quick_xml::Reader::from_str(source);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event()? {
            Event::Start(element) | Event::Empty(element) => {
                return Ok(element.local_name().as_ref().to_owned());
            }
            Event::Eof => return Err(ArtifactError::Empty),
            _ => {}
        }
    }
}

fn is_compiled_legacy_filename(filename: &str) -> bool {
    (filename.starts_with("ld") || filename.starts_with("gk"))
        && !filename.contains('.')
        && filename.len() > 2
}

fn infer_kind(filename: &str) -> ArtifactKind {
    let lower = filename.to_ascii_lowercase();
    if lower.contains("default") {
        ArtifactKind::DefaultConfiguration
    } else if lower.starts_with("sep") || lower.starts_with("sip") {
        ArtifactKind::DeviceConfiguration
    } else {
        ArtifactKind::Other
    }
}

fn looks_like_legacy_sip(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim();
        !line.starts_with('#')
            && !line.starts_with(';')
            && ["proxy1_address", "line1_name", "image_version"]
                .iter()
                .any(|key| {
                    line.strip_prefix(key)
                        .is_some_and(|rest| rest.trim_start().starts_with([':', '=']))
                })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_roots_and_legacy_names() {
        assert_eq!(
            detect_artifact("<?xml version=\"1.0\"?><device/>", None)
                .expect("device should be detected")
                .dialect,
            ArtifactDialect::EnterpriseXml
        );
        assert_eq!(
            detect_artifact("<Default/>", Some("XMLDefault.cnf.xml"))
                .expect("default should be detected")
                .kind,
            ArtifactKind::DefaultConfiguration
        );
        assert_eq!(
            detect_artifact("line1_name: \"1001\"", Some("SIP001122334455.cnf"))
                .expect("legacy SIP should be detected")
                .dialect,
            ArtifactDialect::LegacySipText
        );
        let sep_default = detect_artifact("\u{1}\u{2}\u{1}\u{ff}", Some("SEPDefault.cnf"))
            .expect("compiled SCCP default should be detected");
        assert_eq!(sep_default.dialect, ArtifactDialect::CompiledBinary);
        assert_eq!(sep_default.kind, ArtifactKind::DefaultConfiguration);
    }

    #[test]
    fn reports_unknown_xml_and_retains_source() {
        let source = concat!(
            "<device>",
            "<deviceProtocol>SCCP</deviceProtocol>",
            "<devicePool><callManagerGroup><members/></callManagerGroup></devicePool>",
            "<futureCiscoField>true</futureCiscoField>",
            "</device>",
        );
        let parsed =
            parse_artifact(source, Some("SEP001122334455.cnf.xml")).expect("device should parse");

        assert_eq!(parsed.original_source(), source);
        assert!(
            parsed
                .ignored_paths()
                .iter()
                .any(|path| path.contains("futureCiscoField"))
        );
    }
}
