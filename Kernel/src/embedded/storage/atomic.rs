//! Atomic state updates.
//!
//! For important state, updates follow a write-temp-flush-verify-activate
//! pattern rather than overwriting in place.

use super::StorageError;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

/// The state of an atomic update operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateState {
    /// No update in progress.
    Idle,
    /// A temporary version is being written.
    WritingTemp,
    /// The temporary version has been flushed to storage.
    Flushed,
    /// The temporary version has been verified.
    Verified,
    /// The update has been activated (renamed into place).
    Activated,
    /// The update failed and was rolled back.
    Failed,
}

/// An atomic state update in progress.
pub struct AtomicStateUpdate {
    /// The path being updated.
    pub target_path: String,
    /// The temporary path used during the update.
    pub temp_path: String,
    /// The current state of the update.
    pub state: UpdateState,
    /// The size of the data being written.
    pub data_size: usize,
    /// The number of bytes written so far.
    pub bytes_written: usize,
    /// Whether a checksum was computed.
    pub checksum_verified: bool,
}

impl AtomicStateUpdate {
    /// Start a new atomic update.
    pub fn new(target_path: &str, temp_path: &str, data_size: usize) -> Self {
        Self {
            target_path: target_path.to_string(),
            temp_path: temp_path.to_string(),
            state: UpdateState::WritingTemp,
            data_size,
            bytes_written: 0,
            checksum_verified: false,
        }
    }

    /// Record that bytes have been written to the temporary version.
    pub fn write_progress(&mut self, bytes: usize) -> Result<(), StorageError> {
        if self.state != UpdateState::WritingTemp {
            return Err(StorageError::WriteFailed);
        }
        self.bytes_written = self.bytes_written.saturating_add(bytes);
        if self.bytes_written > self.data_size {
            return Err(StorageError::InsufficientSpace);
        }
        Ok(())
    }

    /// Mark the temporary version as flushed to storage.
    pub fn mark_flushed(&mut self) -> Result<(), StorageError> {
        if self.state != UpdateState::WritingTemp {
            return Err(StorageError::WriteFailed);
        }
        if self.bytes_written != self.data_size {
            return Err(StorageError::WriteFailed);
        }
        self.state = UpdateState::Flushed;
        Ok(())
    }

    /// Mark the temporary version as verified.
    pub fn mark_verified(&mut self, checksum_ok: bool) -> Result<(), StorageError> {
        if self.state != UpdateState::Flushed {
            return Err(StorageError::WriteFailed);
        }
        self.checksum_verified = checksum_ok;
        if !checksum_ok {
            self.state = UpdateState::Failed;
            return Err(StorageError::CorruptMetadata);
        }
        self.state = UpdateState::Verified;
        Ok(())
    }

    /// Mark the update as activated (temporary version renamed into place).
    pub fn mark_activated(&mut self) -> Result<(), StorageError> {
        if self.state != UpdateState::Verified {
            return Err(StorageError::WriteFailed);
        }
        self.state = UpdateState::Activated;
        Ok(())
    }

    /// Roll back the update: mark as failed.
    pub fn rollback(&mut self) {
        self.state = UpdateState::Failed;
    }

    /// Whether the update completed successfully.
    pub fn is_complete(&self) -> bool {
        self.state == UpdateState::Activated
    }

    /// Whether the update failed.
    pub fn is_failed(&self) -> bool {
        self.state == UpdateState::Failed
    }
}

impl fmt::Display for AtomicStateUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "atomic update {} -> {}: {:?}",
            self.temp_path, self.target_path, self.state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_update_lifecycle() {
        let mut update = AtomicStateUpdate::new(
            "/state/config",
            "/staging/config.tmp",
            1024,
        );
        assert_eq!(update.state, UpdateState::WritingTemp);
        assert!(!update.is_complete());
        assert!(!update.is_failed());

        update.write_progress(1024).unwrap();
        assert_eq!(update.bytes_written, 1024);

        update.mark_flushed().unwrap();
        assert_eq!(update.state, UpdateState::Flushed);

        update.mark_verified(true).unwrap();
        assert_eq!(update.state, UpdateState::Verified);
        assert!(update.checksum_verified);

        update.mark_activated().unwrap();
        assert_eq!(update.state, UpdateState::Activated);
        assert!(update.is_complete());
    }

    #[test]
    fn failed_checksum_marks_update_failed() {
        let mut update = AtomicStateUpdate::new(
            "/state/config",
            "/staging/config.tmp",
            1024,
        );
        update.write_progress(1024).unwrap();
        update.mark_flushed().unwrap();
        let result = update.mark_verified(false);
        assert!(matches!(result, Err(StorageError::CorruptMetadata)));
        assert!(update.is_failed());
    }

    #[test]
    fn incomplete_write_cannot_flush() {
        let mut update = AtomicStateUpdate::new(
            "/state/config",
            "/staging/config.tmp",
            1024,
        );
        update.write_progress(512).unwrap();
        let result = update.mark_flushed();
        assert!(matches!(result, Err(StorageError::WriteFailed)));
    }

    #[test]
    fn rollback_marks_failed() {
        let mut update = AtomicStateUpdate::new(
            "/state/config",
            "/staging/config.tmp",
            1024,
        );
        update.rollback();
        assert!(update.is_failed());
    }

    #[test]
    fn write_progress_exceeds_size_rejected() {
        let mut update = AtomicStateUpdate::new(
            "/state/config",
            "/staging/config.tmp",
            100,
        );
        let result = update.write_progress(200);
        assert!(matches!(result, Err(StorageError::InsufficientSpace)));
    }
}
