//! Service supervisor: starts, monitors, restarts, and shuts down services.
//!
//! The supervisor manages the lifecycle of embedded services. It reads
//! service manifests, resolves dependencies, starts services in order,
//! restarts them according to policy, monitors health, and shuts down in
//! reverse dependency order.

use super::manifest::{
    HealthCheck, RestartPolicy, ServiceManifest, ServiceState,
};
use crate::serial;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Errors of the supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorError {
    /// A service with that name is not known.
    UnknownService,
    /// A dependency of a service is missing.
    MissingDependency,
    /// Services have a circular dependency.
    CircularDependency,
    /// A service failed to start.
    StartFailed,
    /// A service failed its health check.
    HealthFailed,
    /// The supervisor is not running.
    NotRunning,
    /// The supervisor is already running.
    AlreadyRunning,
    /// A service manifest is invalid.
    InvalidManifest,
}

impl SupervisorError {
    pub fn description(self) -> &'static str {
        match self {
            SupervisorError::UnknownService => "unknown service",
            SupervisorError::MissingDependency => "missing dependency",
            SupervisorError::CircularDependency => "circular dependency",
            SupervisorError::StartFailed => "service start failed",
            SupervisorError::HealthFailed => "service health check failed",
            SupervisorError::NotRunning => "supervisor not running",
            SupervisorError::AlreadyRunning => "supervisor already running",
            SupervisorError::InvalidManifest => "invalid service manifest",
        }
    }
}

impl fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

/// The recorded runtime state of one service.
#[derive(Debug, Clone)]
pub struct ServiceRecord {
    /// The manifest of the service.
    pub manifest: ServiceManifest,
    /// The current state of the service.
    pub state: ServiceState,
    /// The number of restart attempts made so far.
    pub restart_count: u64,
    /// The timestamp (in ticks) of the last failure.
    pub last_failure_tick: u64,
    /// The timestamp (in ticks) of the last successful health check.
    pub last_healthy_tick: u64,
    /// The process identifier of the running service, if any.
    pub pid: Option<u64>,
    /// The exit status of the last run, if any.
    pub last_exit: Option<u64>,
    /// Whether the service is currently healthy.
    pub healthy: bool,
}

/// The supervisor of all services.
pub struct ServiceSupervisor {
    /// The services, indexed by name.
    services: BTreeMap<String, ServiceRecord>,
    /// The order in which services should be started.
    start_order: Vec<String>,
    /// Whether the supervisor has been started.
    running: bool,
    /// Whether the supervisor is shutting down.
    shutting_down: bool,
    /// The current tick count (mock time for restart backoff).
    tick: u64,
    /// The next process identifier to assign.
    next_pid_counter: u64,
}

impl ServiceSupervisor {
    /// Create a new supervisor from a list of manifests.
    pub fn new(manifests: Vec<ServiceManifest>) -> Result<Self, SupervisorError> {
        if manifests.is_empty() {
            return Err(SupervisorError::InvalidManifest);
        }

        let mut services: BTreeMap<String, ServiceRecord> = BTreeMap::new();
        for manifest in manifests {
            let name = manifest.name.clone();
            services.insert(
                name,
                ServiceRecord {
                    manifest,
                    state: ServiceState::Defined,
                    restart_count: 0,
                    last_failure_tick: 0,
                    last_healthy_tick: 0,
                    pid: None,
                    last_exit: None,
                    healthy: false,
                },
            );
        }

        let start_order = resolve_start_order(&services)?;

        Ok(Self {
            services,
            start_order,
            running: false,
            shutting_down: false,
            tick: 0,
            next_pid_counter: 101,
        })
    }

    /// Whether the supervisor is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Whether the supervisor is shutting down.
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down
    }

    /// The number of services managed.
    pub fn service_count(&self) -> usize {
        self.services.len()
    }

    /// The record of a service, if known.
    pub fn service(&self, name: &str) -> Option<&ServiceRecord> {
        self.services.get(name)
    }

    /// The names of all services, in start order.
    pub fn service_names(&self) -> Vec<String> {
        self.start_order.clone()
    }

    /// The state of a service, if known.
    pub fn service_state(&self, name: &str) -> Option<ServiceState> {
        self.services.get(name).map(|r| r.state)
    }

    /// Whether a service is running.
    pub fn is_running_service(&self, name: &str) -> bool {
        matches!(
            self.services.get(name).map(|r| r.state),
            Some(ServiceState::Running)
        )
    }

    /// Whether a service is healthy.
    pub fn is_healthy(&self, name: &str) -> bool {
        self.services
            .get(name)
            .map(|r| r.healthy)
            .unwrap_or(false)
    }

    /// Whether all services are healthy.
    pub fn all_healthy(&self) -> bool {
        self.services.values().all(|r| r.healthy)
    }

    /// Advance the supervisor tick (used for restart backoff timing).
    pub fn tick(&mut self) {
        self.tick = self.tick.saturating_add(1);
    }

    /// The current tick count.
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Start all services in dependency order.
    ///
    /// This marks each service as started and records its process identifier.
    /// Services that fail to start are marked failed and may be restarted
    /// according to policy.
    pub fn start_services(&mut self) -> Result<(), SupervisorError> {
        if self.running {
            return Err(SupervisorError::AlreadyRunning);
        }
        self.running = true;

        // Clone names first to avoid borrowing self.start_order immutably
        // while calling start_service which borrows self mutably.
        let names: Vec<String> = self.start_order.clone();
        for name in names {
            self.start_service(&name)?;
        }

        Ok(())
    }

    /// Start one service by name.
    fn start_service(&mut self, name: &str) -> Result<(), SupervisorError> {
        // Check dependencies are satisfied first (immutable borrow).
        let deps_satisfied = {
            let record = self
                .services
                .get(name)
                .ok_or(SupervisorError::UnknownService)?;
            for dep in &record.manifest.dependencies {
                let dep_record = self
                    .services
                    .get(dep)
                    .ok_or(SupervisorError::MissingDependency)?;
                if dep_record.state != ServiceState::Running {
                    return Err(SupervisorError::MissingDependency);
                }
            }
            true
        };
        let _ = deps_satisfied;

        // Now mutate the record. Compute the PID before borrowing mutably.
        let next_pid = self.next_pid_counter;
        self.next_pid_counter += 1;
        let record = self
            .services
            .get_mut(name)
            .ok_or(SupervisorError::UnknownService)?;
        record.pid = Some(next_pid);
        record.last_exit = None;
        record.state = ServiceState::Running;
        record.healthy = true;
        record.last_healthy_tick = self.tick;

        if cfg!(not(test)) {
            unsafe {
                serial::write_str("Elvaris kernel: init: service started: ");
                serial::write_str(name);
                serial::write_str(" pid=");
                serial::write_fmt(core::format_args!("{}", record.pid.unwrap()));
                serial::write_newline();
            }
        }

        Ok(())
    }

    /// Stop all services in reverse dependency order.
    pub fn stop_services(&mut self) -> Result<(), SupervisorError> {
        if !self.running {
            return Err(SupervisorError::NotRunning);
        }
        self.shutting_down = true;

        // Clone names to avoid borrowing self.start_order immutably
        // while calling stop_service which borrows self mutably.
        let names: Vec<String> = self.start_order.iter().rev().cloned().collect();
        for name in names {
            self.stop_service(&name)?;
        }

        self.running = false;
        self.shutting_down = false;
        Ok(())
    }

    /// Stop one service by name.
    fn stop_service(&mut self, name: &str) -> Result<(), SupervisorError> {
        let record = self
            .services
            .get_mut(name)
            .ok_or(SupervisorError::UnknownService)?;

        if record.state == ServiceState::Stopped
            || record.state == ServiceState::Disabled
        {
            return Ok(());
        }

        record.state = ServiceState::Stopping;
        record.pid = None;
        record.state = ServiceState::Stopped;
        record.healthy = false;

        if cfg!(not(test)) {
            unsafe {
                serial::write_str("Elvaris kernel: init: service stopped: ");
                serial::write_str(name);
                serial::write_newline();
            }
        }

        Ok(())
    }

    /// Record that a service exited, possibly triggering a restart.
    pub fn service_exited(
        &mut self,
        name: &str,
        exit_code: u64,
    ) -> Result<(), SupervisorError> {
        let record = self
            .services
            .get_mut(name)
            .ok_or(SupervisorError::UnknownService)?;

        record.last_exit = Some(exit_code);
        record.pid = None;
        record.healthy = false;

        let failed = exit_code != 0;
        if failed {
            record.state = ServiceState::Failed;
            record.last_failure_tick = self.tick;
            record.restart_count += 1;

            if cfg!(not(test)) {
                unsafe {
                    serial::write_str("Elvaris kernel: init: service failed: ");
                    serial::write_str(name);
                    serial::write_str(" exit=");
                    serial::write_fmt(core::format_args!("{}", exit_code));
                    serial::write_str(" restarts=");
                    serial::write_fmt(core::format_args!("{}", record.restart_count));
                    serial::write_newline();
                }
            }

            // Apply restart policy.
            let policy = record.manifest.restart_policy;
            if policy == RestartPolicy::Never {
                record.state = ServiceState::Disabled;
                return Ok(());
            }

            if record.restart_count >= record.manifest.max_restarts {
                record.state = ServiceState::Disabled;

            if cfg!(not(test)) {
                unsafe {
                    serial::write_str("Elvaris kernel: init: service disabled: ");
                    serial::write_str(name);
                    serial::write_str(" restart limit reached");
                    serial::write_newline();
                }
            }
                return Ok(());
            }

            if policy == RestartPolicy::OnFailure || policy == RestartPolicy::Always {
                record.state = ServiceState::Restarting;
            }
        } else {
            // Clean exit with code 0.
            match record.manifest.restart_policy {
                RestartPolicy::Never => {
                    record.state = ServiceState::Stopped;
                }
                RestartPolicy::Always => {
                    record.state = ServiceState::Restarting;
                }
                RestartPolicy::OnFailure => {
                    record.state = ServiceState::Stopped;
                }
            }
        }

        Ok(())
    }

    /// Attempt to restart a service that is in the Restarting state.
    pub fn try_restart(&mut self, name: &str) -> Result<(), SupervisorError> {
        let (backoff_met, deps_satisfied) = {
            let record = self
                .services
                .get(name)
                .ok_or(SupervisorError::UnknownService)?;
            if record.state != ServiceState::Restarting {
                return Ok(());
            }
            let elapsed = self.tick.saturating_sub(record.last_failure_tick);
            let backoff_met = elapsed >= record.manifest.backoff_delay_seconds;
            let mut deps_ok = true;
            for dep in &record.manifest.dependencies {
                let dep_record = self
                    .services
                    .get(dep)
                    .ok_or(SupervisorError::MissingDependency)?;
                if dep_record.state != ServiceState::Running {
                    deps_ok = false;
                    break;
                }
            }
            (backoff_met, deps_ok)
        };

        if !backoff_met || !deps_satisfied {
            return Ok(());
        }

        let record = self
            .services
            .get_mut(name)
            .ok_or(SupervisorError::UnknownService)?;
        let next_pid = self.next_pid_counter;
        self.next_pid_counter += 1;
        record.state = ServiceState::Starting;
        record.pid = Some(next_pid);
        record.last_exit = None;
        record.state = ServiceState::Running;
        record.healthy = true;
        record.last_healthy_tick = self.tick;

        if cfg!(not(test)) {
            unsafe {
                serial::write_str("Elvaris kernel: init: service restarted: ");
                serial::write_str(name);
                serial::write_newline();
            }
        }

        Ok(())
    }

    /// Record a health-check result for a service.
    pub fn report_health(&mut self, name: &str, healthy: bool) -> Result<(), SupervisorError> {
        let record = self
            .services
            .get_mut(name)
            .ok_or(SupervisorError::UnknownService)?;

        record.healthy = healthy;
        if healthy {
            record.last_healthy_tick = self.tick;
        } else if record.state == ServiceState::Running {
            record.state = ServiceState::Failed;
            record.last_failure_tick = self.tick;
            record.restart_count += 1;

            if cfg!(not(test)) {
                unsafe {
                    serial::write_str("Elvaris kernel: init: service unhealthy: ");
                    serial::write_str(name);
                    serial::write_newline();
                }
            }
        }

        Ok(())
    }
}

/// Resolve the start order of services using topological sort.
///
/// Returns the names of services in dependency order, or an error if there
/// is a circular dependency or a missing dependency.
fn resolve_start_order(
    services: &BTreeMap<String, ServiceRecord>,
) -> Result<Vec<String>, SupervisorError> {
    let mut visited: BTreeMap<String, bool> = BTreeMap::new();
    let mut in_stack: BTreeMap<String, bool> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();

    fn visit(
        name: &str,
        services: &BTreeMap<String, ServiceRecord>,
        visited: &mut BTreeMap<String, bool>,
        in_stack: &mut BTreeMap<String, bool>,
        order: &mut Vec<String>,
    ) -> Result<(), SupervisorError> {
        if *in_stack.get(name).unwrap_or(&false) {
            return Err(SupervisorError::CircularDependency);
        }
        if *visited.get(name).unwrap_or(&false) {
            return Ok(());
        }

        let record = services
            .get(name)
            .ok_or(SupervisorError::UnknownService)?;

        in_stack.insert(name.to_string(), true);

        for dep in &record.manifest.dependencies {
            if !services.contains_key(dep) {
                return Err(SupervisorError::MissingDependency);
            }
            visit(dep, services, visited, in_stack, order)?;
        }

        in_stack.insert(name.to_string(), false);
        visited.insert(name.to_string(), true);
        order.push(name.to_string());

        Ok(())
    }

    // Visit services in name order for deterministic output.
    let mut names: Vec<&String> = services.keys().collect();
    names.sort();

    for name in names {
        visit(name, services, &mut visited, &mut in_stack, &mut order)?;
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest(name: &str, deps: &[&str], policy: RestartPolicy) -> ServiceManifest {
        let mut m = ServiceManifest::new(name);
        m.dependencies = deps.iter().map(|s| s.to_string()).collect();
        m.restart_policy = policy;
        m
    }

    #[test]
    fn creates_supervisor() {
        let manifests = vec![
            test_manifest("filesystem", &[], RestartPolicy::Never),
            test_manifest("security", &["filesystem"], RestartPolicy::Never),
        ];
        let sup = ServiceSupervisor::new(manifests).unwrap();
        assert_eq!(sup.service_count(), 2);
        assert!(!sup.is_running());
    }

    #[test]
    fn resolves_start_order() {
        let manifests = vec![
            test_manifest("security", &["filesystem"], RestartPolicy::Never),
            test_manifest("filesystem", &[], RestartPolicy::Never),
            test_manifest("app", &["security", "filesystem"], RestartPolicy::Never),
        ];
        let sup = ServiceSupervisor::new(manifests).unwrap();
        let order = sup.service_names();
        // filesystem must come before security, security before app.
        let fs_idx = order.iter().position(|n| n == "filesystem").unwrap();
        let sec_idx = order.iter().position(|n| n == "security").unwrap();
        let app_idx = order.iter().position(|n| n == "app").unwrap();
        assert!(fs_idx < sec_idx);
        assert!(sec_idx < app_idx);
    }

    #[test]
    fn rejects_circular_dependency() {
        let manifests = vec![
            test_manifest("a", &["b"], RestartPolicy::Never),
            test_manifest("b", &["a"], RestartPolicy::Never),
        ];
        let result = ServiceSupervisor::new(manifests);
        assert!(matches!(
            result,
            Err(SupervisorError::CircularDependency)
        ));
    }

    #[test]
    fn rejects_missing_dependency() {
        let manifests = vec![test_manifest("a", &["missing"], RestartPolicy::Never)];
        let result = ServiceSupervisor::new(manifests);
        assert!(matches!(
            result,
            Err(SupervisorError::MissingDependency)
        ));
    }

    #[test]
    fn start_and_stop_services() {
        let manifests = vec![
            test_manifest("filesystem", &[], RestartPolicy::Never),
            test_manifest("app", &["filesystem"], RestartPolicy::Never),
        ];
        let mut sup = ServiceSupervisor::new(manifests).unwrap();
        sup.start_services().unwrap();
        assert!(sup.is_running_service("filesystem"));
        assert!(sup.is_running_service("app"));
        assert!(sup.is_healthy("filesystem"));
        assert!(sup.is_healthy("app"));

        sup.stop_services().unwrap();
        assert!(!sup.is_running());
        assert!(!sup.is_running_service("filesystem"));
        assert!(!sup.is_running_service("app"));
    }

    #[test]
    fn service_exited_triggers_restart() {
        let mut m = test_manifest("app", &[], RestartPolicy::OnFailure);
        m.max_restarts = 3;
        let mut sup = ServiceSupervisor::new(vec![m]).unwrap();
        sup.start_services().unwrap();

        sup.service_exited("app", 1).unwrap();
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Restarting)
        );
        assert_eq!(sup.service("app").unwrap().restart_count, 1);
    }

    #[test]
    fn restart_limit_disables_service() {
        let mut m = test_manifest("app", &[], RestartPolicy::OnFailure);
        m.max_restarts = 1;
        let mut sup = ServiceSupervisor::new(vec![m]).unwrap();
        sup.start_services().unwrap();

        sup.service_exited("app", 1).unwrap();
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Disabled)
        );
    }

    #[test]
    fn never_restart_policy_stops_service() {
        let mut m = test_manifest("app", &[], RestartPolicy::Never);
        let mut sup = ServiceSupervisor::new(vec![m]).unwrap();
        sup.start_services().unwrap();

        sup.service_exited("app", 1).unwrap();
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Disabled)
        );
    }

    #[test]
    fn always_restart_policy_restarts_on_clean_exit() {
        let mut m = test_manifest("app", &[], RestartPolicy::Always);
        m.max_restarts = 5;
        let mut sup = ServiceSupervisor::new(vec![m]).unwrap();
        sup.start_services().unwrap();

        sup.service_exited("app", 0).unwrap();
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Restarting)
        );
    }

    #[test]
    fn health_check_marks_unhealthy() {
        let mut m = test_manifest("app", &[], RestartPolicy::Never);
        let mut sup = ServiceSupervisor::new(vec![m]).unwrap();
        sup.start_services().unwrap();

        sup.report_health("app", false).unwrap();
        assert!(!sup.is_healthy("app"));
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Failed)
        );
    }

    #[test]
    fn try_restart_respects_backoff() {
        let mut m = test_manifest("app", &[], RestartPolicy::OnFailure);
        m.max_restarts = 5;
        m.backoff_delay_seconds = 10;
        let mut sup = ServiceSupervisor::new(vec![m]).unwrap();
        sup.start_services().unwrap();

        sup.service_exited("app", 1).unwrap();
        // Not enough time has passed for backoff.
        sup.try_restart("app").unwrap();
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Restarting)
        );

        // Advance tick past backoff.
        for _ in 0..15 {
            sup.tick();
        }
        sup.try_restart("app").unwrap();
        assert_eq!(
            sup.service_state("app"),
            Some(ServiceState::Running)
        );
    }

    #[test]
    fn all_healthy_tracks_all_services() {
        let manifests = vec![
            test_manifest("a", &[], RestartPolicy::Never),
            test_manifest("b", &[], RestartPolicy::Never),
        ];
        let mut sup = ServiceSupervisor::new(manifests).unwrap();
        sup.start_services().unwrap();
        assert!(sup.all_healthy());

        sup.report_health("b", false).unwrap();
        assert!(!sup.all_healthy());
    }
}
