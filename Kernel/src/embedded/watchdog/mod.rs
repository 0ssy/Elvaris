//! Watchdog and health monitoring.
//!
//! Detects system or service hangs and triggers controlled recovery.

pub mod monitor;
pub mod interface;

pub use monitor::HealthMonitor;
pub use interface::{Watchdog, WatchdogState};

use alloc::string::String;
use core::fmt;

/// Watchdog errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogError {
    Unavailable,
    AlreadyArmed,
    NotArmed,
    NotPermitted,
    Expired,
}

impl fmt::Display for WatchdogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchdogError::Unavailable => f.write_str("watchdog unavailable"),
            WatchdogError::AlreadyArmed => f.write_str("watchdog already armed"),
            WatchdogError::NotArmed => f.write_str("watchdog not armed"),
            WatchdogError::NotPermitted => f.write_str("watchdog operation not permitted"),
            WatchdogError::Expired => f.write_str("watchdog expired"),
        }
    }
}
