//! Write protection for the immutable system area.
//!
//! The WriteProtector enforces that the system area (/system) cannot be
//! modified by ordinary applications. Only the system administrator identity
//! may write to system paths, and only during system initialization.

use super::layout::StorageArea;
use super::StateError;
use crate::fs::Identity;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;

/// Errors of the write protector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionError {
    /// The path is in the protected system area.
    ProtectedPath,
    /// The identity is not authorized to write to system paths.
    UnauthorizedIdentity,
    /// The path is not under a known storage area.
    UnknownPath,
}

impl ProtectionError {
    pub fn description(self) -> &'static str {
        match self {
            ProtectionError::ProtectedPath => "path is in the protected system area",
            ProtectionError::UnauthorizedIdentity => "identity not authorized for system write",
            ProtectionError::UnknownPath => "path is not under a known storage area",
        }
    }
}

impl fmt::Display for ProtectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

/// Enforces write protection on storage areas.
pub struct WriteProtector {
    /// Paths that are protected (system area paths).
    protected_paths: BTreeSet<String>,
    /// Whether system initialization is still in progress (allows system
    /// writes during boot).
    system_initializing: bool,
    /// Identities authorized to write to protected paths during init.
    authorized_init_identities: BTreeSet<u64>,
}

impl WriteProtector {
    /// Create a new write protector.
    pub fn new() -> Self {
        let mut protected_paths = BTreeSet::new();
        protected_paths.insert("/system".to_string());
        protected_paths.insert("/system/bin".to_string());
        protected_paths.insert("/system/lib".to_string());
        protected_paths.insert("/system/config".to_string());
        protected_paths.insert("/system/services".to_string());

        let mut authorized = BTreeSet::new();
        authorized.insert(0); // root UID

        Self {
            protected_paths,
            system_initializing: true,
            authorized_init_identities: authorized,
        }
    }

    /// Whether a path is under the protected system area.
    pub fn is_protected(&self, path: &str) -> bool {
        for prefix in &self.protected_paths {
            if path == *prefix || path.starts_with(&format!("{}/", prefix)) {
                return true;
            }
        }
        false
    }

    /// Check whether an identity may write to a path.
    ///
    /// Returns an error if the path is protected and the identity is not
    /// authorized. Returns Ok(()) if the path is not protected or the write
    /// is allowed.
    pub fn check_write(
        &self,
        path: &str,
        identity: &Identity,
    ) -> Result<(), ProtectionError> {
        if !self.is_protected(path) {
            return Ok(());
        }

        // Path is protected.
        if self.system_initializing
            && self.authorized_init_identities.contains(&identity.uid)
        {
            return Ok(());
        }

        Err(ProtectionError::ProtectedPath)
    }

    /// Check whether an identity may create an object under a path.
    pub fn check_create(
        &self,
        path: &str,
        identity: &Identity,
    ) -> Result<(), ProtectionError> {
        self.check_write(path, identity)
    }

    /// Mark system initialization as complete. After this, only authorized
    /// identities may write to protected paths.
    pub fn complete_initialization(&mut self) {
        self.system_initializing = false;
    }

    /// Whether system initialization is still in progress.
    pub fn is_initializing(&self) -> bool {
        self.system_initializing
    }

    /// Add an identity authorized to write to protected paths during init.
    pub fn authorize_init_identity(&mut self, uid: u64) {
        self.authorized_init_identities.insert(uid);
    }

    /// The protected paths managed by this protector.
    pub fn protected_paths(&self) -> impl Iterator<Item = &String> {
        self.protected_paths.iter()
    }
}

impl Default for WriteProtector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::Identity;

    #[test]
    fn system_paths_are_protected() {
        let protector = WriteProtector::new();
        assert!(protector.is_protected("/system"));
        assert!(protector.is_protected("/system/bin"));
        assert!(protector.is_protected("/system/bin/sh"));
        assert!(protector.is_protected("/system/config"));
        assert!(protector.is_protected("/system/services"));
    }

    #[test]
    fn state_paths_are_not_protected() {
        let protector = WriteProtector::new();
        assert!(!protector.is_protected("/state"));
        assert!(!protector.is_protected("/state/application"));
        assert!(!protector.is_protected("/state/logs"));
    }

    #[test]
    fn root_can_write_during_init() {
        let protector = WriteProtector::new();
        let root = Identity { uid: 0, gid: 0 };
        assert!(protector.check_write("/system/bin", &root).is_ok());
    }

    #[test]
    fn user_cannot_write_system_during_init() {
        let protector = WriteProtector::new();
        let user = Identity { uid: 1000, gid: 1000 };
        assert!(matches!(
            protector.check_write("/system/bin", &user),
            Err(ProtectionError::ProtectedPath)
        ));
    }

    #[test]
    fn after_init_root_cannot_write_system() {
        let mut protector = WriteProtector::new();
        protector.complete_initialization();
        let root = Identity { uid: 0, gid: 0 };
        assert!(matches!(
            protector.check_write("/system/bin", &root),
            Err(ProtectionError::ProtectedPath)
        ));
    }

    #[test]
    fn state_writes_always_allowed() {
        let mut protector = WriteProtector::new();
        protector.complete_initialization();
        let user = Identity { uid: 1000, gid: 1000 };
        assert!(protector.check_write("/state/application", &user).is_ok());
    }
}
