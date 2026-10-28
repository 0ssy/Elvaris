//! Application manifest types and validation.

use serde::{Deserialize, Serialize};

use crate::error::{PackageError, Result};

/// Package architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Architecture {
    X86_64,
    AArch64,
}

impl Architecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            Architecture::X86_64 => "x86_64",
            Architecture::AArch64 => "aarch64",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "x86_64" => Ok(Architecture::X86_64),
            "aarch64" => Ok(Architecture::AArch64),
            _ => Err(PackageError::UnsupportedArchitecture(s.to_string())),
        }
    }
}

/// A package dependency constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub identifier: String,
    pub constraint: String,
}

/// A declared permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Permission {
    ConsoleInput,
    ConsoleOutput,
    FilesystemRead,
    FilesystemWrite,
    NetworkClient,
    NetworkServer,
    ProcessInspect,
    DeviceAccess,
    SystemShutdown,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::ConsoleInput => "console.input",
            Permission::ConsoleOutput => "console.output",
            Permission::FilesystemRead => "filesystem.read",
            Permission::FilesystemWrite => "filesystem.write",
            Permission::NetworkClient => "network.client",
            Permission::NetworkServer => "network.server",
            Permission::ProcessInspect => "process.inspect",
            Permission::DeviceAccess => "device.access",
            Permission::SystemShutdown => "system.shutdown",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "console.input" => Ok(Permission::ConsoleInput),
            "console.output" => Ok(Permission::ConsoleOutput),
            "filesystem.read" => Ok(Permission::FilesystemRead),
            "filesystem.write" => Ok(Permission::FilesystemWrite),
            "network.client" => Ok(Permission::NetworkClient),
            "network.server" => Ok(Permission::NetworkServer),
            "process.inspect" => Ok(Permission::ProcessInspect),
            "device.access" => Ok(Permission::DeviceAccess),
            "system.shutdown" => Ok(Permission::SystemShutdown),
            _ => Err(PackageError::ManifestValidation(format!(
                "unknown permission: {s}"
            ))),
        }
    }
}

/// The application manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub identifier: String,
    pub version: String,
    pub architecture: Architecture,
    pub entrypoint: String,
    pub runtime: String,
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<Permission>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    pub manifest_version: u32,
}

impl Manifest {
    /// Current manifest format version.
    pub const MANIFEST_VERSION: u32 = 1;

    /// Create a new manifest with default values.
    pub fn new(
        name: String,
        identifier: String,
        version: String,
        architecture: Architecture,
        entrypoint: String,
    ) -> Self {
        Self {
            name,
            identifier,
            version,
            architecture,
            entrypoint,
            runtime: "none".to_string(),
            files: Vec::new(),
            permissions: Vec::new(),
            dependencies: Vec::new(),
            manifest_version: Self::MANIFEST_VERSION,
        }
    }

    /// Validate the manifest. Returns an error if any field is invalid.
    pub fn validate(&self) -> Result<()> {
        self.validate_name()?;
        self.validate_identifier()?;
        self.validate_version()?;
        self.validate_architecture()?;
        self.validate_entrypoint()?;
        self.validate_runtime()?;
        self.validate_files()?;
        self.validate_permissions()?;
        self.validate_dependencies()?;
        self.validate_manifest_version()?;
        Ok(())
    }

    fn validate_name(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(PackageError::ManifestValidation(
                "name is required".to_string(),
            ));
        }
        if self.name.len() > 256 {
            return Err(PackageError::ManifestValidation(
                "name exceeds 256 characters".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_identifier(&self) -> Result<()> {
        if self.identifier.is_empty() {
            return Err(PackageError::ManifestValidation(
                "identifier is required".to_string(),
            ));
        }
        // Reverse-domain identifier: must contain at least one dot
        if !self.identifier.contains('.') {
            return Err(PackageError::ManifestValidation(
                "identifier must be a reverse-domain name (contain at least one dot)"
                    .to_string(),
            ));
        }
        for ch in self.identifier.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '.' && ch != '-' && ch != '_' {
                return Err(PackageError::ManifestValidation(format!(
                    "invalid character in identifier: {ch}"
                )));
            }
        }
        Ok(())
    }

    fn validate_version(&self) -> Result<()> {
        if self.version.is_empty() {
            return Err(PackageError::ManifestValidation(
                "version is required".to_string(),
            ));
        }
        // Simple semantic version check: major.minor.patch
        let parts: Vec<&str> = self.version.split('.').collect();
        if parts.len() != 3 {
            return Err(PackageError::ManifestValidation(format!(
                "invalid version format (expected major.minor.patch): {}",
                self.version
            )));
        }
        for part in &parts {
            if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
                return Err(PackageError::ManifestValidation(format!(
                    "invalid version component: {part}"
                )));
            }
        }
        Ok(())
    }

    fn validate_architecture(&self) -> Result<()> {
        match self.architecture {
            Architecture::X86_64 | Architecture::AArch64 => Ok(()),
        }
    }

    fn validate_entrypoint(&self) -> Result<()> {
        if self.entrypoint.is_empty() {
            return Err(PackageError::ManifestValidation(
                "entrypoint is required".to_string(),
            ));
        }
        if !self.entrypoint.starts_with('/') {
            return Err(PackageError::ManifestValidation(format!(
                "entrypoint must be an absolute path: {}",
                self.entrypoint
            )));
        }
        if self.entrypoint.contains("..") {
            return Err(PackageError::PathTraversal(self.entrypoint.clone()));
        }
        Ok(())
    }

    fn validate_runtime(&self) -> Result<()> {
        if self.runtime != "none" {
            return Err(PackageError::ManifestValidation(format!(
                "unsupported runtime: {}",
                self.runtime
            )));
        }
        Ok(())
    }

    fn validate_files(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for path in &self.files {
            if path.is_empty() {
                return Err(PackageError::InvalidFilePath(
                    "empty file path".to_string(),
                ));
            }
            if !path.starts_with('/') {
                return Err(PackageError::InvalidFilePath(format!(
                    "file path must be absolute: {path}"
                )));
            }
            if path.contains("..") {
                return Err(PackageError::PathTraversal(path.clone()));
            }
            if !seen.insert(path.clone()) {
                return Err(PackageError::DuplicateFilePath(path.clone()));
            }
        }
        if self.files.len() > 1024 {
            return Err(PackageError::FileCountExceedsMaximum(
                self.files.len(),
                1024,
            ));
        }
        Ok(())
    }

    fn validate_permissions(&self) -> Result<()> {
        // Permissions are validated by from_str during deserialization.
        // Additional checks can go here.
        Ok(())
    }

    fn validate_dependencies(&self) -> Result<()> {
        if self.dependencies.len() > 64 {
            return Err(PackageError::ManifestValidation(
                "too many dependencies (max 64)".to_string(),
            ));
        }
        for dep in &self.dependencies {
            if dep.identifier.is_empty() {
                return Err(PackageError::ManifestValidation(
                    "dependency identifier is required".to_string(),
                ));
            }
            if dep.constraint.is_empty() {
                return Err(PackageError::ManifestValidation(
                    "dependency constraint is required".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn validate_manifest_version(&self) -> Result<()> {
        if self.manifest_version != Self::MANIFEST_VERSION {
            return Err(PackageError::ManifestValidation(format!(
                "unsupported manifest version: {} (expected {})",
                self.manifest_version, Self::MANIFEST_VERSION
            )));
        }
        Ok(())
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new(
            "unnamed".to_string(),
            "org.elvaris.unnamed".to_string(),
            "0.0.1".to_string(),
            Architecture::X86_64,
            "/bin/unnamed".to_string(),
        )
    }
}
