//! Kernel-side package support.
//!
//! Provides minimal package parsing and an installed-package database for the
//! Elvaris kernel. This is intentionally small: full manifest parsing, signing,
//! and verification are host-side concerns handled by the `packages` crate and
//! the Elvaris SDK.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use spin::{Once, Mutex};

/// Package magic bytes.
pub const PACKAGE_MAGIC: &[u8; 6] = b"ELVPKG";

/// Package format version.
pub const PACKAGE_FORMAT_VERSION: u32 = 1;

/// Architecture identifiers.
pub const ARCH_X86_64: u32 = 0;
pub const ARCH_AARCH64: u32 = 1;

/// Maximum installed packages.
pub const MAX_INSTALLED_PACKAGES: usize = 64;

/// Error type for kernel package operations.
#[derive(Debug, PartialEq, Eq)]
pub enum PackageError {
    InvalidMagic,
    UnsupportedFormatVersion(u32),
    UnsupportedArchitecture(u32),
    PackageTruncated,
    InvalidPath,
    DuplicatePath,
    PackageDatabaseFull,
    PackageNotFound,
    PackageAlreadyInstalled,
}

/// An installed package record.
#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub identifier: String,
    pub version: String,
    pub architecture: u32,
    pub entrypoint: String,
    pub files: Vec<String>,
    pub signature_verified: bool,
}

/// The installed-package database.
pub struct PackageDatabase {
    packages: Vec<InstalledPackage>,
}

impl PackageDatabase {
    pub fn new() -> Self {
        Self { packages: Vec::new() }
    }

    /// Register an installed package. Returns an error if the database is full
    /// or if a package with the same identifier is already installed.
    pub fn register(
        &mut self,
        identifier: String,
        version: String,
        architecture: u32,
        entrypoint: String,
        files: Vec<String>,
        signature_verified: bool,
    ) -> Result<(), PackageError> {
        if self.packages.len() >= MAX_INSTALLED_PACKAGES {
            return Err(PackageError::PackageDatabaseFull);
        }
        if self
            .packages
            .iter()
            .any(|p| p.identifier == identifier)
        {
            return Err(PackageError::PackageAlreadyInstalled);
        }
        self.packages.push(InstalledPackage {
            identifier,
            version,
            architecture,
            entrypoint,
            files,
            signature_verified,
        });
        Ok(())
    }

    /// Remove an installed package and return its file list.
    pub fn remove(&mut self, identifier: &str) -> Result<Vec<String>, PackageError> {
        let idx = self
            .packages
            .iter()
            .position(|p| p.identifier == identifier)
            .ok_or(PackageError::PackageNotFound)?;
        let package = self.packages.remove(idx);
        Ok(package.files)
    }

    /// Look up an installed package by identifier.
    pub fn find(&self, identifier: &str) -> Option<&InstalledPackage> {
        self.packages.iter().find(|p| p.identifier == identifier)
    }

    /// Check if a path is owned by any installed package.
    pub fn path_owner(&self, path: &str) -> Option<&str> {
        for pkg in &self.packages {
            if pkg.files.iter().any(|f| f == path) {
                return Some(&pkg.identifier);
            }
        }
        None
    }

    /// List all installed package identifiers.
    pub fn list(&self) -> Vec<&str> {
        self.packages.iter().map(|p| p.identifier.as_str()).collect()
    }

    /// Check if a package's signature was verified.
    pub fn is_signature_verified(&self, identifier: &str) -> bool {
        self.packages
            .iter()
            .find(|p| p.identifier == identifier)
            .map_or(false, |p| p.signature_verified)
    }
}

/// Global package database instance.
static PACKAGE_DB: Once<Mutex<PackageDatabase>> = Once::new();

/// Initialize the package subsystem.
pub fn init() {
    // Database starts empty; packages are registered at install time.
    PACKAGE_DB.call_once(|| Mutex::new(PackageDatabase::new()));
}

/// Get the global package database.
pub fn database() -> spin::MutexGuard<'static, PackageDatabase> {
    let db = PACKAGE_DB.call_once(|| Mutex::new(PackageDatabase::new()));
    db.lock()
}

/// Validate a package header (magic, format version, architecture).
pub fn validate_header(data: &[u8]) -> Result<(u32, u32), PackageError> {
    if data.len() < 18 {
        return Err(PackageError::PackageTruncated);
    }
    if &data[0..6] != PACKAGE_MAGIC {
        return Err(PackageError::InvalidMagic);
    }
    let format_version = u32::from_le_bytes([data[6], data[7], data[8], data[9]]);
    if format_version != PACKAGE_FORMAT_VERSION {
        return Err(PackageError::UnsupportedFormatVersion(format_version));
    }
    let architecture = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
    match architecture {
        ARCH_X86_64 | ARCH_AARCH64 => Ok((format_version, architecture)),
        _ => Err(PackageError::UnsupportedArchitecture(architecture)),
    }
}

/// Validate a file path within a package.
pub fn validate_path(path: &str) -> Result<(), PackageError> {
    if path.is_empty() {
        return Err(PackageError::InvalidPath);
    }
    if !path.starts_with('/') {
        return Err(PackageError::InvalidPath);
    }
    if path.contains("..") {
        return Err(PackageError::InvalidPath);
    }
    Ok(())
}
