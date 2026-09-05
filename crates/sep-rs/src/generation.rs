//! Deterministic configuration generation from protocol-independent models.

use std::collections::{BTreeMap, HashSet};

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};
use thiserror::Error;

use crate::artifact::parse_artifact;
use crate::catalog::resolve_profile;
use crate::diagnostic::{Diagnostic, code};
use crate::legacy::{LegacySipConfig, assignment, section};
use crate::model::{
    ArtifactDialect, ArtifactKind, CallControlEndpoint, DefaultSpec, DeviceSpec, GeneratedArtifact,
    Protocol, ProtocolSpec, SignalingMode, SipButton, SipButtonFeature, SipLine, SipSpec,
    Transport,
};
use crate::settings::{
    setting_contains_secret, validate_phone_settings, validate_phone_settings_inner,
};
use crate::validation::validate;
use crate::xml::{
    CallManager, CallManagerGroup, CallManagerMember, CallManagerMembers, CallManagerPorts,
    CommonProfile, DateTimeSetting, DefaultDocument, DeviceDocument, DevicePool, LoadInformation,
    NtpServer, NtpServers, SipLine as XmlSipLine, SipLines, SipProfile, SipProxies, SipStack,
    UserLocale, VendorConfig,
};

const XML_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error("at least one call-control endpoint is required")]
    MissingEndpoints,
    #[error("SIP button {position} references missing line {line_index}")]
    MissingSipLine { position: u8, line_index: u8 },
    #[error("legacy SIP cannot represent button feature at position {position}")]
    UnsupportedLegacyButton { position: u8 },
    #[error("invalid generation specification: {0}")]
    InvalidSpec(String),
    #[error("could not serialize XML: {0}")]
    Xml(#[from] quick_xml::SeError),
}

/// Generate the canonical per-device bootstrap artifact for a phone.
///
/// # Errors
///
/// Returns an error for structurally invalid input, an unrepresentable legacy
/// feature, or XML serialization failure. Unknown or profile-incompatible
/// models use enterprise XML and add warnings to the generated artifact.
pub fn generate_device(spec: &DeviceSpec) -> Result<GeneratedArtifact, GenerationError> {
    validate_device_spec(spec)?;
    let profile = resolve_profile(&spec.model);
    let warnings = profile_warnings(&spec.model, spec.protocol.protocol(), profile);

    if spec.protocol.protocol() == Protocol::Sip
        && profile.is_some_and(|profile| profile.supports_dialect(ArtifactDialect::LegacySipText))
    {
        let mut artifact = generate_legacy_device(spec)?;
        artifact.warnings.extend(warnings);
        return Ok(artifact);
    }
    let mut artifact = generate_enterprise_device(spec)?;
    artifact.warnings.extend(warnings);
    Ok(artifact)
}

/// Generate all defaults files applicable to a semantic defaults input.
/// Enterprise XML is always produced; SIP inputs that include a legacy 7940/
/// 7960 load also receive `SIPDefault.cnf`.
///
/// # Errors
///
/// Returns an error for missing endpoints or XML serialization failure.
/// Unknown and profile-incompatible model loads add warnings; an unlisted load
/// can be emitted when its numeric model ID is supplied.
pub fn generate_defaults(spec: &DefaultSpec) -> Result<Vec<GeneratedArtifact>, GenerationError> {
    if spec.endpoints.is_empty() {
        return Err(GenerationError::MissingEndpoints);
    }

    let mut artifacts = vec![generate_xml_default(spec)?];
    let needs_legacy = spec.protocol == Protocol::Sip
        && (spec.firmware.is_some()
            || spec.model_loads.iter().any(|model_load| {
                resolve_profile(&model_load.model)
                    .is_some_and(|profile| profile.supports_dialect(ArtifactDialect::LegacySipText))
            }));
    if needs_legacy {
        artifacts.push(generate_legacy_default(spec));
    }
    Ok(artifacts)
}

fn profile_warnings(
    model: &crate::model::PhoneModelId,
    protocol: Protocol,
    profile: Option<&crate::catalog::PhoneProfile>,
) -> Vec<Diagnostic> {
    let Some(profile) = profile else {
        return vec![
            Diagnostic::warning(
                code::UNKNOWN_MODEL,
                format!(
                    "no built-in profile for model {model}; continuing with generic enterprise assumptions"
                ),
            )
            .at("model"),
        ];
    };
    if profile.supports_protocol(protocol) {
        Vec::new()
    } else {
        vec![
            Diagnostic::warning(
                code::MODEL_PROTOCOL_MISMATCH,
                format!(
                    "{} is not known to support {protocol}; configuration was generated anyway",
                    profile.id
                ),
            )
            .at("protocol"),
        ]
    }
}

fn generate_enterprise_device(spec: &DeviceSpec) -> Result<GeneratedArtifact, GenerationError> {
    let protocol = spec.protocol.protocol();
    let (security_mode, transport_layer_protocol) =
        signaling_wire_values(&spec.protocol, spec.endpoints[0].transport);
    let document = DeviceDocument {
        device_protocol: Some(protocol.to_string()),
        device_pool: DevicePool {
            date_time_setting: date_time_setting(
                spec.date_template.as_deref(),
                spec.time_zone.as_deref(),
                spec.ntp_server.as_ref().map(ToString::to_string),
            ),
            call_manager_group: call_manager_group(&spec.endpoints, protocol),
            connection_monitor_duration: Some(120),
            ..DevicePool::default()
        },
        sip_profile: match &spec.protocol {
            ProtocolSpec::Sccp(_) => None,
            ProtocolSpec::Sip(sip) => Some(sip_profile(spec, sip)?),
        },
        common_profile: (protocol == Protocol::Sip).then_some(CommonProfile {
            phone_password: None,
        }),
        load_information: spec.firmware.clone(),
        vendor_config: Some(VendorConfig {
            disable_speaker: Some(false),
            disable_speaker_and_headset: Some(false),
            settings_access: Some(1),
            web_access: Some(0),
            ssh_access: (protocol == Protocol::Sip).then_some(0),
            ..VendorConfig::default()
        }),
        network_locale: spec.locale.clone(),
        user_locale: spec.locale.as_ref().map(|locale| UserLocale {
            name: Some(locale.clone()),
            ..UserLocale::default()
        }),
        device_security_mode: Some(security_mode),
        authentication_url: None,
        directory_url: spec.services.directory.clone(),
        idle_url: spec.services.idle.clone(),
        information_url: spec.services.information.clone(),
        messages_url: spec.services.messages.clone(),
        services_url: spec.services.services.clone(),
        transport_layer_protocol,
        capf_auth_mode: Some(0),
        encr_config: Some(security_mode == 3),
        ..DeviceDocument::default()
    };
    let contents = apply_xml_settings(&serialize_xml(&document)?, &spec.settings)?;
    if !spec.settings.is_empty() {
        validate_generated_device(spec, &contents)?;
    }
    let contains_secrets = matches!(&spec.protocol, ProtocolSpec::Sip(sip) if sip.lines.iter().any(|line| line.auth_secret.is_some()))
        || spec
            .settings
            .iter()
            .any(|setting| setting_contains_secret(&spec.model, protocol, setting));
    let warnings = if spec.allow_unknown_settings {
        validate_phone_settings(&spec.model, protocol, &spec.settings)
            .into_iter()
            .filter(|diagnostic| diagnostic.code == code::UNKNOWN_FIELD)
            .map(|diagnostic| {
                let mut warning = Diagnostic::warning(diagnostic.code, diagnostic.message);
                warning.path = diagnostic.path;
                warning
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(GeneratedArtifact {
        filename: spec.mac.sep_filename(),
        kind: ArtifactKind::DeviceConfiguration,
        dialect: ArtifactDialect::EnterpriseXml,
        contents,
        contains_secrets,
        warnings,
    })
}

fn validate_generated_device(spec: &DeviceSpec, contents: &str) -> Result<(), GenerationError> {
    let filename = spec.mac.sep_filename();
    let artifact = parse_artifact(contents, Some(&filename)).map_err(|error| {
        GenerationError::InvalidSpec(format!("generated XML could not be parsed: {error}"))
    })?;
    let model_hint = resolve_profile(&spec.model).map(|_| &spec.model);
    let errors = validate(&artifact, model_hint)
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.is_error()
                && !(spec.allow_unknown_settings && diagnostic.code == code::UNKNOWN_FIELD)
        })
        .map(|diagnostic| diagnostic.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(GenerationError::InvalidSpec(errors.join("; ")))
    }
}

fn sip_profile(spec: &DeviceSpec, sip: &SipSpec) -> Result<SipProfile, GenerationError> {
    let primary = &spec.endpoints[0];
    let mut buttons = if sip.buttons.is_empty() {
        sip.lines
            .iter()
            .map(|line| SipButton {
                position: line.index,
                feature: SipButtonFeature::Line {
                    line_index: line.index,
                },
            })
            .collect::<Vec<_>>()
    } else {
        sip.buttons.clone()
    };
    buttons.sort_by_key(|button| button.position);
    let lines = sip
        .lines
        .iter()
        .map(|line| (line.index, line))
        .collect::<BTreeMap<_, _>>();
    let wire_lines = buttons
        .iter()
        .map(|button| sip_button(button, &lines, primary))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SipProfile {
        sip_proxies: SipProxies {
            outbound_proxy: sip.outbound_proxy.as_ref().map(ToString::to_string),
            outbound_proxy_port: sip.outbound_proxy.as_ref().map(|_| primary.port),
            register_with_proxy: Some(true),
            ..SipProxies::default()
        },
        sip_stack: SipStack {
            sip_invite_retx: Some(6),
            sip_retx: Some(10),
            timer_invite_expires: sip.timers.invite_expires_seconds.or(Some(180)),
            timer_register_expires: sip.timers.register_expires_seconds.or(Some(3600)),
            timer_keep_alive_expires: sip.timers.keepalive_seconds.or(Some(120)),
            timer_t1: Some(500),
            timer_t2: Some(4000),
            ..SipStack::default()
        },
        phone_label: spec.phone_label.clone(),
        start_media_port: sip.media_ports.map(|range| range.start).or(Some(16_384)),
        stop_media_port: sip.media_ports.map(|range| range.end).or(Some(32_766)),
        organization_top_level_domain: Some(primary.host.to_string()),
        sip_lines: SipLines { line: wire_lines },
        voip_control_port: Some(primary.port),
        dial_template: Some("dialplan.xml".to_owned()),
    })
}

fn sip_button(
    button: &SipButton,
    lines: &BTreeMap<u8, &SipLine>,
    primary: &CallControlEndpoint,
) -> Result<XmlSipLine, GenerationError> {
    let mut wire = XmlSipLine {
        button: button.position,
        ..XmlSipLine::default()
    };
    match &button.feature {
        SipButtonFeature::Line { line_index } => {
            let line = lines
                .get(line_index)
                .ok_or(GenerationError::MissingSipLine {
                    position: button.position,
                    line_index: *line_index,
                })?;
            wire.feature_id = 9;
            wire.feature_label.clone_from(&line.display_name);
            wire.proxy = Some(primary.host.to_string());
            wire.port = Some(primary.port);
            wire.name = Some(line.directory_number.clone());
            wire.display_name = line
                .display_name
                .clone()
                .or_else(|| Some(line.directory_number.clone()));
            wire.auth_name = line
                .auth_name
                .clone()
                .or_else(|| Some(line.directory_number.clone()));
            wire.auth_password = line
                .auth_secret
                .as_ref()
                .map(|secret| secret.expose_secret().to_owned());
            wire.contact = Some(line.directory_number.clone());
            wire.line_index = Some(*line_index);
        }
        SipButtonFeature::SpeedDial { label, target } => {
            wire.feature_id = 2;
            wire.feature_label = Some(label.clone());
            wire.speed_dial_number = Some(target.clone());
        }
        SipButtonFeature::ServiceUri { label, uri } => {
            wire.feature_id = 20;
            wire.feature_label = Some(label.clone());
            wire.service_uri = Some(uri.clone());
        }
        SipButtonFeature::Blf { label, target } => {
            wire.feature_id = 21;
            wire.feature_label = Some(label.clone());
            wire.speed_dial_number = Some(target.clone());
            wire.feature_option_mask = Some(1);
        }
        SipButtonFeature::Intercom { line_index } => {
            let line = lines
                .get(line_index)
                .ok_or(GenerationError::MissingSipLine {
                    position: button.position,
                    line_index: *line_index,
                })?;
            wire.feature_id = 23;
            wire.feature_label.clone_from(&line.display_name);
            wire.proxy = Some(primary.host.to_string());
            wire.port = Some(primary.port);
            wire.name = Some(line.directory_number.clone());
            wire.line_index = Some(*line_index);
        }
        SipButtonFeature::Raw {
            feature_id,
            label,
            target,
        } => {
            wire.feature_id = *feature_id;
            wire.feature_label.clone_from(label);
            wire.speed_dial_number.clone_from(target);
        }
    }
    Ok(wire)
}

fn generate_legacy_device(spec: &DeviceSpec) -> Result<GeneratedArtifact, GenerationError> {
    let ProtocolSpec::Sip(sip) = &spec.protocol else {
        unreachable!("legacy device generation is selected only for SIP")
    };
    validate_legacy_buttons(sip)?;

    let mut entries = Vec::new();
    section(&mut entries, "Call control");
    for (index, endpoint) in spec.endpoints.iter().enumerate() {
        let number = index + 1;
        entries.push(assignment(
            format!("proxy{number}_address"),
            endpoint.host.to_string(),
        ));
        entries.push(assignment(
            format!("proxy{number}_port"),
            endpoint.port.to_string(),
        ));
    }

    section(&mut entries, "Lines");
    let mut lines = sip.lines.iter().collect::<Vec<_>>();
    lines.sort_by_key(|line| line.index);
    for line in lines {
        let prefix = format!("line{}", line.index);
        entries.push(assignment(
            format!("{prefix}_name"),
            line.directory_number.clone(),
        ));
        entries.push(assignment(
            format!("{prefix}_shortname"),
            line.display_name
                .clone()
                .unwrap_or_else(|| line.directory_number.clone()),
        ));
        entries.push(assignment(
            format!("{prefix}_displayname"),
            line.display_name
                .clone()
                .unwrap_or_else(|| line.directory_number.clone()),
        ));
        entries.push(assignment(
            format!("{prefix}_authname"),
            line.auth_name
                .clone()
                .unwrap_or_else(|| line.directory_number.clone()),
        ));
        entries.push(assignment(
            format!("{prefix}_password"),
            line.auth_secret
                .as_ref()
                .map_or_else(String::new, |secret| secret.expose_secret().to_owned()),
        ));
    }

    section(&mut entries, "Media and registration");
    let primary = &spec.endpoints[0];
    entries.push(assignment("proxy_register", "1"));
    entries.push(assignment(
        "timer_register_expires",
        sip.timers
            .register_expires_seconds
            .unwrap_or(3600)
            .to_string(),
    ));
    entries.push(assignment("voip_control_port", primary.port.to_string()));
    let media = sip.media_ports.unwrap_or(crate::model::MediaPortRange {
        start: 16_384,
        end: 32_766,
    });
    entries.push(assignment("start_media_port", media.start.to_string()));
    entries.push(assignment("end_media_port", media.end.to_string()));
    if let Some(outbound_proxy) = &sip.outbound_proxy {
        entries.push(assignment("outbound_proxy", outbound_proxy.to_string()));
        entries.push(assignment("outbound_proxy_port", primary.port.to_string()));
    }

    section(&mut entries, "Phone");
    if let Some(label) = &spec.phone_label {
        entries.push(assignment("phone_label", label.clone()));
    }
    if let Some(time_zone) = &spec.time_zone {
        entries.push(assignment("time_zone", time_zone.clone()));
    }
    if let Some(ntp) = &spec.ntp_server {
        entries.push(assignment("sntp_server", ntp.to_string()));
        entries.push(assignment("sntp_mode", "unicast"));
    }
    if let Some(firmware) = &spec.firmware {
        entries.push(assignment("image_version", firmware.clone()));
    }
    if let Some(url) = &spec.services.services {
        entries.push(assignment("services_url", url.clone()));
    }
    if let Some(url) = &spec.services.directory {
        entries.push(assignment("directory_url", url.clone()));
    }

    Ok(GeneratedArtifact {
        filename: spec.mac.legacy_sip_filename(),
        kind: ArtifactKind::DeviceConfiguration,
        dialect: ArtifactDialect::LegacySipText,
        contents: LegacySipConfig { entries }.to_text(),
        contains_secrets: sip.lines.iter().any(|line| line.auth_secret.is_some()),
        warnings: Vec::new(),
    })
}

fn validate_legacy_buttons(sip: &SipSpec) -> Result<(), GenerationError> {
    if let Some(button) = sip
        .buttons
        .iter()
        .find(|button| !matches!(button.feature, SipButtonFeature::Line { .. }))
    {
        return Err(GenerationError::UnsupportedLegacyButton {
            position: button.position,
        });
    }
    Ok(())
}

fn generate_xml_default(spec: &DefaultSpec) -> Result<GeneratedArtifact, GenerationError> {
    let mut document = DefaultDocument {
        call_manager_group: call_manager_group(&spec.endpoints, spec.protocol),
        user_locale: spec.locale.as_ref().map(|locale| UserLocale {
            name: Some(locale.clone()),
            ..UserLocale::default()
        }),
        ..DefaultDocument::default()
    };
    let mut extra_loads = BTreeMap::new();
    let mut warnings = Vec::new();

    for model_load in &spec.model_loads {
        let profile = resolve_profile(&model_load.model);
        warnings.extend(profile_warnings(&model_load.model, spec.protocol, profile));
        let inferred_model_id = model_load.model.as_str().parse::<u16>().ok();
        let Some(model_id) = model_load
            .model_id
            .or_else(|| profile.map(|profile| profile.model_id))
            .or(inferred_model_id)
        else {
            warnings.push(
                Diagnostic::warning(
                    code::UNKNOWN_MODEL,
                    format!(
                        "default load for {} was omitted because it has no numeric model ID",
                        model_load.model
                    ),
                )
                .at("model_loads"),
            );
            continue;
        };
        if let Some(profile) = profile
            && model_load
                .model_id
                .is_some_and(|explicit| explicit != profile.model_id)
        {
            warnings.push(
                Diagnostic::warning(
                    code::UNKNOWN_MODEL,
                    format!(
                        "explicit model ID {model_id} overrides the known ID {} for {}",
                        profile.model_id, profile.id
                    ),
                )
                .at("model_loads.model_id"),
            );
        }
        let load = LoadInformation {
            model: Some(profile.map_or_else(
                || model_load.model.to_string(),
                |profile| profile.display_name.to_owned(),
            )),
            firmware: model_load.firmware.clone(),
        };
        if let Err(load) = document.set_load(model_id, load) {
            extra_loads.insert(model_id, load);
        }
    }

    Ok(GeneratedArtifact {
        filename: "XMLDefault.cnf.xml".to_owned(),
        kind: ArtifactKind::DefaultConfiguration,
        dialect: ArtifactDialect::EnterpriseXml,
        contents: serialize_xml_default(&document, &extra_loads)?,
        contains_secrets: false,
        warnings,
    })
}

fn generate_legacy_default(spec: &DefaultSpec) -> GeneratedArtifact {
    let mut entries = Vec::new();
    section(&mut entries, "Call control defaults");
    for (index, endpoint) in spec.endpoints.iter().enumerate() {
        let number = index + 1;
        entries.push(assignment(
            format!("proxy{number}_address"),
            endpoint.host.to_string(),
        ));
        entries.push(assignment(
            format!("proxy{number}_port"),
            endpoint.port.to_string(),
        ));
    }
    entries.push(assignment("proxy_register", "1"));
    entries.push(assignment("start_media_port", "16384"));
    entries.push(assignment("end_media_port", "32766"));
    let legacy_firmware = spec.firmware.as_ref().or_else(|| {
        spec.model_loads.iter().find_map(|model_load| {
            resolve_profile(&model_load.model)
                .is_some_and(|profile| profile.supports_dialect(ArtifactDialect::LegacySipText))
                .then_some(&model_load.firmware)
        })
    });
    if let Some(firmware) = legacy_firmware {
        entries.push(assignment("image_version", firmware.clone()));
    }
    if let Some(time_zone) = &spec.time_zone {
        entries.push(assignment("time_zone", time_zone.clone()));
    }
    if let Some(ntp) = &spec.ntp_server {
        entries.push(assignment("sntp_server", ntp.to_string()));
        entries.push(assignment("sntp_mode", "unicast"));
    }

    GeneratedArtifact {
        filename: "SIPDefault.cnf".to_owned(),
        kind: ArtifactKind::DefaultConfiguration,
        dialect: ArtifactDialect::LegacySipText,
        contents: LegacySipConfig { entries }.to_text(),
        contains_secrets: false,
        warnings: Vec::new(),
    }
}

fn call_manager_group(endpoints: &[CallControlEndpoint], protocol: Protocol) -> CallManagerGroup {
    let mut endpoints = endpoints.iter().collect::<Vec<_>>();
    endpoints.sort_by_key(|endpoint| endpoint.priority);
    CallManagerGroup {
        members: CallManagerMembers {
            member: endpoints
                .into_iter()
                .map(|endpoint| {
                    let ports = match protocol {
                        Protocol::Sccp => CallManagerPorts {
                            ethernet_phone_port: Some(endpoint.port),
                            ..CallManagerPorts::default()
                        },
                        Protocol::Sip => CallManagerPorts {
                            ethernet_phone_port: Some(2000),
                            sip_port: (endpoint.transport != Transport::Tls)
                                .then_some(endpoint.port),
                            secured_sip_port: (endpoint.transport == Transport::Tls)
                                .then_some(endpoint.port),
                        },
                    };
                    CallManagerMember {
                        priority: Some(endpoint.priority),
                        call_manager: CallManager {
                            name: Some(endpoint.host.to_string()),
                            ports,
                            process_node_name: endpoint.host.to_string(),
                        },
                    }
                })
                .collect(),
        },
    }
}

fn date_time_setting(
    date_template: Option<&str>,
    time_zone: Option<&str>,
    ntp_server: Option<String>,
) -> Option<DateTimeSetting> {
    if date_template.is_none() && time_zone.is_none() && ntp_server.is_none() {
        return None;
    }
    Some(DateTimeSetting {
        name: Some("CMLocal".to_owned()),
        date_template: date_template.map(ToOwned::to_owned),
        time_zone: time_zone.map(ToOwned::to_owned),
        ntps: ntp_server.map(|name| NtpServers {
            ntp: vec![NtpServer {
                name,
                ntp_mode: Some("Unicast".to_owned()),
            }],
        }),
    })
}

fn signaling_wire_values(protocol: &ProtocolSpec, transport: Transport) -> (u8, Option<u8>) {
    match protocol {
        ProtocolSpec::Sccp(sccp) => (
            match sccp.signaling {
                SignalingMode::NonSecure => 1,
                SignalingMode::Authenticated => 2,
                SignalingMode::Encrypted => 3,
            },
            None,
        ),
        ProtocolSpec::Sip(sip) => (
            match sip.signaling {
                SignalingMode::NonSecure => 1,
                SignalingMode::Authenticated => 2,
                SignalingMode::Encrypted => 3,
            },
            Some(match transport {
                Transport::Udp => 1,
                Transport::Tcp => 2,
                Transport::Tls => 3,
            }),
        ),
    }
}

fn validate_device_spec(spec: &DeviceSpec) -> Result<(), GenerationError> {
    if spec.endpoints.is_empty() {
        return Err(GenerationError::MissingEndpoints);
    }
    let mut priorities = HashSet::new();
    for endpoint in &spec.endpoints {
        if endpoint.port == 0 {
            return Err(GenerationError::InvalidSpec(
                "call-control ports must be in 1..=65535".to_owned(),
            ));
        }
        if !priorities.insert(endpoint.priority) {
            return Err(GenerationError::InvalidSpec(format!(
                "duplicate call-control priority {}",
                endpoint.priority
            )));
        }
    }

    let signaling = match &spec.protocol {
        ProtocolSpec::Sccp(settings) => settings.signaling,
        ProtocolSpec::Sip(settings) => settings.signaling,
    };
    let requires_tls = signaling != SignalingMode::NonSecure;
    if spec
        .endpoints
        .iter()
        .any(|endpoint| (endpoint.transport == Transport::Tls) != requires_tls)
    {
        return Err(GenerationError::InvalidSpec(
            "authenticated/encrypted signaling requires TLS endpoints, while nonsecure signaling must use TCP or UDP"
                .to_owned(),
        ));
    }
    if matches!(spec.protocol, ProtocolSpec::Sccp(_))
        && spec
            .endpoints
            .iter()
            .any(|endpoint| endpoint.transport == Transport::Udp)
    {
        return Err(GenerationError::InvalidSpec(
            "SCCP signaling does not use UDP".to_owned(),
        ));
    }
    if matches!(spec.protocol, ProtocolSpec::Sccp(_))
        && spec.endpoints.iter().any(|endpoint| {
            requires_tls && endpoint.port == 2000 || !requires_tls && endpoint.port == 2443
        })
    {
        return Err(GenerationError::InvalidSpec(
            "SCCP signaling mode contradicts the standard secure/nonsecure port".to_owned(),
        ));
    }
    if let Some(firmware) = spec.firmware.as_deref() {
        let upper = firmware.to_ascii_uppercase();
        let contradicts = match spec.protocol.protocol() {
            Protocol::Sccp => upper.starts_with("SIP") || upper.starts_with("P0S"),
            Protocol::Sip => upper.starts_with("SCCP") || upper.starts_with("P003"),
        };
        if contradicts {
            return Err(GenerationError::InvalidSpec(format!(
                "firmware load {firmware} contradicts {}",
                spec.protocol.protocol()
            )));
        }
    }

    if !spec.settings.is_empty() {
        if spec.protocol.protocol() == Protocol::Sip
            && resolve_profile(&spec.model)
                .is_some_and(|profile| profile.supports_dialect(ArtifactDialect::LegacySipText))
        {
            return Err(GenerationError::InvalidSpec(
                "advanced `/device` settings require enterprise XML and cannot be applied to legacy 7940/7960 SIP text"
                    .to_owned(),
            ));
        }
        let diagnostics = validate_phone_settings_inner(
            &spec.model,
            spec.protocol.protocol(),
            &spec.settings,
            spec.allow_unknown_settings,
        );
        let errors = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(GenerationError::InvalidSpec(errors.join("; ")));
        }
    }

    let ProtocolSpec::Sip(sip) = &spec.protocol else {
        return Ok(());
    };
    validate_sip_spec(sip)
}

fn validate_sip_spec(sip: &SipSpec) -> Result<(), GenerationError> {
    if sip.lines.is_empty() {
        return Err(GenerationError::InvalidSpec(
            "SIP generation requires at least one line".to_owned(),
        ));
    }
    if let Some(range) = sip.media_ports
        && (range.start == 0 || range.end == 0 || range.start > range.end)
    {
        return Err(GenerationError::InvalidSpec(
            "SIP media port range must be nonzero and ordered".to_owned(),
        ));
    }
    let mut line_indexes = HashSet::new();
    for line in &sip.lines {
        if line.index == 0 || !line_indexes.insert(line.index) {
            return Err(GenerationError::InvalidSpec(
                "SIP line indexes must be nonzero and unique".to_owned(),
            ));
        }
        if line.directory_number.trim().is_empty() {
            return Err(GenerationError::InvalidSpec(
                "SIP line directory numbers cannot be empty".to_owned(),
            ));
        }
    }
    let mut positions = HashSet::new();
    for button in &sip.buttons {
        if button.position == 0 || !positions.insert(button.position) {
            return Err(GenerationError::InvalidSpec(
                "SIP button positions must be nonzero and unique".to_owned(),
            ));
        }
    }
    Ok(())
}

fn serialize_xml<T: serde::Serialize>(document: &T) -> Result<String, GenerationError> {
    let mut output = String::from(XML_DECLARATION);
    output.push_str(&serialize_pretty(document)?);
    output.push('\n');
    Ok(output)
}

#[derive(Debug)]
struct XmlElement {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Self>,
    text: Option<String>,
}

impl XmlElement {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attributes: Vec::new(),
            children: Vec::new(),
            text: None,
        }
    }
}

fn apply_xml_settings(
    source: &str,
    settings: &[crate::model::SepSetting],
) -> Result<String, GenerationError> {
    if settings.is_empty() {
        return Ok(source.to_owned());
    }
    let mut root = parse_generated_xml(source)?;
    for setting in settings {
        apply_xml_setting(&mut root, setting)?;
    }
    write_generated_xml(&root)
}

fn parse_generated_xml(source: &str) -> Result<XmlElement, GenerationError> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<XmlElement>::new();
    let mut root = None;
    loop {
        match reader.read_event().map_err(|error| {
            GenerationError::InvalidSpec(format!("could not read generated XML: {error}"))
        })? {
            Event::Start(start) => stack.push(xml_element_from_start(&start)?),
            Event::Empty(start) => {
                let element = xml_element_from_start(&start)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(element);
                } else if root.replace(element).is_some() {
                    return Err(GenerationError::InvalidSpec(
                        "generated XML has more than one root element".to_owned(),
                    ));
                }
            }
            Event::Text(text) => {
                if let Some(element) = stack.last_mut() {
                    append_generated_text(element, &text.xml_content(XmlVersion::default()));
                }
            }
            Event::CData(text) => {
                if let Some(element) = stack.last_mut() {
                    append_generated_text(element, &text.xml_content(XmlVersion::default()));
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(element) = stack.last_mut() {
                    let encoded = format!("&{};", reference.as_ref());
                    let value = quick_xml::escape::unescape(&encoded).map_err(|error| {
                        GenerationError::InvalidSpec(format!(
                            "could not decode generated XML: {error}"
                        ))
                    })?;
                    append_generated_text(element, &value);
                }
            }
            Event::End(_) => {
                let element = stack.pop().ok_or_else(|| {
                    GenerationError::InvalidSpec(
                        "generated XML has an unmatched end element".to_owned(),
                    )
                })?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(element);
                } else if root.replace(element).is_some() {
                    return Err(GenerationError::InvalidSpec(
                        "generated XML has more than one root element".to_owned(),
                    ));
                }
            }
            Event::Eof if stack.is_empty() => break,
            Event::Eof => {
                return Err(GenerationError::InvalidSpec(
                    "generated XML has an unclosed element".to_owned(),
                ));
            }
            _ => {}
        }
    }
    root.ok_or_else(|| GenerationError::InvalidSpec("generated XML has no root element".to_owned()))
}

fn append_generated_text(element: &mut XmlElement, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    element.text.get_or_insert_default().push_str(value);
}

fn xml_element_from_start(start: &BytesStart<'_>) -> Result<XmlElement, GenerationError> {
    let mut element = XmlElement::new(start.name().as_ref());
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| {
            GenerationError::InvalidSpec(format!("invalid generated XML attribute: {error}"))
        })?;
        let name = attribute.key.as_ref().to_owned();
        let value = attribute
            .normalized_value(XmlVersion::default())
            .map_err(|error| {
                GenerationError::InvalidSpec(format!("invalid generated XML attribute: {error}"))
            })?
            .into_owned();
        element.attributes.push((name, value));
    }
    Ok(element)
}

fn apply_xml_setting(
    root: &mut XmlElement,
    setting: &crate::model::SepSetting,
) -> Result<(), GenerationError> {
    let segments = setting.path.as_str().split('/').skip(1).collect::<Vec<_>>();
    if segments.first().copied() != Some(root.name.as_str()) {
        return Err(GenerationError::InvalidSpec(format!(
            "setting path `{}` does not address the generated <{}> root",
            setting.path, root.name
        )));
    }
    let mut current = root;
    for (offset, segment) in segments.iter().enumerate().skip(1) {
        let last = offset + 1 == segments.len();
        if let Some(attribute) = segment.strip_prefix('@') {
            if !last {
                return Err(GenerationError::InvalidSpec(
                    "an XML attribute must be the final setting path segment".to_owned(),
                ));
            }
            if let Some((_, value)) = current
                .attributes
                .iter_mut()
                .find(|(name, _)| name == attribute)
            {
                *value = setting.value.to_xml_text();
            } else {
                current
                    .attributes
                    .push((attribute.to_owned(), setting.value.to_xml_text()));
            }
            return Ok(());
        }
        let (name, occurrence) = parse_element_segment(segment)?;
        while current
            .children
            .iter()
            .filter(|child| child.name == name)
            .count()
            < occurrence
        {
            current.children.push(XmlElement::new(name));
        }
        let child_index = current
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| (child.name == name).then_some(index))
            .nth(occurrence - 1)
            .expect("missing children were inserted");
        current = &mut current.children[child_index];
        if last {
            current.children.clear();
            current.text = Some(setting.value.to_xml_text());
        }
    }
    Ok(())
}

fn parse_element_segment(segment: &str) -> Result<(&str, usize), GenerationError> {
    if let Some((name, index)) = segment.rsplit_once('[')
        && let Some(index) = index.strip_suffix(']')
    {
        let occurrence = index.parse::<usize>().map_err(|_| {
            GenerationError::InvalidSpec(format!("invalid element index in `{segment}`"))
        })?;
        return Ok((name, occurrence));
    }
    Ok((segment, 1))
}

fn write_generated_xml(root: &XmlElement) -> Result<String, GenerationError> {
    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    write_xml_element(&mut writer, root)?;
    let body = String::from_utf8(writer.into_inner()).map_err(|error| {
        GenerationError::InvalidSpec(format!("generated XML was not UTF-8: {error}"))
    })?;
    Ok(format!("{XML_DECLARATION}{body}\n"))
}

fn write_xml_element(
    writer: &mut Writer<Vec<u8>>,
    element: &XmlElement,
) -> Result<(), GenerationError> {
    let mut start = BytesStart::new(element.name.as_str());
    for (name, value) in &element.attributes {
        start.push_attribute((name.as_str(), value.as_str()));
    }
    writer.write_event(Event::Start(start)).map_err(|error| {
        GenerationError::InvalidSpec(format!("could not write generated XML: {error}"))
    })?;
    if let Some(text) = &element.text {
        writer
            .write_event(Event::Text(BytesText::new(text)))
            .map_err(|error| {
                GenerationError::InvalidSpec(format!("could not write generated XML: {error}"))
            })?;
    }
    for child in &element.children {
        write_xml_element(writer, child)?;
    }
    writer
        .write_event(Event::End(BytesEnd::new(element.name.as_str())))
        .map_err(|error| {
            GenerationError::InvalidSpec(format!("could not write generated XML: {error}"))
        })?;
    Ok(())
}

fn serialize_xml_default(
    document: &DefaultDocument,
    extra_loads: &BTreeMap<u16, LoadInformation>,
) -> Result<String, GenerationError> {
    let mut body = serialize_pretty(document)?;
    let insertion = body.rfind("</Default>").ok_or_else(|| {
        GenerationError::InvalidSpec("serialized default has no closing element".to_owned())
    })?;
    let mut fragments = String::new();
    for (model_id, load) in extra_loads {
        let fragment = serialize_pretty_with_root(&format!("loadInformation{model_id}"), load)?;
        for line in fragment.lines() {
            fragments.push_str("  ");
            fragments.push_str(line);
            fragments.push('\n');
        }
    }
    body.insert_str(insertion, &fragments);

    let mut output = String::from(XML_DECLARATION);
    output.push_str(&body);
    output.push('\n');
    Ok(output)
}

fn serialize_pretty<T: serde::Serialize>(document: &T) -> Result<String, GenerationError> {
    let mut output = String::new();
    let mut serializer = quick_xml::se::Serializer::new(&mut output);
    serializer.indent(' ', 2);
    document.serialize(serializer)?;
    Ok(output)
}

fn serialize_pretty_with_root<T: serde::Serialize>(
    root: &str,
    document: &T,
) -> Result<String, GenerationError> {
    let mut output = String::new();
    let mut serializer = quick_xml::se::Serializer::with_root(&mut output, Some(root))?;
    serializer.indent(' ', 2);
    document.serialize(serializer)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use crate::model::{
        Host, MacAddress, PhoneModelId, ProtocolSpec, SccpSpec, SepSetting, SepSettingValue,
        ServiceUrls,
    };

    use super::*;

    #[test]
    fn enterprise_generation_escapes_user_controlled_xml() {
        let mut spec = DeviceSpec {
            mac: MacAddress::from_str("00:08:2f:b6:b4:aa").expect("MAC"),
            model: PhoneModelId::from_str("7965").expect("model"),
            firmware: Some("SCCP45.test".to_owned()),
            protocol: ProtocolSpec::Sccp(SccpSpec::default()),
            endpoints: vec![CallControlEndpoint {
                host: Host::from_str("pbx.example.test").expect("host"),
                port: 2000,
                priority: 0,
                transport: Transport::Tcp,
            }],
            phone_label: Some("Ops & Support".to_owned()),
            time_zone: None,
            date_template: None,
            ntp_server: None,
            locale: None,
            services: ServiceUrls {
                services: Some("https://example.test/menu?a=1&b=2".to_owned()),
                ..ServiceUrls::default()
            },
            settings: Vec::new(),
            allow_unknown_settings: false,
        };
        spec.settings.push(SepSetting::new(
            "/device/vendorConfig/displayOnTime"
                .parse()
                .expect("valid display setting path"),
            SepSettingValue::String("09:15".to_owned()),
        ));

        let artifact = generate_device(&spec).expect("generation");
        assert!(artifact.contents.contains("a=1&amp;b=2"));
        assert!(
            artifact
                .contents
                .contains("\n  <deviceProtocol>SCCP</deviceProtocol>\n")
        );
        assert!(artifact.contents.contains("\n    <callManagerGroup>\n"));
        assert_eq!(artifact.filename, "SEP00082FB6B4AA.cnf.xml");
        assert!(!artifact.contains_secrets);
    }
}
