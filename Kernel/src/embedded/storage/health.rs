//! Storage health tracking.
//!
//! Tracks storage health metrics: read errors, write errors, timeouts,
//! available space, mount state, filesystem validation result.

use super::StorageError;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use core::fmt;

/// The health state of a storage area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaHealth {
    /// The area is healthy.
    Healthy,
    /// The area has warnings but is usable.
    Degraded,
    /// The area is unhealthy and should not be used.
    Unhealthy,
    /// The area's health is unknown.
    Unknown,
}

/// Storage health metrics.
pub struct StorageHealth {
    /// Read error count.
    pub read_errors: u64,
    /// Write error count.
    pub write_errors: u64,
    /// Timeout count.
    pub timeouts: u64,
    /// Available space in bytes.
    pub available_space: u64,
    /// Total space in bytes.
    pub total_space: u64,
    /// The health of each storage area.
    pub area_health: BTreeMap<String, AreaHealth>,
    /// Whether the filesystem passed its last validation.
    pub filesystem_valid: bool,
}

impl StorageHealth {
    /// Create a new health tracker.
    pub fn new(total_space: u64) -> Self {
        Self {
            read_errors: 0,
            write_errors: 0,
            timeouts: 0,
            available_space: total_space,
            total_space,
            area_health: BTreeMap::new(),
            filesystem_valid: true,
        }
    }

    /// Record a read error.
    pub fn record_read_error(&mut self) {
        self.read_errors = self.read_errors.saturating_add(1);
    }

    /// Record a write error.
    pub fn record_write_error(&mut self) {
        self.write_errors = self.write_errors.saturating_add(1);
    }

    /// Record a timeout.
    pub fn record_timeout(&mut self) {
        self.timeouts = self.timeouts.saturating_add(1);
    }

    /// Update available space.
    pub fn update_available_space(&mut self, bytes: u64) {
        self.available_space = bytes.min(self.total_space);
    }

    /// Set the health of a storage area.
    pub fn set_area_health(&mut self, area: &str, health: AreaHealth) {
        self.area_health.insert(area.to_string(), health);
    }

    /// The health of a storage area, or Unknown if not tracked.
    pub fn area_health(&self, area: &str) -> AreaHealth {
        self.area_health
            .get(area)
            .copied()
            .unwrap_or(AreaHealth::Unknown)
    }

    /// Whether storage is healthy overall.
    pub fn is_healthy(&self) -> bool {
        self.filesystem_valid
            && self.read_errors == 0
            && self.write_errors == 0
            && self.timeouts == 0
            && self.area_health.values().all(|h| *h != AreaHealth::Unhealthy)
    }

    /// Whether storage is degraded (has warnings but is usable).
    pub fn is_degraded(&self) -> bool {
        !self.is_healthy()
            && self.area_health.values().all(|h| *h != AreaHealth::Unhealthy)
    }

    /// The percentage of space used.
    pub fn usage_percent(&self) -> u8 {
        if self.total_space == 0 {
            return 0;
        }
        let used = self.total_space.saturating_sub(self.available_space);
        (used * 100 / self.total_space) as u8
    }
}

impl fmt::Display for StorageHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "storage health: read_errors={} write_errors={} timeouts={} available={}/{} ({}% used) fs_valid={}",
            self.read_errors,
            self.write_errors,
            self.timeouts,
            self.available_space,
            self.total_space,
            self.usage_percent(),
            self.filesystem_valid
        )
    }
}

impl Default for StorageHealth {
    fn default() -> Self {
        Self::new(32 * 1024 * 1024 * 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_health_is_healthy() {
        let health = StorageHealth::new(1024);
        assert!(health.is_healthy());
        assert!(!health.is_degraded());
    }

    #[test]
    fn read_error_makes_degraded() {
        let mut health = StorageHealth::new(1024);
        health.record_read_error();
        assert!(!health.is_healthy());
        assert!(health.is_degraded());
    }

    #[test]
    fn write_error_makes_degraded() {
        let mut health = StorageHealth::new(1024);
        health.record_write_error();
        assert!(!health.is_healthy());
        assert!(health.is_degraded());
    }

    #[test]
    fn timeout_makes_degraded() {
        let mut health = StorageHealth::new(1024);
        health.record_timeout();
        assert!(!health.is_healthy());
        assert!(health.is_degraded());
    }

    #[test]
    fn unhealthy_area_makes_unhealthy() {
        let mut health = StorageHealth::new(1024);
        health.set_area_health("system", AreaHealth::Unhealthy);
        assert!(!health.is_healthy());
        assert!(!health.is_degraded());
    }

    #[test]
    fn invalid_filesystem_makes_unhealthy() {
        let mut health = StorageHealth::new(1024);
        health.filesystem_valid = false;
        assert!(!health.is_healthy());
    }

    #[test]
    fn usage_percent() {
        let mut health = StorageHealth::new(1000);
        health.update_available_space(300);
        assert_eq!(health.usage_percent(), 70);
    }

    #[test]
    fn area_health_defaults_to_unknown() {
        let health = StorageHealth::new(1024);
        assert_eq!(health.area_health("nonexistent"), AreaHealth::Unknown);
    }
}
