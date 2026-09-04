//! Cisco enterprise phone generation profiles. The catalog retains only the
//! small set of public wire-format facts needed to select a codec and construct
//! a bootstrap artifact.

use crate::model::{ArtifactDialect, PhoneModelId, Protocol};

/// Static generation hints for a phone model or closely related model family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhoneProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    /// Numeric suffix used by the model's `loadInformation` element.
    pub model_id: u16,
    pub protocols: &'static [Protocol],
    pub dialects: &'static [ArtifactDialect],
    pub load_prefixes: &'static [&'static str],
}

/// Backwards-compatible descriptive alias used by the crate facade.
pub type ModelProfile = PhoneProfile;

impl PhoneProfile {
    #[must_use]
    pub fn supports_protocol(self, protocol: Protocol) -> bool {
        self.protocols.contains(&protocol)
    }

    #[must_use]
    pub fn supports_dialect(self, dialect: ArtifactDialect) -> bool {
        self.dialects.contains(&dialect)
    }
}

const BOTH_PROTOCOLS: &[Protocol] = &[Protocol::Sccp, Protocol::Sip];
const SIP_ONLY: &[Protocol] = &[Protocol::Sip];
const ENTERPRISE_XML: &[ArtifactDialect] = &[ArtifactDialect::EnterpriseXml];
const LEGACY_AND_XML: &[ArtifactDialect] = &[
    ArtifactDialect::EnterpriseXml,
    ArtifactDialect::LegacySipText,
];

/// Known enterprise phone profiles used for compatibility checks and dialect
/// selection. Unlisted models fall back to generic enterprise XML with a
/// warning.
pub static PHONE_PROFILES: &[PhoneProfile] = &[
    PhoneProfile {
        id: "CP-7940G",
        display_name: "Cisco 7940",
        aliases: &["7940", "7940G", "Cisco 7940"],
        model_id: 8,
        protocols: BOTH_PROTOCOLS,
        dialects: LEGACY_AND_XML,
        load_prefixes: &["P003", "P0S3"],
    },
    PhoneProfile {
        id: "CP-7960G",
        display_name: "Cisco 7960",
        aliases: &["7960", "7960G", "Cisco 7960"],
        model_id: 7,
        protocols: BOTH_PROTOCOLS,
        dialects: LEGACY_AND_XML,
        load_prefixes: &["P003", "P0S3"],
    },
    PhoneProfile {
        id: "CP-7911G",
        display_name: "Cisco 7911",
        aliases: &["7911", "7911G", "Cisco 7911"],
        model_id: 307,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP11", "SIP11"],
    },
    PhoneProfile {
        id: "CP-7941G",
        display_name: "Cisco 7941",
        aliases: &["7941", "7941G"],
        model_id: 115,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP41", "SIP41"],
    },
    PhoneProfile {
        id: "CP-7941G-GE",
        display_name: "Cisco 7941G-GE",
        aliases: &["7941GE", "7941G-GE"],
        model_id: 309,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP41", "SIP41"],
    },
    PhoneProfile {
        id: "CP-7961G",
        display_name: "Cisco 7961",
        aliases: &["7961", "7961G"],
        model_id: 30_018,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP41", "SIP41"],
    },
    PhoneProfile {
        id: "CP-7961G-GE",
        display_name: "Cisco 7961G-GE",
        aliases: &["7961GE", "7961G-GE"],
        model_id: 308,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP41", "SIP41"],
    },
    PhoneProfile {
        id: "CP-7942G",
        display_name: "Cisco 7942",
        aliases: &["7942", "7942G"],
        model_id: 434,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP42", "SIP42"],
    },
    PhoneProfile {
        id: "CP-7962G",
        display_name: "Cisco 7962",
        aliases: &["7962", "7962G"],
        model_id: 404,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP42", "SIP42"],
    },
    PhoneProfile {
        id: "CP-7945G",
        display_name: "Cisco 7945",
        aliases: &["7945", "7945G"],
        model_id: 435,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP45", "SIP45"],
    },
    PhoneProfile {
        id: "CP-7965G",
        display_name: "Cisco 7965",
        aliases: &["7965", "7965G"],
        model_id: 436,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP45", "SIP45"],
    },
    PhoneProfile {
        id: "CP-7975G",
        display_name: "Cisco 7975",
        aliases: &["7975", "7975G"],
        model_id: 437,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP75", "SIP75"],
    },
    PhoneProfile {
        id: "CP-6921",
        display_name: "Cisco 6921",
        aliases: &["6921", "Cisco 6921"],
        model_id: 495,
        protocols: BOTH_PROTOCOLS,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["SCCP69", "SIP69"],
    },
    PhoneProfile {
        id: "CP-7821",
        display_name: "Cisco 7821",
        aliases: &["7821", "Cisco 7821"],
        model_id: 621,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip78xx"],
    },
    PhoneProfile {
        id: "CP-7841",
        display_name: "Cisco 7841",
        aliases: &["7841", "Cisco 7841"],
        model_id: 622,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip78xx"],
    },
    PhoneProfile {
        id: "CP-7861",
        display_name: "Cisco 7861",
        aliases: &["7861", "Cisco 7861"],
        model_id: 623,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip78xx"],
    },
    PhoneProfile {
        id: "CP-8811",
        display_name: "Cisco 8811",
        aliases: &["8811", "Cisco 8811"],
        model_id: 36_217,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip88xx"],
    },
    PhoneProfile {
        id: "CP-8821",
        display_name: "Cisco 8821",
        aliases: &["8821", "Cisco 8821"],
        model_id: 36_216,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip8821"],
    },
    PhoneProfile {
        id: "CP-8841",
        display_name: "Cisco 8841",
        aliases: &["8841", "Cisco 8841"],
        model_id: 683,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip88xx"],
    },
    PhoneProfile {
        id: "CP-8845",
        display_name: "Cisco 8845",
        aliases: &["8845", "Cisco 8845"],
        model_id: 36_224,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip8845_65"],
    },
    PhoneProfile {
        id: "CP-8851",
        display_name: "Cisco 8851",
        aliases: &["8851", "Cisco 8851"],
        model_id: 684,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip88xx"],
    },
    PhoneProfile {
        id: "CP-8861",
        display_name: "Cisco 8861",
        aliases: &["8861", "Cisco 8861"],
        model_id: 685,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip88xx"],
    },
    PhoneProfile {
        id: "CP-8865",
        display_name: "Cisco 8865",
        aliases: &["8865", "Cisco 8865"],
        model_id: 36_225,
        protocols: SIP_ONLY,
        dialects: ENTERPRISE_XML,
        load_prefixes: &["sip8845_65"],
    },
];

#[must_use]
pub const fn profiles() -> &'static [PhoneProfile] {
    PHONE_PROFILES
}

/// Find a profile using a canonical model ID or a documented alias.
#[must_use]
pub fn find_profile(model: &PhoneModelId) -> Option<&'static PhoneProfile> {
    let wanted = model.normalized();
    let wanted_without_cp = wanted.strip_prefix("CP").unwrap_or(&wanted);
    PHONE_PROFILES.iter().find(|profile| {
        let canonical = normalize(profile.id);
        canonical == wanted
            || canonical.strip_prefix("CP") == Some(wanted_without_cp)
            || profile.aliases.iter().any(|alias| {
                let alias = normalize(alias);
                alias == wanted || alias == wanted_without_cp
            })
    })
}

/// Resolve a user-supplied model identifier to its generation profile.
#[must_use]
pub fn resolve_profile(model: &PhoneModelId) -> Option<&'static ModelProfile> {
    find_profile(model)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_and_short_model_names_find_the_same_profile() {
        let canonical = find_profile(&"CP-7965G".parse().expect("model"));
        let short = find_profile(&"7965".parse().expect("model"));
        let common = find_profile(&"CP-7965".parse().expect("model"));
        assert_eq!(canonical, short);
        assert_eq!(canonical, common);
        assert_eq!(short.expect("known model").id, "CP-7965G");
    }

    #[test]
    fn catalog_matches_public_model_ids_and_protocols() {
        let expected = [
            ("CP-7940G", 8, BOTH_PROTOCOLS),
            ("CP-7960G", 7, BOTH_PROTOCOLS),
            ("CP-7911G", 307, BOTH_PROTOCOLS),
            ("CP-7941G", 115, BOTH_PROTOCOLS),
            ("CP-7941G-GE", 309, BOTH_PROTOCOLS),
            ("CP-7961G", 30_018, BOTH_PROTOCOLS),
            ("CP-7961G-GE", 308, BOTH_PROTOCOLS),
            ("CP-7942G", 434, BOTH_PROTOCOLS),
            ("CP-7962G", 404, BOTH_PROTOCOLS),
            ("CP-7945G", 435, BOTH_PROTOCOLS),
            ("CP-7965G", 436, BOTH_PROTOCOLS),
            ("CP-7975G", 437, BOTH_PROTOCOLS),
            ("CP-6921", 495, BOTH_PROTOCOLS),
            ("CP-7821", 621, SIP_ONLY),
            ("CP-7841", 622, SIP_ONLY),
            ("CP-7861", 623, SIP_ONLY),
            ("CP-8811", 36_217, SIP_ONLY),
            ("CP-8821", 36_216, SIP_ONLY),
            ("CP-8841", 683, SIP_ONLY),
            ("CP-8845", 36_224, SIP_ONLY),
            ("CP-8851", 684, SIP_ONLY),
            ("CP-8861", 685, SIP_ONLY),
            ("CP-8865", 36_225, SIP_ONLY),
        ];
        assert_eq!(profiles().len(), expected.len());
        for (id, model_id, protocols) in expected {
            let model = find_profile(&id.parse().expect("model")).expect("known model");
            assert_eq!(model.model_id, model_id, "{id}");
            assert_eq!(model.protocols, protocols, "{id}");
        }
    }
}
