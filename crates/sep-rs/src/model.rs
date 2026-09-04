//! Protocol-independent models used by generators and validators.

use std::{fmt, net::IpAddr, str::FromStr};

use macaddr::MacAddr6;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::diagnostic::Diagnostic;

/// The syntax and provisioning ecosystem used by an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactDialect {
    /// Enterprise `SEP<MAC>.cnf.xml` and `XMLDefault.cnf.xml` files.
    EnterpriseXml,
    /// Colon/equal-delimited SIP configuration used by the 7940 and 7960.
    LegacySipText,
    /// Compiled 7905/7912 profiles produced by Cisco's external tooling.
    CompiledBinary,
    /// A signed enterprise XML artifact which can be inspected but not generated.
    SignedXml,
    /// An encrypted and signed enterprise artifact which cannot be decoded here.
    EncryptedXml,
    /// Cisco multiplatform/3PCC provisioning, intentionally outside v1 scope.
    Mpp3pcc,
}

/// The role of a bootstrap artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    DeviceConfiguration,
    DefaultConfiguration,
    LoadDescriptor,
    Firmware,
    DialPlan,
    SoftKeyPolicy,
    Locale,
    TrustList,
    Other,
}

/// Call-control protocol selected for a phone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Sccp,
    Sip,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sccp => "SCCP",
            Self::Sip => "SIP",
        })
    }
}

/// Security mode requested for signaling.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalingMode {
    #[default]
    NonSecure,
    Authenticated,
    Encrypted,
}

/// Transport used to reach a call-control endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Udp,
    #[default]
    Tcp,
    Tls,
}

/// A normalized six-octet phone MAC address.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MacAddress(MacAddr6);

impl MacAddress {
    #[must_use]
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(MacAddr6::new(
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5],
        ))
    }

    #[must_use]
    pub const fn octets(self) -> [u8; 6] {
        self.0.into_array()
    }

    /// Return the uppercase twelve-digit form used in Cisco filenames.
    #[must_use]
    pub fn compact(self) -> String {
        let octets = self.octets();
        format!(
            "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            octets[0], octets[1], octets[2], octets[3], octets[4], octets[5]
        )
    }

    /// Return the canonical enterprise device identifier.
    #[must_use]
    pub fn sep_name(self) -> String {
        format!("SEP{}", self.compact())
    }

    /// Return the canonical enterprise XML filename.
    #[must_use]
    pub fn sep_filename(self) -> String {
        format!("{}.cnf.xml", self.sep_name())
    }

    /// Return the canonical legacy SIP filename.
    #[must_use]
    pub fn legacy_sip_filename(self) -> String {
        format!("SIP{}.cnf", self.compact())
    }
}

impl fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.compact())
    }
}

impl FromStr for MacAddress {
    type Err = ParseMacAddressError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let value = input.trim();
        let value = value
            .strip_prefix("SEP")
            .or_else(|| value.strip_prefix("sep"))
            .or_else(|| value.strip_prefix("SIP"))
            .or_else(|| value.strip_prefix("sip"))
            .unwrap_or(value);

        if value.len() == 12 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            let mut octets = [0_u8; 6];
            for (index, octet) in octets.iter_mut().enumerate() {
                let offset = index * 2;
                *octet = u8::from_str_radix(&value[offset..offset + 2], 16)
                    .map_err(|_| ParseMacAddressError)?;
            }
            return Ok(Self::new(octets));
        }

        value
            .parse::<MacAddr6>()
            .map(Self)
            .map_err(|_| ParseMacAddressError)
    }
}

impl Serialize for MacAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.compact())
    }
}

impl<'de> Deserialize<'de> for MacAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("expected a six-octet MAC address")]
pub struct ParseMacAddressError;

/// An IP address or a DNS hostname.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Host {
    Ip(IpAddr),
    Name(String),
}

impl Host {
    #[must_use]
    pub const fn as_ip(&self) -> Option<IpAddr> {
        match self {
            Self::Ip(address) => Some(*address),
            Self::Name(_) => None,
        }
    }

    #[must_use]
    pub fn as_name(&self) -> Option<&str> {
        match self {
            Self::Ip(_) => None,
            Self::Name(name) => Some(name),
        }
    }
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ip(address) => address.fmt(f),
            Self::Name(name) => f.write_str(name),
        }
    }
}

impl From<IpAddr> for Host {
    fn from(value: IpAddr) -> Self {
        Self::Ip(value)
    }
}

impl FromStr for Host {
    type Err = ParseHostError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let value = input.trim();
        if value.is_empty()
            || value.chars().any(char::is_whitespace)
            || value.contains('/')
            || value.contains("//")
        {
            return Err(ParseHostError);
        }
        Ok(value
            .parse::<IpAddr>()
            .map_or_else(|_| Self::Name(value.to_owned()), Self::Ip))
    }
}

impl Serialize for Host {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Host {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("expected an IP address or non-empty hostname")]
pub struct ParseHostError;

/// Stable, user-facing model identifier such as `CP-7965G`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PhoneModelId(String);

impl PhoneModelId {
    /// Construct a non-empty model identifier.
    ///
    /// # Errors
    ///
    /// Returns [`ParsePhoneModelIdError`] when the identifier is empty or
    /// contains control characters.
    pub fn new(value: impl Into<String>) -> Result<Self, ParsePhoneModelIdError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(ParsePhoneModelIdError);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Normalized comparison key used for matching catalog aliases.
    #[must_use]
    pub fn normalized(&self) -> String {
        self.0
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_uppercase)
            .collect()
    }
}

impl fmt::Display for PhoneModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PhoneModelId {
    type Err = ParsePhoneModelIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl Serialize for PhoneModelId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PhoneModelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("phone model identifier cannot be empty or contain control characters")]
pub struct ParsePhoneModelIdError;

/// A call-control server in phone preference order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallControlEndpoint {
    pub host: Host,
    pub port: u16,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub transport: Transport,
}

const fn default_priority() -> u8 {
    0
}

/// Protocol-specific device settings.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolSpec {
    Sccp(SccpSpec),
    Sip(SipSpec),
}

impl ProtocolSpec {
    #[must_use]
    pub const fn protocol(&self) -> Protocol {
        match self {
            Self::Sccp(_) => Protocol::Sccp,
            Self::Sip(_) => Protocol::Sip,
        }
    }
}

impl fmt::Debug for ProtocolSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sccp(settings) => f.debug_tuple("Sccp").field(settings).finish(),
            Self::Sip(settings) => f.debug_tuple("Sip").field(settings).finish(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SccpSpec {
    #[serde(default)]
    pub signaling: SignalingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepalive_seconds: Option<u16>,
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SipSpec {
    #[serde(default)]
    pub signaling: SignalingMode,
    #[serde(default)]
    pub lines: Vec<SipLine>,
    #[serde(default)]
    pub buttons: Vec<SipButton>,
    #[serde(default)]
    pub timers: SipTimers,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_ports: Option<MediaPortRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_proxy: Option<Host>,
}

impl fmt::Debug for SipSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SipSpec")
            .field("signaling", &self.signaling)
            .field("lines", &self.lines)
            .field("buttons", &self.buttons)
            .field("timers", &self.timers)
            .field("media_ports", &self.media_ports)
            .field("outbound_proxy", &self.outbound_proxy)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SipLine {
    pub index: u8,
    pub directory_number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_secret: Option<Secret>,
}

impl fmt::Debug for SipLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SipLine")
            .field("index", &self.index)
            .field("directory_number", &self.directory_number)
            .field("display_name", &self.display_name)
            .field("auth_name", &self.auth_name)
            .field(
                "auth_secret",
                &self.auth_secret.as_ref().map(|_| Secret::REDACTED),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SipButton {
    pub position: u8,
    #[serde(flatten)]
    pub feature: SipButtonFeature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "feature", rename_all = "snake_case")]
pub enum SipButtonFeature {
    Line {
        line_index: u8,
    },
    SpeedDial {
        label: String,
        target: String,
    },
    ServiceUri {
        label: String,
        uri: String,
    },
    Blf {
        label: String,
        target: String,
    },
    Intercom {
        line_index: u8,
    },
    /// Escape hatch for vendor feature IDs without a named library variant.
    Raw {
        feature_id: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SipTimers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register_expires_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_expires_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepalive_seconds: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaPortRange {
    pub start: u16,
    pub end: u16,
}

/// Secret string that is serializable for manifests but always redacted in
/// formatting and diagnostics.
#[derive(Clone)]
pub struct Secret(SecretString);

impl Secret {
    pub const REDACTED: &'static str = "[REDACTED]";

    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::from(value.into()))
    }

    /// Explicitly expose the value for configuration generation.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.expose_secret() == other.expose_secret()
    }
}

impl Eq for Secret {}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::REDACTED)
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::REDACTED)
    }
}

impl Serialize for Secret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.expose_secret())
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::new(String::deserialize(deserializer)?))
    }
}

/// Optional service URLs supported by enterprise XML phones.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceUrls {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub services: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle: Option<String>,
}

/// Complete semantic input for one phone configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSpec {
    pub mac: MacAddress,
    pub model: PhoneModelId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    pub protocol: ProtocolSpec,
    pub endpoints: Vec<CallControlEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ntp_server: Option<Host>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default)]
    pub services: ServiceUrls,
}

/// A model-specific firmware reference in a defaults document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelLoad {
    pub model: PhoneModelId,
    /// Explicit numeric suffix for an unlisted model's `loadInformation`
    /// element. A purely numeric `model` value works without this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<u16>,
    pub firmware: String,
}

/// Inputs for a default configuration artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultSpec {
    pub protocol: Protocol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(default)]
    pub endpoints: Vec<CallControlEndpoint>,
    #[serde(default)]
    pub model_loads: Vec<ModelLoad>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ntp_server: Option<Host>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// An artifact which is required by a deployment but not generated by this
/// library (for example a firmware payload or trust list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalArtifact {
    pub filename: String,
    pub kind: ArtifactKind,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

const fn default_true() -> bool {
    true
}

/// Multi-device input manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleSpec {
    #[serde(default)]
    pub devices: Vec<DeviceSpec>,
    #[serde(default)]
    pub defaults: Vec<DefaultSpec>,
    #[serde(default)]
    pub external_artifacts: Vec<ExternalArtifact>,
}

/// A generated plaintext configuration artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedArtifact {
    pub filename: String,
    pub kind: ArtifactKind,
    pub dialect: ArtifactDialect,
    pub contents: String,
    #[serde(default)]
    pub contains_secrets: bool,
    /// Compatibility concerns which did not prevent safe serialization.
    #[serde(default)]
    pub warnings: Vec<Diagnostic>,
}

/// Whether an inventory entry is generated or must be supplied externally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventorySource {
    Generated,
    External,
}

/// A single file expected in a complete TFTP bootstrap directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub filename: String,
    pub kind: ArtifactKind,
    pub source: InventorySource,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleInventory {
    #[serde(default)]
    pub files: Vec<InventoryEntry>,
}

/// One file supplied to the platform-neutral bundle validator. Opaque binary
/// artifacts may omit `contents`; their presence still satisfies inventory
/// checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFile {
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<String>,
}

/// Generated files and the inventory needed to deploy them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapBundle {
    #[serde(default)]
    pub artifacts: Vec<GeneratedArtifact>,
    #[serde(default)]
    pub inventory: BundleInventory,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_formats_are_normalized_for_cisco_filenames() {
        for input in [
            "00:08:2f:b6:b4:aa",
            "00-08-2F-B6-B4-AA",
            "00082fb6b4aa",
            "SEP00082FB6B4AA",
        ] {
            let mac: MacAddress = input.parse().expect("valid fixture MAC");
            assert_eq!(mac.compact(), "00082FB6B4AA");
            assert_eq!(mac.sep_filename(), "SEP00082FB6B4AA.cnf.xml");
        }
    }

    #[test]
    fn secrets_are_redacted_from_nested_debug_output() {
        let line = SipLine {
            index: 1,
            directory_number: "1001".into(),
            display_name: None,
            auth_name: Some("1001".into()),
            auth_secret: Some(Secret::new("not-for-logs")),
        };
        let debug = format!("{line:?}");
        assert!(debug.contains(Secret::REDACTED));
        assert!(!debug.contains("not-for-logs"));

        let encoded = serde_json::to_string(&line).expect("serialize manifest input");
        assert!(encoded.contains("not-for-logs"));
    }

    #[test]
    fn generated_artifacts_serialize_an_empty_warning_list() {
        let artifact = GeneratedArtifact {
            filename: "SEP001122334455.cnf.xml".into(),
            kind: ArtifactKind::DeviceConfiguration,
            dialect: ArtifactDialect::EnterpriseXml,
            contents: "<device/>".into(),
            contains_secrets: false,
            warnings: Vec::new(),
        };

        let encoded = serde_json::to_value(artifact).expect("serialize generated artifact");
        assert_eq!(encoded["warnings"], serde_json::json!([]));
    }
}
