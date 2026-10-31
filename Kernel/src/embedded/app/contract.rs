use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Installed,
    Validated,
    Starting,
    Running,
    Ready,
    Degraded,
    Stopping,
    Stopped,
    Failed,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct AppResourceLimits {
    pub max_memory: usize,
    pub max_handles: usize,
    pub max_processes: usize,
    pub max_threads: usize,
    pub max_storage: usize,
    pub max_log_volume: usize,
    pub max_network_sockets: usize,
}

impl Default for AppResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 65536,
            max_handles: 256,
            max_processes: 1,
            max_threads: 4,
            max_storage: 1024 * 1024,
            max_log_volume: 64 * 1024,
            max_network_sockets: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppContract {
    pub app_id: String,
    pub version: String,
    pub arch: String,
    pub permissions: Vec<String>,
    pub devices: Vec<String>,
    pub state_paths: Vec<String>,
    pub limits: AppResourceLimits,
    pub health_check: String,
    pub restart_policy: String,
    pub state: AppState,
}

impl AppContract {
    pub fn new(app_id: &str, version: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            version: version.to_string(),
            arch: "x86_64".to_string(),
            permissions: Vec::new(),
            devices: Vec::new(),
            state_paths: Vec::new(),
            limits: AppResourceLimits::default(),
            health_check: "alive".to_string(),
            restart_policy: "on_failure".to_string(),
            state: AppState::Installed,
        }
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.iter().any(|p| p == perm)
    }

    pub fn can_access_state_path(&self, path: &str) -> bool {
        self.state_paths.iter().any(|p| path.starts_with(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_contract_basics() {
        let mut app = AppContract::new("test-app", "1.0.0");
        app.permissions.push("console.output".to_string());
        app.state_paths.push("/state/applications/test-app".to_string());
        assert!(app.has_permission("console.output"));
        assert!(!app.has_permission("network.server"));
        assert!(app.can_access_state_path("/state/applications/test-app/data"));
    }
}
