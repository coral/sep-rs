//! Structured validation diagnostics.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

/// A stable machine-readable diagnostic accompanied by human context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }

    pub fn new(severity: Severity, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.into(),
            path: None,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]", self.severity, self.code)?;
        if let Some(path) = &self.path {
            write!(f, " at {path}")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// Frequently emitted diagnostic codes. The `Diagnostic` wire shape retains a
/// string code so future versions can add codes without breaking consumers.
pub mod code {
    pub const MALFORMED: &str = "malformed_artifact";
    pub const FILENAME_MAC_MISMATCH: &str = "filename_mac_mismatch";
    pub const MISSING_ENDPOINT: &str = "missing_endpoint";
    pub const INVALID_PORT: &str = "invalid_port";
    pub const DUPLICATE_PRIORITY: &str = "duplicate_priority";
    pub const DUPLICATE_BUTTON: &str = "duplicate_button";
    pub const DUPLICATE_LINE: &str = "duplicate_line";
    pub const MODEL_PROTOCOL_MISMATCH: &str = "model_protocol_mismatch";
    pub const SECURE_MODE_MISMATCH: &str = "secure_mode_mismatch";
    pub const UNKNOWN_FIELD: &str = "unknown_field";
    pub const UNKNOWN_MODEL: &str = "unknown_model";
    pub const UNKNOWN_FIRMWARE: &str = "unknown_firmware";
    pub const RAW_FEATURE: &str = "raw_feature";
    pub const PLACEHOLDER: &str = "unresolved_placeholder";
    pub const CLEARTEXT_SECRET: &str = "cleartext_secret";
    pub const MISSING_EXTERNAL_ARTIFACT: &str = "missing_external_artifact";
}
