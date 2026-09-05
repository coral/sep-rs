//! Profile-aware semantic validation for parsed bootstrap artifacts.

use std::collections::HashSet;
use std::str::FromStr;

use url::Url;

use crate::artifact::{ParsedArtifact, ParsedDocument};
use crate::catalog::resolve_profile;
use crate::diagnostic::{Diagnostic, code};
use crate::legacy::LegacyEntry;
use crate::model::{Host, PhoneModelId, Protocol};
use crate::settings::validate_xml_settings;
use crate::xml::{CallManagerGroup, DeviceDocument, SipLine};

/// Validate an already parsed artifact. A model hint enables compatibility
/// checks that cannot be inferred from a SEP document itself.
#[must_use]
pub fn validate(artifact: &ParsedArtifact, model_hint: Option<&PhoneModelId>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    validate_placeholders(artifact.original_source(), &mut diagnostics);
    validate_filename(artifact, &mut diagnostics);

    match artifact.document() {
        ParsedDocument::Device(document) => {
            if let Some(protocol) = validate_device(document, model_hint, &mut diagnostics) {
                diagnostics.extend(validate_xml_settings(
                    artifact.original_source(),
                    model_hint,
                    protocol,
                ));
            }
        }
        ParsedDocument::Default(document) => {
            validate_call_managers(&document.call_manager_group, &mut diagnostics);
            for (model_id, load) in document.loads() {
                if load.firmware.trim().is_empty() {
                    diagnostics.push(
                        Diagnostic::warning(
                            code::UNKNOWN_FIRMWARE,
                            "default load entry has an empty firmware value",
                        )
                        .at(format!("Default.loadInformation{model_id}")),
                    );
                }
            }
        }
        ParsedDocument::LegacySip(document) => {
            validate_legacy(document, artifact.kind(), &mut diagnostics);
        }
    }

    diagnostics
}

fn validate_device(
    document: &DeviceDocument,
    model_hint: Option<&PhoneModelId>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Protocol> {
    let protocol = match document.device_protocol.as_deref().map(str::parse) {
        Some(Ok(protocol)) => Some(protocol),
        Some(Err(_)) => {
            diagnostics.push(
                Diagnostic::error(
                    code::MODEL_PROTOCOL_MISMATCH,
                    "deviceProtocol must be SCCP or SIP",
                )
                .at("device.deviceProtocol"),
            );
            None
        }
        None => {
            diagnostics.push(
                Diagnostic::error(
                    code::MODEL_PROTOCOL_MISMATCH,
                    "deviceProtocol is required for a device configuration",
                )
                .at("device.deviceProtocol"),
            );
            None
        }
    };

    validate_call_managers(&document.device_pool.call_manager_group, diagnostics);
    validate_ntp_servers(document, diagnostics);

    if let Some(protocol) = protocol {
        validate_endpoint_ports(document, protocol, diagnostics);
        validate_security(document, protocol, diagnostics);
        validate_firmware(document.load_information.as_deref(), protocol, diagnostics);
        if let Some(model) = model_hint {
            validate_profile(
                model,
                protocol,
                document.load_information.as_deref(),
                diagnostics,
            );
        }
    }

    match (protocol, document.sip_profile.as_ref()) {
        (Some(Protocol::Sip), Some(profile)) => validate_sip_profile(profile, diagnostics),
        (Some(Protocol::Sip), None) => diagnostics.push(
            Diagnostic::error(
                code::MODEL_PROTOCOL_MISMATCH,
                "enterprise SIP configuration requires sipProfile",
            )
            .at("device.sipProfile"),
        ),
        (Some(Protocol::Sccp), Some(_)) => diagnostics.push(
            Diagnostic::error(
                code::MODEL_PROTOCOL_MISMATCH,
                "SCCP bootstrap must not contain SIP line credentials",
            )
            .at("device.sipProfile"),
        ),
        _ => {}
    }

    for (path, secret) in [
        ("device.sshPassword", document.ssh_password.as_deref()),
        (
            "device.commonProfile.phonePassword",
            document
                .common_profile
                .as_ref()
                .and_then(|profile| profile.phone_password.as_deref()),
        ),
    ] {
        if secret.is_some_and(|value| !value.is_empty()) {
            diagnostics.push(
                Diagnostic::warning(
                    code::CLEARTEXT_SECRET,
                    "configuration contains a cleartext device credential",
                )
                .at(path),
            );
        }
    }

    for (path, value) in [
        (
            "device.authenticationURL",
            document.authentication_url.as_deref(),
        ),
        ("device.directoryURL", document.directory_url.as_deref()),
        ("device.idleURL", document.idle_url.as_deref()),
        ("device.informationURL", document.information_url.as_deref()),
        ("device.messagesURL", document.messages_url.as_deref()),
        (
            "device.proxyServerURL",
            document.proxy_server_url.as_deref(),
        ),
        ("device.servicesURL", document.services_url.as_deref()),
    ] {
        validate_url(path, value, diagnostics);
    }
    protocol
}

fn validate_ntp_servers(document: &DeviceDocument, diagnostics: &mut Vec<Diagnostic>) {
    let Some(ntps) = document
        .device_pool
        .date_time_setting
        .as_ref()
        .and_then(|settings| settings.ntps.as_ref())
    else {
        return;
    };

    for (index, ntp) in ntps.ntp.iter().enumerate() {
        if ntp.name.trim().is_empty() {
            diagnostics.push(
                Diagnostic::error(code::MISSING_ENDPOINT, "NTP server name is required").at(
                    format!("device.devicePool.dateTimeSetting.ntps.ntp[{index}].name"),
                ),
            );
        }
    }
}

fn validate_call_managers(group: &CallManagerGroup, diagnostics: &mut Vec<Diagnostic>) {
    let members = &group.members.member;
    if members.is_empty() {
        diagnostics.push(
            Diagnostic::error(
                code::MISSING_ENDPOINT,
                "at least one call-control member is required",
            )
            .at("callManagerGroup.members"),
        );
        return;
    }

    let mut priorities = HashSet::new();
    for (index, member) in members.iter().enumerate() {
        let path = format!("callManagerGroup.members.member[{index}]");
        if let Some(priority) = member.priority {
            if !priorities.insert(priority) {
                diagnostics.push(
                    Diagnostic::error(
                        code::DUPLICATE_PRIORITY,
                        format!("duplicate call-manager priority {priority}"),
                    )
                    .at(format!("{path}.@priority")),
                );
            }
        } else {
            diagnostics.push(
                Diagnostic::warning(
                    code::DUPLICATE_PRIORITY,
                    "call-manager priority is absent; some phone families require it",
                )
                .at(format!("{path}.@priority")),
            );
        }
        if Host::from_str(&member.call_manager.process_node_name).is_err() {
            diagnostics.push(
                Diagnostic::error(code::MISSING_ENDPOINT, "invalid call-control host")
                    .at(format!("{path}.callManager.processNodeName")),
            );
        }
        for (name, port) in [
            (
                "ethernetPhonePort",
                member.call_manager.ports.ethernet_phone_port,
            ),
            ("sipPort", member.call_manager.ports.sip_port),
            ("securedSipPort", member.call_manager.ports.secured_sip_port),
        ] {
            if port == Some(0) {
                diagnostics.push(
                    Diagnostic::error(code::INVALID_PORT, "port must be in 1..=65535")
                        .at(format!("{path}.callManager.ports.{name}")),
                );
            }
        }
    }

    let mut ordered = priorities.into_iter().collect::<Vec<_>>();
    ordered.sort_unstable();
    if ordered
        .windows(2)
        .any(|window| window[1] != window[0].saturating_add(1))
    {
        diagnostics.push(Diagnostic::warning(
            code::DUPLICATE_PRIORITY,
            "call-manager priorities are not contiguous",
        ));
    }
}

fn validate_endpoint_ports(
    document: &DeviceDocument,
    protocol: Protocol,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, member) in document
        .device_pool
        .call_manager_group
        .members
        .member
        .iter()
        .enumerate()
    {
        let ports = &member.call_manager.ports;
        let usable = match protocol {
            Protocol::Sccp => ports.ethernet_phone_port.is_some_and(|port| port != 0),
            Protocol::Sip => {
                ports.sip_port.is_some_and(|port| port != 0)
                    || ports.secured_sip_port.is_some_and(|port| port != 0)
            }
        };
        if !usable {
            diagnostics.push(
                Diagnostic::error(
                    code::INVALID_PORT,
                    format!("{protocol} endpoint has no usable signaling port"),
                )
                .at(format!(
                    "device.devicePool.callManagerGroup.members.member[{index}].callManager.ports"
                )),
            );
        }
    }
}

fn validate_security(
    document: &DeviceDocument,
    protocol: Protocol,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mode = document.device_security_mode;
    if mode.is_some_and(|value| value > 3) {
        diagnostics.push(
            Diagnostic::warning(
                code::UNKNOWN_FIELD,
                "deviceSecurityMode is not mapped by the selected common profile",
            )
            .at("device.deviceSecurityMode"),
        );
        return;
    }
    let secure = mode.is_some_and(|value| value >= 2);
    let encrypted = mode == Some(3);
    let missing_secure_sip_endpoint = document
        .device_pool
        .call_manager_group
        .members
        .member
        .iter()
        .any(|member| {
            member
                .call_manager
                .ports
                .secured_sip_port
                .is_none_or(|port| port == 0)
        });

    match protocol {
        Protocol::Sccp if secure => {
            if document
                .device_pool
                .call_manager_group
                .members
                .member
                .iter()
                .any(|member| {
                    member
                        .call_manager
                        .ports
                        .ethernet_phone_port
                        .is_none_or(|port| port == 2000)
                })
            {
                diagnostics.push(
                    Diagnostic::error(
                        code::SECURE_MODE_MISMATCH,
                        "authenticated/encrypted SCCP cannot use nonsecure port 2000 (the standard secure port is 2443)",
                    )
                    .at("device.devicePool.callManagerGroup"),
                );
            }
        }
        Protocol::Sip
            if (secure || document.transport_layer_protocol == Some(3))
                && missing_secure_sip_endpoint =>
        {
            diagnostics.push(
                Diagnostic::error(
                    code::SECURE_MODE_MISMATCH,
                    "authenticated/encrypted SIP requires a secured SIP endpoint",
                )
                .at("device.devicePool.callManagerGroup"),
            );
        }
        _ => {}
    }

    if document.encr_config == Some(true) && !encrypted
        || encrypted && document.encr_config == Some(false)
    {
        diagnostics.push(
            Diagnostic::error(
                code::SECURE_MODE_MISMATCH,
                "encrConfig contradicts deviceSecurityMode",
            )
            .at("device.encrConfig"),
        );
    }
    if secure
        && document
            .cert_hash
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        diagnostics.push(
            Diagnostic::warning(
                code::SECURE_MODE_MISMATCH,
                "secure mode has no certificate hash; verify CAPF/ITL/CTL provisioning",
            )
            .at("device.certHash"),
        );
    }
}

fn validate_sip_profile(profile: &crate::xml::SipProfile, diagnostics: &mut Vec<Diagnostic>) {
    if profile.sip_lines.line.is_empty() {
        diagnostics.push(
            Diagnostic::error(
                code::DUPLICATE_LINE,
                "SIP configuration requires at least one registration line",
            )
            .at("device.sipProfile.sipLines"),
        );
    }

    if let (Some(start), Some(stop)) = (profile.start_media_port, profile.stop_media_port)
        && start > stop
    {
        diagnostics.push(
            Diagnostic::error(
                code::INVALID_PORT,
                "startMediaPort must not exceed stopMediaPort",
            )
            .at("device.sipProfile"),
        );
    }

    let mut buttons = HashSet::new();
    let mut line_indexes = HashSet::new();
    for (index, line) in profile.sip_lines.line.iter().enumerate() {
        validate_sip_line(line, index, &mut buttons, &mut line_indexes, diagnostics);
    }
    let mut ordered = line_indexes.into_iter().collect::<Vec<_>>();
    ordered.sort_unstable();
    if ordered
        .windows(2)
        .any(|window| window[1] != window[0].saturating_add(1))
    {
        diagnostics.push(
            Diagnostic::error(
                code::DUPLICATE_LINE,
                "SIP lineIndex values must be contiguous",
            )
            .at("device.sipProfile.sipLines"),
        );
    }
}

fn validate_sip_line(
    line: &SipLine,
    index: usize,
    buttons: &mut HashSet<u8>,
    line_indexes: &mut HashSet<u8>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = format!("device.sipProfile.sipLines.line[{index}]");
    if line.button == 0 || !buttons.insert(line.button) {
        diagnostics.push(
            Diagnostic::error(code::DUPLICATE_BUTTON, "button must be nonzero and unique")
                .at(format!("{path}.@button")),
        );
    }
    if let Some(line_index) = line.line_index
        && (line_index == 0 || !line_indexes.insert(line_index))
    {
        diagnostics.push(
            Diagnostic::error(code::DUPLICATE_LINE, "lineIndex must be nonzero and unique")
                .at(format!("{path}.@lineIndex")),
        );
    }
    if line.port == Some(0) {
        diagnostics.push(
            Diagnostic::error(code::INVALID_PORT, "SIP line port must be in 1..=65535")
                .at(format!("{path}.port")),
        );
    }
    if line.feature_id == 9 && line.name.as_deref().is_none_or(str::is_empty) {
        diagnostics.push(
            Diagnostic::error(
                code::DUPLICATE_LINE,
                "registration line requires a non-empty name",
            )
            .at(format!("{path}.name")),
        );
    }
    match line.feature_id {
        2 | 21 if line.speed_dial_number.as_deref().is_none_or(str::is_empty) => {
            diagnostics.push(
                Diagnostic::error(
                    code::DUPLICATE_BUTTON,
                    "speed-dial/BLF button requires speedDialNumber",
                )
                .at(format!("{path}.speedDialNumber")),
            );
        }
        20 if line.service_uri.as_deref().is_none_or(str::is_empty) => {
            diagnostics.push(
                Diagnostic::error(code::DUPLICATE_BUTTON, "service button requires serviceURI")
                    .at(format!("{path}.serviceURI")),
            );
        }
        2 | 9 | 20 | 21 | 23 => {}
        feature_id => diagnostics.push(
            Diagnostic::warning(
                code::RAW_FEATURE,
                format!("featureID {feature_id} is preserved as a raw vendor feature"),
            )
            .at(format!("{path}.featureID")),
        ),
    }
    if line
        .auth_password
        .as_deref()
        .is_some_and(|value| !value.is_empty())
    {
        diagnostics.push(
            Diagnostic::warning(
                code::CLEARTEXT_SECRET,
                "configuration contains a cleartext SIP credential",
            )
            .at(format!("{path}.authPassword")),
        );
    }
}

fn validate_profile(
    model: &PhoneModelId,
    protocol: Protocol,
    firmware: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(profile) = resolve_profile(model) else {
        diagnostics.push(Diagnostic::warning(
            code::UNKNOWN_MODEL,
            format!("no built-in profile for model {model}"),
        ));
        return;
    };
    if !profile.supports_protocol(protocol) {
        diagnostics.push(Diagnostic::warning(
            code::MODEL_PROTOCOL_MISMATCH,
            format!("{} does not support {protocol}", profile.id),
        ));
    }
    if let Some(firmware) = firmware
        && !profile.load_prefixes.is_empty()
        && !profile.load_prefixes.iter().any(|prefix| {
            firmware
                .to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
        })
    {
        diagnostics.push(Diagnostic::warning(
            code::UNKNOWN_FIRMWARE,
            format!(
                "firmware does not match known load prefixes for {}",
                profile.id
            ),
        ));
    }
}

fn validate_firmware(
    firmware: Option<&str>,
    protocol: Protocol,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(firmware) = firmware.filter(|value| !value.trim().is_empty()) else {
        diagnostics.push(Diagnostic::warning(
            code::UNKNOWN_FIRMWARE,
            "firmware/load information is empty or absent",
        ));
        return;
    };
    let upper = firmware.to_ascii_uppercase();
    let contradiction = match protocol {
        Protocol::Sccp => upper.starts_with("SIP") || upper.starts_with("P0S"),
        Protocol::Sip => upper.starts_with("SCCP") || upper.starts_with("P003"),
    };
    if contradiction {
        diagnostics.push(Diagnostic::error(
            code::MODEL_PROTOCOL_MISMATCH,
            format!("firmware load {firmware} contradicts {protocol}"),
        ));
    }
}

fn validate_legacy(
    document: &crate::legacy::LegacySipConfig,
    kind: crate::model::ArtifactKind,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if document.get("proxy1_address").is_none_or(str::is_empty) {
        diagnostics.push(
            Diagnostic::error(code::MISSING_ENDPOINT, "proxy1_address is required")
                .at("proxy1_address"),
        );
    }
    if kind == crate::model::ArtifactKind::DeviceConfiguration
        && document.get("line1_name").is_none_or(str::is_empty)
        && document.get("image_version").is_none()
    {
        diagnostics.push(
            Diagnostic::error(code::DUPLICATE_LINE, "device profile requires line1_name")
                .at("line1_name"),
        );
    }
    for entry in &document.entries {
        let LegacyEntry::Assignment(assignment) = entry else {
            continue;
        };
        let key = assignment.key.to_ascii_lowercase();
        if key.ends_with("_port") || key == "voip_control_port" {
            match assignment.value.parse::<u16>() {
                Ok(1..) => {}
                _ if assignment.value.is_empty() => {}
                _ => diagnostics.push(
                    Diagnostic::error(code::INVALID_PORT, "port must be in 1..=65535")
                        .at(&assignment.key),
                ),
            }
        }
        if (key.ends_with("password") || key.ends_with("_password")) && !assignment.value.is_empty()
        {
            diagnostics.push(
                Diagnostic::warning(
                    code::CLEARTEXT_SECRET,
                    "configuration contains a cleartext SIP credential",
                )
                .at(&assignment.key),
            );
        }
    }
}

fn validate_filename(artifact: &ParsedArtifact, diagnostics: &mut Vec<Diagnostic>) {
    let Some(filename) = artifact.filename_hint() else {
        return;
    };
    let filename = std::path::Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(filename);
    let valid = match artifact.document() {
        ParsedDocument::Device(_) => canonical_device_filename(filename, "SEP", ".cnf.xml"),
        ParsedDocument::LegacySip(_)
            if artifact.kind() == crate::model::ArtifactKind::DeviceConfiguration =>
        {
            canonical_device_filename(filename, "SIP", ".cnf")
        }
        ParsedDocument::Default(_) => filename.eq_ignore_ascii_case("XMLDefault.cnf.xml"),
        ParsedDocument::LegacySip(_) => filename.eq_ignore_ascii_case("SIPDefault.cnf"),
    };
    if !valid {
        diagnostics.push(Diagnostic::warning(
            code::FILENAME_MAC_MISMATCH,
            "filename is not canonical for this artifact dialect",
        ));
    }
}

fn canonical_device_filename(filename: &str, prefix: &str, suffix: &str) -> bool {
    filename.len() == prefix.len() + 12 + suffix.len()
        && filename.starts_with(prefix)
        && filename.ends_with(suffix)
        && filename[prefix.len()..prefix.len() + 12]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_lowercase())
}

fn validate_placeholders(source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let upper = source.to_ascii_uppercase();
    if source.contains("###")
        || upper.contains("UNPROVISIONED")
        || upper.contains("FAILOVER_NODE_IP")
        || upper.contains("REALM_DOMAIN")
        || upper.contains("_HERE")
    {
        diagnostics.push(Diagnostic::error(
            code::PLACEHOLDER,
            "configuration contains an unresolved template placeholder",
        ));
    }
}

fn validate_url(path: &str, value: Option<&str>, diagnostics: &mut Vec<Diagnostic>) {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    if Url::parse(value).is_err() {
        diagnostics.push(
            Diagnostic::warning(code::UNKNOWN_FIELD, "URL is not syntactically valid").at(path),
        );
    }
}
