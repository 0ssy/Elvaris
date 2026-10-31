//! Storage failure classes and responses.
//!
//! Distinguishes different storage failure modes and defines the defined
//! response for each.

use super::StorageError;
use alloc::string::{String, ToString};
use core::fmt;

/// A classified storage failure with its context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageFailure {
    /// The error classification.
    pub error: StorageError,
    /// The storage area affected.
    pub area: Option<String>,
    /// The operation that was in progress.
    pub operation: String,
    /// Whether the failure is recoverable without intervention.
    pub recoverable: bool,
    /// Whether the failure requires entering safe/recovery mode.
    pub requires_recovery_mode: bool,
}

impl StorageFailure {
    /// Create a new storage failure.
    pub fn new(
        error: StorageError,
        area: Option<&str>,
        operation: &str,
    ) -> Self {
        let recoverable = matches!(
            error,
            StorageError::Timeout | StorageError::ReadFailed
        );
        let requires_recovery_mode = matches!(
            error,
            StorageError::CorruptMetadata
                | StorageError::CorruptFilesystem
                | StorageError::Unavailable
        );

        Self {
            error,
            area: area.map(|s| s.to_string()),
            operation: operation.to_string(),
            recoverable,
            requires_recovery_mode,
        }
    }

    /// A short description of the failure.
    pub fn description(&self) -> &'static str {
        self.error.description()
    }
}

impl fmt::Display for StorageFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error.description())
    }
}

/// The response policy for a storage failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureResponse {
    /// Retry the operation.
    Retry,
    /// Retry with backoff.
    RetryWithBackoff,
    /// Fail the current operation but continue.
    FailOperation,
    /// Enter degraded mode.
    DegradedMode,
    /// Enter safe/recovery mode.
    RecoveryMode,
    /// Halt the system.
    Halt,
}

/// Determine the response for a storage failure.
pub fn response_for_failure(failure: &StorageFailure) -> FailureResponse {
    match failure.error {
        StorageError::Timeout => FailureResponse::RetryWithBackoff,
        StorageError::ReadFailed => FailureResponse::Retry,
        StorageError::WriteFailed => FailureResponse::FailOperation,
        StorageError::CorruptMetadata => FailureResponse::RecoveryMode,
        StorageError::CorruptFilesystem => FailureResponse::RecoveryMode,
        StorageError::Unavailable => FailureResponse::RecoveryMode,
        StorageError::InsufficientSpace => FailureResponse::FailOperation,
        StorageError::InvalidPartition => FailureResponse::Halt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_recoverable() {
        let failure = StorageFailure::new(
            StorageError::Timeout,
            Some("state"),
            "write",
        );
        assert!(failure.recoverable);
        assert!(!failure.requires_recovery_mode);
    }

    #[test]
    fn corrupt_filesystem_requires_recovery() {
        let failure = StorageFailure::new(
            StorageError::CorruptFilesystem,
            Some("system"),
            "mount",
        );
        assert!(!failure.recoverable);
        assert!(failure.requires_recovery_mode);
    }

    #[test]
    fn unavailable_requires_recovery() {
        let failure = StorageFailure::new(
            StorageError::Unavailable,
            None,
            "discover",
        );
        assert!(!failure.recoverable);
        assert!(failure.requires_recovery_mode);
    }

    #[test]
    fn response_mapping() {
        assert_eq!(
            response_for_failure(&StorageFailure::new(
                StorageError::Timeout, None, "op"
            )),
            FailureResponse::RetryWithBackoff
        );
        assert_eq!(
            response_for_failure(&StorageFailure::new(
                StorageError::CorruptFilesystem, None, "op"
            )),
            FailureResponse::RecoveryMode
        );
        assert_eq!(
            response_for_failure(&StorageFailure::new(
                StorageError::InvalidPartition, None, "op"
            )),
            FailureResponse::Halt
        );
    }

    #[test]
    fn failure_description() {
        let failure = StorageFailure::new(
            StorageError::ReadFailed,
            Some("state"),
            "read",
        );
        assert_eq!(failure.description(), "storage read failed");
    }
}
