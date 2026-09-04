//! Bundle assembly and offline dependency validation.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::artifact::{detect_artifact, parse_artifact};
use crate::diagnostic::{Diagnostic, code};
use crate::generation::{GenerationError, generate_defaults, generate_device};
use crate::model::{
    ArtifactDialect, ArtifactKind, BootstrapBundle, BundleFile, BundleInventory, BundleSpec,
    ExternalArtifact, InventoryEntry, InventorySource, Protocol, ProtocolSpec, SignalingMode,
};
use crate::validation::validate;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error("bundle would contain duplicate filename {0}")]
    DuplicateFilename(String),
    #[error("bundle input does not contain bootstrap-manifest.json")]
    MissingInventory,
    #[error("bundle input filename is not a canonical basename: {0}")]
    InvalidFilename(String),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not decode {path}: {source}")]
    Decode {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// Generate every configuration in a manifest and create an inventory for
/// files which must be supplied separately.
///
/// # Errors
///
/// Returns an error when a device/default cannot be generated or two outputs
/// resolve to the same canonical filename.
pub fn generate_bundle(spec: &BundleSpec) -> Result<BootstrapBundle, BundleError> {
    let mut artifacts = Vec::new();
    for device in &spec.devices {
        artifacts.push(generate_device(device)?);
    }
    for defaults in &spec.defaults {
        artifacts.extend(generate_defaults(defaults)?);
    }

    let mut names = HashSet::new();
    for artifact in &artifacts {
        if !names.insert(artifact.filename.clone()) {
            return Err(BundleError::DuplicateFilename(artifact.filename.clone()));
        }
    }

    let mut files = artifacts
        .iter()
        .map(|artifact| InventoryEntry {
            filename: artifact.filename.clone(),
            kind: artifact.kind,
            source: InventorySource::Generated,
            required: true,
            description: None,
        })
        .collect::<Vec<_>>();

    let mut external = spec.external_artifacts.clone();
    add_inferred_dependencies(spec, &mut external);
    for item in external {
        if names.insert(item.filename.clone()) {
            files.push(InventoryEntry {
                filename: item.filename,
                kind: item.kind,
                source: InventorySource::External,
                required: item.required,
                description: item.description,
            });
        }
    }
    files.sort_by(|left, right| left.filename.cmp(&right.filename));

    Ok(BootstrapBundle {
        artifacts,
        inventory: BundleInventory { files },
    })
}

/// Validate a generated bundle directory and all supported configuration files
/// named by its inventory.
///
/// # Errors
///
/// Returns an error when the inventory cannot be read or decoded. Problems in
/// files named by a valid inventory are returned as diagnostics instead.
pub fn validate_bundle(directory: &Path) -> Result<Vec<Diagnostic>, BundleError> {
    let inventory_path = directory.join("bootstrap-manifest.json");
    let source = fs::read_to_string(&inventory_path).map_err(|source| BundleError::Read {
        path: inventory_path.clone(),
        source,
    })?;
    let inventory: BundleInventory =
        serde_json::from_str(&source).map_err(|source| BundleError::Decode {
            path: inventory_path,
            source,
        })?;

    let mut files = vec![BundleFile {
        filename: "bootstrap-manifest.json".to_owned(),
        contents: Some(source),
    }];
    for entry in &inventory.files {
        if !is_safe_filename(&entry.filename) {
            return Err(BundleError::InvalidFilename(entry.filename.clone()));
        }
        let path = directory.join(&entry.filename);
        if path.is_file() {
            files.push(BundleFile {
                filename: entry.filename.clone(),
                contents: is_text_configuration(entry.kind, &entry.filename)
                    .then(|| fs::read_to_string(path).ok())
                    .flatten(),
            });
        }
    }
    validate_bundle_files(&files)
}

/// Validate a bundle represented entirely in memory. This is the canonical
/// implementation used by native filesystem adapters and WebAssembly hosts.
///
/// # Errors
///
/// Returns an error when filenames are unsafe or duplicated, or when the
/// inventory is missing or malformed. Artifact problems are diagnostics.
pub fn validate_bundle_files(files: &[BundleFile]) -> Result<Vec<Diagnostic>, BundleError> {
    let mut by_name = HashMap::new();
    for file in files {
        if !is_safe_filename(&file.filename) {
            return Err(BundleError::InvalidFilename(file.filename.clone()));
        }
        if by_name.insert(file.filename.as_str(), file).is_some() {
            return Err(BundleError::DuplicateFilename(file.filename.clone()));
        }
    }
    let inventory_file = by_name
        .get("bootstrap-manifest.json")
        .ok_or(BundleError::MissingInventory)?;
    let inventory_source = inventory_file
        .contents
        .as_deref()
        .ok_or(BundleError::MissingInventory)?;
    let inventory: BundleInventory =
        serde_json::from_str(inventory_source).map_err(|source| BundleError::Decode {
            path: PathBuf::from("bootstrap-manifest.json"),
            source,
        })?;

    let mut diagnostics = Vec::new();
    for entry in &inventory.files {
        if !is_safe_filename(&entry.filename) {
            diagnostics.push(
                Diagnostic::error(
                    code::MALFORMED,
                    "inventory filename must be a canonical basename",
                )
                .at(&entry.filename),
            );
            continue;
        }
        let Some(file) = by_name.get(entry.filename.as_str()) else {
            if entry.required {
                diagnostics.push(
                    Diagnostic::error(
                        code::MISSING_EXTERNAL_ARTIFACT,
                        format!("required bundle artifact {} is missing", entry.filename),
                    )
                    .at(&entry.filename),
                );
            }
            continue;
        };

        if !is_text_configuration(entry.kind, &entry.filename) {
            continue;
        }
        let Some(contents) = file.contents.as_deref() else {
            diagnostics.push(
                Diagnostic::error(code::MALFORMED, "configuration cannot be read as text")
                    .at(&entry.filename),
            );
            continue;
        };
        match parse_artifact(contents, Some(&entry.filename)) {
            Ok(parsed) => {
                diagnostics.extend(validate(&parsed, None).into_iter().map(|mut diagnostic| {
                    let inner = diagnostic.path.take();
                    diagnostic.path = Some(inner.map_or_else(
                        || entry.filename.clone(),
                        |inner| format!("{}:{inner}", entry.filename),
                    ));
                    diagnostic
                }));
            }
            Err(error) => diagnostics
                .push(Diagnostic::error(code::MALFORMED, error.to_string()).at(&entry.filename)),
        }
    }
    Ok(diagnostics)
}

fn is_safe_filename(filename: &str) -> bool {
    let mut components = Path::new(filename).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn is_text_configuration(kind: ArtifactKind, filename: &str) -> bool {
    matches!(
        kind,
        ArtifactKind::DeviceConfiguration | ArtifactKind::DefaultConfiguration
    ) && !matches!(
        detect_artifact("", Some(filename)),
        Ok(detection)
            if matches!(
                detection.dialect,
                ArtifactDialect::CompiledBinary
                    | ArtifactDialect::SignedXml
                    | ArtifactDialect::EncryptedXml
            )
    )
}

fn add_inferred_dependencies(spec: &BundleSpec, dependencies: &mut Vec<ExternalArtifact>) {
    for device in &spec.devices {
        if let Some(firmware) = &device.firmware {
            dependencies.push(load_dependency(firmware));
        }
        let enterprise_sip = matches!(device.protocol, ProtocolSpec::Sip(_))
            && crate::catalog::resolve_profile(&device.model)
                .is_none_or(|profile| !profile.supports_dialect(ArtifactDialect::LegacySipText));
        if enterprise_sip {
            dependencies.push(ExternalArtifact {
                filename: "dialplan.xml".to_owned(),
                kind: ArtifactKind::DialPlan,
                required: true,
                description: Some("SIP digit-analysis template referenced by SEP XML".to_owned()),
            });
        }
        let secure = match &device.protocol {
            ProtocolSpec::Sccp(settings) => settings.signaling != SignalingMode::NonSecure,
            ProtocolSpec::Sip(settings) => settings.signaling != SignalingMode::NonSecure,
        };
        if secure {
            dependencies.push(ExternalArtifact {
                filename: format!("CTL{}.tlv", device.mac.sep_name()),
                kind: ArtifactKind::TrustList,
                required: true,
                description: Some(
                    "device trust list must be supplied by the deployment security workflow"
                        .to_owned(),
                ),
            });
        }
    }
    for defaults in &spec.defaults {
        if let Some(firmware) = &defaults.firmware {
            dependencies.push(load_dependency(firmware));
        }
        for model_load in &defaults.model_loads {
            dependencies.push(load_dependency(&model_load.firmware));
        }
        let needs_compiled_sccp_default = defaults.protocol == Protocol::Sccp
            && defaults.model_loads.iter().any(|model_load| {
                crate::catalog::resolve_profile(&model_load.model)
                    .is_some_and(|profile| profile.supports_dialect(ArtifactDialect::LegacySipText))
            });
        if needs_compiled_sccp_default {
            dependencies.push(ExternalArtifact {
                filename: "SEPDefault.cnf".to_owned(),
                kind: ArtifactKind::DefaultConfiguration,
                required: true,
                description: Some(
                    "compiled SCCP default for the legacy 7940/7960 family must be supplied separately"
                        .to_owned(),
                ),
            });
        }
    }
}

fn load_dependency(firmware: &str) -> ExternalArtifact {
    let filename = if firmware.to_ascii_lowercase().ends_with(".loads") {
        firmware.to_owned()
    } else {
        format!("{firmware}.loads")
    };
    ExternalArtifact {
        filename,
        kind: ArtifactKind::LoadDescriptor,
        required: true,
        description: Some(
            "Cisco load descriptor and its payloads must be supplied separately".to_owned(),
        ),
    }
}
