//! Embedded application contract.

pub mod contract;

pub use contract::{AppContract, AppState, AppResourceLimits};

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppError {
    InvalidManifest,
    PermissionDenied,
    ResourceLimit,
    NotFound,
    Failed,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::InvalidManifest => f.write_str("invalid app manifest"),
            AppError::PermissionDenied => f.write_str("permission denied"),
            AppError::ResourceLimit => f.write_str("resource limit exceeded"),
            AppError::NotFound => f.write_str("app not found"),
            AppError::Failed => f.write_str("app failed"),
        }
    }
}
