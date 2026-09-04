//! Typed wire representations for enterprise XML bootstrap files.
//!
//! These types intentionally model the stable subset needed by the generator
//! and validator.  `artifact::parse_artifact` uses `serde_ignored` around these
//! DTOs, so fields outside that subset are reported instead of being silently
//! mistaken for supported fields.  The original document is retained by the
//! parsed artifact for lossless inspection.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "device", rename_all = "camelCase")]
pub struct DeviceDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_password: Option<String>,
    #[serde(default)]
    pub device_pool: DevicePool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sip_profile: Option<SipProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_profile: Option<CommonProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_information: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_config: Option<VendorConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_locale: Option<UserLocale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_stamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_security_mode: Option<u8>,
    #[serde(
        default,
        rename = "authenticationURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_url: Option<String>,
    #[serde(
        default,
        rename = "directoryURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub directory_url: Option<String>,
    #[serde(default, rename = "idleURL", skip_serializing_if = "Option::is_none")]
    pub idle_url: Option<String>,
    #[serde(
        default,
        rename = "informationURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub information_url: Option<String>,
    #[serde(
        default,
        rename = "messagesURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub messages_url: Option<String>,
    #[serde(
        default,
        rename = "proxyServerURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub proxy_server_url: Option<String>,
    #[serde(
        default,
        rename = "servicesURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub services_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dscp_for_cm2_dvce: Option<u8>,
    #[serde(
        default,
        rename = "dscpForSCCPPhoneConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub dscp_for_sccp_phone_config: Option<u8>,
    #[serde(
        default,
        rename = "dscpForSCCPPhoneServices",
        skip_serializing_if = "Option::is_none"
    )]
    pub dscp_for_sccp_phone_services: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_layer_protocol: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capf_auth_mode: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encr_config: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePool {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_time_setting: Option<DateTimeSetting>,
    #[serde(default)]
    pub call_manager_group: CallManagerGroup,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub srst_info: Option<SrstInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_monitor_duration: Option<u16>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeSetting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ntps: Option<NtpServers>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct NtpServers {
    #[serde(default, rename = "ntp")]
    pub ntp: Vec<NtpServer>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NtpServer {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ntp_mode: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CallManagerGroup {
    #[serde(default)]
    pub members: CallManagerMembers,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CallManagerMembers {
    #[serde(default, rename = "member")]
    pub member: Vec<CallManagerMember>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallManagerMember {
    #[serde(default, rename = "@priority", skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    #[serde(default)]
    pub call_manager: CallManager,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallManager {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub ports: CallManagerPorts,
    #[serde(default)]
    pub process_node_name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct CallManagerPorts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethernet_phone_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sip_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secured_sip_port: Option<u16>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SipProfile {
    #[serde(default)]
    pub sip_proxies: SipProxies,
    #[serde(default)]
    pub sip_stack: SipStack,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_media_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_media_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_top_level_domain: Option<String>,
    #[serde(default)]
    pub sip_lines: SipLines,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voip_control_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dial_template: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SipProxies {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_proxy_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency_proxy_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outbound_proxy_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register_with_proxy: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SipStack {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sip_invite_retx: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sip_retx: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_invite_expires: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_register_expires: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_register_delta: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_keep_alive_expires: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_subscribe_expires: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_subscribe_delta: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_t1: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_t2: Option<u16>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SipLines {
    #[serde(default, rename = "line")]
    pub line: Vec<SipLine>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SipLine {
    #[serde(default, rename = "@button")]
    pub button: u8,
    #[serde(default, rename = "featureID")]
    pub feature_id: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_password: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_dial_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_option_mask: Option<u32>,
    #[serde(default, rename = "@lineIndex")]
    pub line_index: Option<u8>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_password: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_speaker: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_speaker_and_headset: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarding_delay: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_access: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garp: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_vlan_access: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_capability: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_select_line_enable: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_access: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_access: Option<u8>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SrstInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub srst_option: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_modifiable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port1: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_secure: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLocale {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub win_char_set: Option<String>,
}

/// Typed subset of `XMLDefault.cnf.xml`.
///
/// The format puts a model identifier in the element name (`loadInformation436`).
/// Common public IDs are represented explicitly for typed parsing; generation
/// can also serialize an additional numeric ID supplied by the caller.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename = "Default", rename_all = "camelCase")]
pub struct DefaultDocument {
    #[serde(default)]
    pub call_manager_group: CallManagerGroup,
    #[serde(
        default,
        rename = "loadInformation7",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_7: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation8",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_8: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation115",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_115: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation119",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_119: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation307",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_307: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation308",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_308: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation309",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_309: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation404",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_404: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation434",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_434: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation435",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_435: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation436",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_436: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation495",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_495: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation503",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_503: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation621",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_621: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation622",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_622: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation623",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_623: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation683",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_683: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation684",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_684: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation685",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_685: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation437",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_437: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation30006",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_30006: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation30007",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_30007: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation30018",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_30018: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation36216",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_36216: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation36217",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_36217: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation36224",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_36224: Option<LoadInformation>,
    #[serde(
        default,
        rename = "loadInformation36225",
        skip_serializing_if = "Option::is_none"
    )]
    pub load_36225: Option<LoadInformation>,
    #[serde(
        default,
        rename = "authenticationURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub authentication_url: Option<String>,
    #[serde(
        default,
        rename = "directoryURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub directory_url: Option<String>,
    #[serde(default, rename = "idleURL", skip_serializing_if = "Option::is_none")]
    pub idle_url: Option<String>,
    #[serde(
        default,
        rename = "informationURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub information_url: Option<String>,
    #[serde(
        default,
        rename = "messagesURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub messages_url: Option<String>,
    #[serde(
        default,
        rename = "servicesURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub services_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_locale: Option<UserLocale>,
}

impl DefaultDocument {
    #[must_use]
    pub fn loads(&self) -> Vec<(u32, &LoadInformation)> {
        [
            (7, self.load_7.as_ref()),
            (8, self.load_8.as_ref()),
            (115, self.load_115.as_ref()),
            (119, self.load_119.as_ref()),
            (307, self.load_307.as_ref()),
            (308, self.load_308.as_ref()),
            (309, self.load_309.as_ref()),
            (404, self.load_404.as_ref()),
            (434, self.load_434.as_ref()),
            (435, self.load_435.as_ref()),
            (436, self.load_436.as_ref()),
            (495, self.load_495.as_ref()),
            (503, self.load_503.as_ref()),
            (621, self.load_621.as_ref()),
            (622, self.load_622.as_ref()),
            (623, self.load_623.as_ref()),
            (683, self.load_683.as_ref()),
            (684, self.load_684.as_ref()),
            (685, self.load_685.as_ref()),
            (437, self.load_437.as_ref()),
            (30_006, self.load_30006.as_ref()),
            (30_007, self.load_30007.as_ref()),
            (30_018, self.load_30018.as_ref()),
            (36_216, self.load_36216.as_ref()),
            (36_217, self.load_36217.as_ref()),
            (36_224, self.load_36224.as_ref()),
            (36_225, self.load_36225.as_ref()),
        ]
        .into_iter()
        .filter_map(|(id, load)| load.map(|load| (id, load)))
        .collect()
    }

    pub(crate) fn set_load(
        &mut self,
        model_id: u16,
        load: LoadInformation,
    ) -> Result<(), LoadInformation> {
        let slot = match model_id {
            7 => &mut self.load_7,
            8 => &mut self.load_8,
            115 => &mut self.load_115,
            119 => &mut self.load_119,
            307 => &mut self.load_307,
            308 => &mut self.load_308,
            309 => &mut self.load_309,
            404 => &mut self.load_404,
            434 => &mut self.load_434,
            435 => &mut self.load_435,
            436 => &mut self.load_436,
            437 => &mut self.load_437,
            495 => &mut self.load_495,
            503 => &mut self.load_503,
            621 => &mut self.load_621,
            622 => &mut self.load_622,
            623 => &mut self.load_623,
            683 => &mut self.load_683,
            684 => &mut self.load_684,
            685 => &mut self.load_685,
            30_006 => &mut self.load_30006,
            30_007 => &mut self.load_30007,
            30_018 => &mut self.load_30018,
            36_216 => &mut self.load_36216,
            36_217 => &mut self.load_36217,
            36_224 => &mut self.load_36224,
            36_225 => &mut self.load_36225,
            _ => return Err(load),
        };
        *slot = Some(load);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LoadInformation {
    #[serde(default, rename = "@model")]
    pub model: Option<String>,
    #[serde(default, rename = "$text")]
    pub firmware: String,
}
