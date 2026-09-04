//! Deterministic configuration generation from protocol-independent models.

use std::collections::{BTreeMap, HashSet};

use thiserror::Error;

use crate::catalog::resolve_profile;
use crate::diagnostic::{Diagnostic, code};
use crate::legacy::{LegacySipConfig, assignment, section};
use crate::model::{
    ArtifactDialect, ArtifactKind, CallControlEndpoint, DefaultSpec, DeviceSpec, GeneratedArtifact,
    Protocol, ProtocolSpec, SignalingMode, SipButton, SipButtonFeature, SipLine, SipSpec,
    Transport,
};
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
        artifact.warnings = warnings;
        return Ok(artifact);
    }
    let mut artifact = generate_enterprise_device(spec)?;
    artifact.warnings = warnings;
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
    let contents = serialize_xml(&document)?;
    let contains_secrets = matches!(&spec.protocol, ProtocolSpec::Sip(sip) if sip.lines.iter().any(|line| line.auth_secret.is_some()));

    Ok(GeneratedArtifact {
        filename: spec.mac.sep_filename(),
        kind: ArtifactKind::DeviceConfiguration,
        dialect: ArtifactDialect::EnterpriseXml,
        contents,
        contains_secrets,
        warnings: Vec::new(),
    })
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

    use crate::model::{Host, MacAddress, PhoneModelId, ProtocolSpec, SccpSpec, ServiceUrls};

    use super::*;

    #[test]
    fn enterprise_generation_escapes_user_controlled_xml() {
        let spec = DeviceSpec {
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
        };

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
