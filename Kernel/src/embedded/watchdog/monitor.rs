use super::{Watchdog, WatchdogState};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// Health check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Warning,
    Failed,
}

/// Health monitor.
pub struct HealthMonitor {
    checks: Vec<String>,
    watchdog: Watchdog,
    boot_loop_count: u32,
    boot_loop_threshold: u32,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            watchdog: Watchdog::new(),
            boot_loop_count: 0,
            boot_loop_threshold: 3,
        }
    }

    pub fn add_check(&mut self, name: &str) {
        self.checks.push(name.to_string());
    }

    pub fn check_count(&self) -> usize {
        self.checks.len()
    }

    pub fn arm_watchdog(&mut self, timeout: u64) -> Result<(), super::WatchdogError> {
        self.watchdog.arm(timeout)
    }

    pub fn feed_watchdog(&mut self) -> Result<(), super::WatchdogError> {
        self.watchdog.feed()
    }

    pub fn watchdog_state(&self) -> WatchdogState {
        self.watchdog.state()
    }

    pub fn boot_loop_count(&self) -> u32 {
        self.boot_loop_count
    }

    pub fn boot_loop_threshold(&self) -> u32 {
        self.boot_loop_threshold
    }

    pub fn record_boot(&mut self) {
        self.boot_loop_count += 1;
    }

    pub fn reset_boot_count(&mut self) {
        self.boot_loop_count = 0;
    }

    pub fn should_enter_safe_mode(&self) -> bool {
        self.boot_loop_count >= self.boot_loop_threshold
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}
