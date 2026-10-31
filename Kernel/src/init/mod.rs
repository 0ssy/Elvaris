//! Minimal init and service supervision for embedded profiles.
//!
//! The supervisor is the first user-mode program of an embedded Elvaris
//! system. It reads service manifests, starts services in dependency order,
//! restarts them according to policy, monitors health, and shuts down in
//! order.
//!
//! # Startup path
//!
//! ```text
//! Kernel
//!     ↓
//! Filesystem
//!     ↓
//! Security initialization
//!     ↓
//! Minimal init
//!     ↓
//! Service configuration
//!     ↓
//! Required service
//!     ↓
//! Health state
//! ```

pub mod manifest;
pub mod supervisor;

pub use manifest::{
    ServiceManifest, ServiceId, ServiceState, RestartPolicy, HealthCheck,
};
pub use supervisor::ServiceSupervisor;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Errors of the init subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    /// A manifest could not be parsed.
    InvalidManifest,
    /// A service is not known.
    UnknownService,
    /// A dependency of a service is missing.
    MissingDependency,
    /// Services have a circular dependency.
    CircularDependency,
    /// A service failed to start.
    StartFailed,
    /// A service failed health check.
    HealthFailed,
    /// The supervisor is already running.
    AlreadyRunning,
}

impl InitError {
    pub fn description(self) -> &'static str {
        match self {
            InitError::InvalidManifest => "invalid service manifest",
            InitError::UnknownService => "unknown service",
            InitError::MissingDependency => "missing dependency",
            InitError::CircularDependency => "circular dependency",
            InitError::StartFailed => "service start failed",
            InitError::HealthFailed => "service health check failed",
            InitError::AlreadyRunning => "supervisor already running",
        }
    }
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}
