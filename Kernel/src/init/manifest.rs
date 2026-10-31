//! Service manifest definition and parsing.
//!
//! A manifest describes one service: what program to run, with what
//! identity, permissions, limits, dependencies, and restart policy.
//!
//! # Manifest format
//!
//! Manifests are simple key-value files with section headers:
//!
//! ```text
//! [service:my-service]
//! executable = /system/bin/my-service
//! arguments = --flag
//! user = service-app
//! group = service-app
//! permissions = console.output, network.client
//! devices =
//! paths = /state/my-service
//! dependencies = filesystem, security
//! memory_limit = 65536
//! handle_limit = 256
//! restart_policy = on_failure
//! max_restarts = 5
//! restart_window_seconds = 30
//! backoff_delay_seconds = 2
//! health_check = alive
//! shutdown_timeout_seconds = 10
//! ```
//!
//! Sections are separated by blank lines. Each service has its own section.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// A service identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(pub usize);

impl ServiceId {
    pub const fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "service-{}", self.0)
    }
}

/// The restart policy of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Never restart the service after it exits.
    Never,
    /// Restart the service when it exits with a non-zero code or a fault.
    OnFailure,
    /// Always restart the service when it exits, for any reason.
    Always,
}

impl RestartPolicy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "never" => Some(RestartPolicy::Never),
            "on_failure" => Some(RestartPolicy::OnFailure),
            "always" => Some(RestartPolicy::Always),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RestartPolicy::Never => "never",
            RestartPolicy::OnFailure => "on_failure",
            RestartPolicy::Always => "always",
        }
    }
}

/// The health-check method of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthCheck {
    /// The service is healthy if its process is alive.
    Alive,
    /// The service must report a heartbeat within the window.
    Heartbeat,
    /// The service must write a readiness marker to its state path.
    ReadinessMarker,
}

impl HealthCheck {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "alive" => Some(HealthCheck::Alive),
            "heartbeat" => Some(HealthCheck::Heartbeat),
            "readiness_marker" => Some(HealthCheck::ReadinessMarker),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HealthCheck::Alive => "alive",
            HealthCheck::Heartbeat => "heartbeat",
            HealthCheck::ReadinessMarker => "readiness_marker",
        }
    }
}

/// The state of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// The manifest has been loaded but the service has not been started.
    Defined,
    /// The service is waiting for its dependencies to become ready.
    WaitingForDependencies,
    /// The service is being started.
    Starting,
    /// The service process is running.
    Running,
    /// The service is being shut down.
    Stopping,
    /// The service has stopped.
    Stopped,
    /// The service failed to start or crashed.
    Failed,
    /// The service is waiting before a restart attempt.
    Restarting,
    /// The service is disabled and will not be started.
    Disabled,
}

impl ServiceState {
    pub fn as_str(self) -> &'static str {
        match self {
            ServiceState::Defined => "defined",
            ServiceState::WaitingForDependencies => "waiting_for_dependencies",
            ServiceState::Starting => "starting",
            ServiceState::Running => "running",
            ServiceState::Stopping => "stopping",
            ServiceState::Stopped => "stopped",
            ServiceState::Failed => "failed",
            ServiceState::Restarting => "restarting",
            ServiceState::Disabled => "disabled",
        }
    }
}

/// Valid state transitions.
impl ServiceState {
    pub fn can_transition_to(self, next: Self) -> bool {
        match self {
            ServiceState::Defined => matches!(
                next,
                ServiceState::WaitingForDependencies
                    | ServiceState::Disabled
                    | ServiceState::Starting
            ),
            ServiceState::WaitingForDependencies => matches!(
                next,
                ServiceState::Starting | ServiceState::Failed | ServiceState::Disabled
            ),
            ServiceState::Starting => matches!(
                next,
                ServiceState::Running | ServiceState::Failed | ServiceState::Stopping
            ),
            ServiceState::Running => matches!(
                next,
                ServiceState::Stopping | ServiceState::Failed | ServiceState::Disabled
            ),
            ServiceState::Stopping => matches!(
                next,
                ServiceState::Stopped | ServiceState::Failed | ServiceState::Disabled
            ),
            ServiceState::Stopped => matches!(
                next,
                ServiceState::Starting | ServiceState::Failed | ServiceState::Disabled
            ),
            ServiceState::Failed => matches!(
                next,
                ServiceState::Restarting | ServiceState::Disabled | ServiceState::Stopped
            ),
            ServiceState::Restarting => matches!(
                next,
                ServiceState::Starting | ServiceState::Failed | ServiceState::Disabled
            ),
            ServiceState::Disabled => matches!(
                next,
                ServiceState::Defined | ServiceState::Stopped
            ),
        }
    }
}

/// A service manifest: the complete declaration of one service.
#[derive(Debug, Clone)]
pub struct ServiceManifest {
    /// The identifier of the service.
    pub name: String,
    /// The executable path of the service program.
    pub executable: String,
    /// The arguments passed to the service program.
    pub arguments: String,
    /// The user identity the service runs as.
    pub user: String,
    /// The group identity the service runs in.
    pub group: String,
    /// The permissions the service is granted.
    pub permissions: Vec<String>,
    /// The devices the service may access.
    pub devices: Vec<String>,
    /// The filesystem paths the service may write.
    pub paths: Vec<String>,
    /// The identifiers of services this service depends on.
    pub dependencies: Vec<String>,
    /// The memory limit in bytes.
    pub memory_limit: usize,
    /// The handle limit.
    pub handle_limit: usize,
    /// The restart policy.
    pub restart_policy: RestartPolicy,
    /// The maximum number of restart attempts.
    pub max_restarts: u64,
    /// The restart window in seconds.
    pub restart_window_seconds: u64,
    /// The backoff delay between restarts in seconds.
    pub backoff_delay_seconds: u64,
    /// The health-check method.
    pub health_check: HealthCheck,
    /// The shutdown timeout in seconds.
    pub shutdown_timeout_seconds: u64,
}

impl ServiceManifest {
    /// Create a manifest with default values.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            executable: String::new(),
            arguments: String::new(),
            user: "service-app".to_string(),
            group: "service-app".to_string(),
            permissions: Vec::new(),
            devices: Vec::new(),
            paths: Vec::new(),
            dependencies: Vec::new(),
            memory_limit: 65536,
            handle_limit: 256,
            restart_policy: RestartPolicy::OnFailure,
            max_restarts: 5,
            restart_window_seconds: 30,
            backoff_delay_seconds: 2,
            health_check: HealthCheck::Alive,
            shutdown_timeout_seconds: 10,
        }
    }

    /// Validate the manifest.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.is_empty() {
            return Err(ManifestError::MissingName);
        }
        if self.executable.is_empty() {
            return Err(ManifestError::MissingExecutable);
        }
        if self.memory_limit == 0 {
            return Err(ManifestError::InvalidMemoryLimit);
        }
        if self.handle_limit == 0 {
            return Err(ManifestError::InvalidHandleLimit);
        }
        Ok(())
    }
}

/// Errors of manifest validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    MissingName,
    MissingExecutable,
    InvalidMemoryLimit,
    InvalidHandleLimit,
}

impl ManifestError {
    pub fn description(self) -> &'static str {
        match self {
            ManifestError::MissingName => "service name is missing",
            ManifestError::MissingExecutable => "service executable is missing",
            ManifestError::InvalidMemoryLimit => "memory limit must be non-zero",
            ManifestError::InvalidHandleLimit => "handle limit must be non-zero",
        }
    }
}

/// Parse a manifest section from lines.
///
/// The first line must be a section header `[service:name]`.
/// Subsequent lines are `key = value` pairs until a blank line or EOF.
pub fn parse_section(lines: &[&str]) -> Result<ServiceManifest, ManifestParseError> {
    if lines.is_empty() {
        return Err(ManifestParseError::EmptySection);
    }

    let header = lines[0].trim();
    let name = header
        .strip_prefix("[service:")
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| ManifestParseError::InvalidHeader(header.to_string()))?
        .trim();

    if name.is_empty() {
        return Err(ManifestParseError::EmptyServiceName);
    }

    let mut manifest = ServiceManifest::new(name);

    for line in &lines[1..] {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();

        match key {
            "executable" => manifest.executable = value.to_string(),
            "arguments" => manifest.arguments = value.to_string(),
            "user" => manifest.user = value.to_string(),
            "group" => manifest.group = value.to_string(),
            "permissions" => {
                manifest.permissions = split_list(value);
            }
            "devices" => {
                manifest.devices = split_list(value);
            }
            "paths" => {
                manifest.paths = split_list(value);
            }
            "dependencies" => {
                manifest.dependencies = split_list(value);
            }
            "memory_limit" => {
                manifest.memory_limit = value
                    .parse::<usize>()
                    .map_err(|_| ManifestParseError::InvalidValue(key.to_string(), value.to_string()))?;
            }
            "handle_limit" => {
                manifest.handle_limit = value
                    .parse::<usize>()
                    .map_err(|_| ManifestParseError::InvalidValue(key.to_string(), value.to_string()))?;
            }
            "restart_policy" => {
                manifest.restart_policy =
                    RestartPolicy::from_str(value).ok_or_else(|| {
                        ManifestParseError::InvalidValue(key.to_string(), value.to_string())
                    })?;
            }
            "max_restarts" => {
                manifest.max_restarts = value
                    .parse::<u64>()
                    .map_err(|_| ManifestParseError::InvalidValue(key.to_string(), value.to_string()))?;
            }
            "restart_window_seconds" => {
                manifest.restart_window_seconds = value
                    .parse::<u64>()
                    .map_err(|_| ManifestParseError::InvalidValue(key.to_string(), value.to_string()))?;
            }
            "backoff_delay_seconds" => {
                manifest.backoff_delay_seconds = value
                    .parse::<u64>()
                    .map_err(|_| ManifestParseError::InvalidValue(key.to_string(), value.to_string()))?;
            }
            "health_check" => {
                manifest.health_check =
                    HealthCheck::from_str(value).ok_or_else(|| {
                        ManifestParseError::InvalidValue(key.to_string(), value.to_string())
                    })?;
            }
            "shutdown_timeout_seconds" => {
                manifest.shutdown_timeout_seconds = value
                    .parse::<u64>()
                    .map_err(|_| ManifestParseError::InvalidValue(key.to_string(), value.to_string()))?;
            }
            _ => {
                // Unknown keys are ignored for forward compatibility.
            }
        }
    }

    manifest.validate().map_err(|_| ManifestParseError::InvalidValue(
        "validation".to_string(),
        "failed".to_string(),
    ))?;
    Ok(manifest)
}

/// Split a comma-separated list into trimmed, non-empty strings.
fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a complete manifest file into a list of manifests.
pub fn parse_file(content: &str) -> Result<Vec<ServiceManifest>, ManifestParseError> {
    let mut manifests: Vec<ServiceManifest> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with("[service:") {
            // Collect all lines of this section until the next section or EOF.
            let mut section: Vec<&str> = Vec::new();
            section.push(line);
            i += 1;
            while i < lines.len() {
                let next = lines[i].trim();
                if next.starts_with("[service:") {
                    break;
                }
                section.push(next);
                i += 1;
            }
            let manifest = parse_section(&section)?;
            manifests.push(manifest);
        } else if line.is_empty() || line.starts_with('#') {
            i += 1;
        } else {
            return Err(ManifestParseError::UnexpectedLine(line.to_string()));
        }
    }

    if manifests.is_empty() {
        return Err(ManifestParseError::NoServices);
    }

    Ok(manifests)
}

/// Errors of manifest parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestParseError {
    EmptySection,
    InvalidHeader(String),
    EmptyServiceName,
    UnexpectedLine(String),
    InvalidValue(String, String),
    NoServices,
}

impl ManifestParseError {
    pub fn description(self) -> &'static str {
        match self {
            ManifestParseError::EmptySection => "empty service section",
            ManifestParseError::InvalidHeader(_) => "invalid section header",
            ManifestParseError::EmptyServiceName => "empty service name",
            ManifestParseError::UnexpectedLine(_) => "unexpected line outside section",
            ManifestParseError::InvalidValue(_, _) => "invalid value",
            ManifestParseError::NoServices => "no services defined",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_MANIFEST: &str = r#"[service:test-service]
executable = /bin/test
user = service-app
dependencies = filesystem
restart_policy = on_failure
"#;

    const FULL_MANIFEST: &str = r#"[service:application]
executable = /system/bin/application
arguments = --flag
user = service-app
group = service-app
permissions = console.output, network.client
devices =
paths = /state/application
dependencies = filesystem, security
memory_limit = 65536
handle_limit = 256
restart_policy = on_failure
max_restarts = 5
restart_window_seconds = 30
backoff_delay_seconds = 2
health_check = alive
shutdown_timeout_seconds = 10

[service:logger]
executable = /system/bin/logger
user = service-logger
dependencies = filesystem
restart_policy = always
max_restarts = 10
health_check = heartbeat
"#;

    #[test]
    fn parses_simple_manifest() {
        let manifests = parse_file(SIMPLE_MANIFEST).unwrap();
        assert_eq!(manifests.len(), 1);
        let m = &manifests[0];
        assert_eq!(m.name, "test-service");
        assert_eq!(m.executable, "/bin/test");
        assert_eq!(m.user, "service-app");
        assert_eq!(m.restart_policy, RestartPolicy::OnFailure);
    }

    #[test]
    fn parses_full_manifest() {
        let manifests = parse_file(FULL_MANIFEST).unwrap();
        assert_eq!(manifests.len(), 2);
        let app = &manifests[0];
        assert_eq!(app.name, "application");
        assert_eq!(app.executable, "/system/bin/application");
        assert_eq!(app.arguments, "--flag");
        assert_eq!(app.permissions, vec!["console.output", "network.client"]);
        assert_eq!(app.dependencies, vec!["filesystem", "security"]);
        assert_eq!(app.memory_limit, 65536);
        assert_eq!(app.handle_limit, 256);
        assert_eq!(app.restart_policy, RestartPolicy::OnFailure);
        assert_eq!(app.max_restarts, 5);
        assert_eq!(app.health_check, HealthCheck::Alive);

        let log = &manifests[1];
        assert_eq!(log.name, "logger");
        assert_eq!(log.restart_policy, RestartPolicy::Always);
        assert_eq!(log.max_restarts, 10);
        assert_eq!(log.health_check, HealthCheck::Heartbeat);
    }

    #[test]
    fn rejects_empty_file() {
        let result = parse_file("");
        assert!(matches!(result, Err(ManifestParseError::NoServices)));
    }

    #[test]
    fn rejects_missing_executable() {
        let content = "[service:bad]\nuser = service-app\n";
        let result = parse_file(content);
        assert!(matches!(
            result,
            Err(ManifestParseError::InvalidValue(_, _))
        ));
    }

    #[test]
    fn validates_state_transitions() {
        assert!(ServiceState::Defined.can_transition_to(ServiceState::Starting));
        assert!(!ServiceState::Defined.can_transition_to(ServiceState::Running));
        assert!(ServiceState::Running.can_transition_to(ServiceState::Failed));
        assert!(ServiceState::Failed.can_transition_to(ServiceState::Restarting));
        assert!(ServiceState::Restarting.can_transition_to(ServiceState::Starting));
    }

    #[test]
    fn restart_policy_from_str() {
        assert_eq!(
            RestartPolicy::from_str("never"),
            Some(RestartPolicy::Never)
        );
        assert_eq!(
            RestartPolicy::from_str("on_failure"),
            Some(RestartPolicy::OnFailure)
        );
        assert_eq!(
            RestartPolicy::from_str("always"),
            Some(RestartPolicy::Always)
        );
        assert_eq!(RestartPolicy::from_str("invalid"), None);
    }
}
