//! Rollback, safe mode, and recovery.

pub mod modes;

pub use modes::{BootMode, RecoveryManager};

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    NotInRecovery,
    BootLoopExceeded,
    NoPreviousImage,
    RecoveryFailed,
    NotAuthorized,
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoveryError::NotInRecovery => f.write_str("not in recovery mode"),
            RecoveryError::BootLoopExceeded => f.write_str("boot loop exceeded"),
            RecoveryError::NoPreviousImage => f.write_str("no previous image available"),
            RecoveryError::RecoveryFailed => f.write_str("recovery failed"),
            RecoveryError::NotAuthorized => f.write_str("recovery not authorized"),
        }
    }
}
