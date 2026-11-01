//! Compatibility metadata for Milestone 21.
use serde::{Deserialize, Serialize};

/// Compatibility/runtime metadata for foreign applications (Milestone 21.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompatibilityMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_platform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_compat_apis: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_libraries: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem_paths: Option<Vec<String>>,
    #[serde(default)]
    pub network_access: bool,
    #[serde(default)]
    pub device_access: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_limit_kb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_limit: Option<u32>,
}
