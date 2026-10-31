//! Signed embedded updates.

pub mod artifact;
pub mod updater;

pub use artifact::UpdateArtifact;
pub use updater::UpdateManager;

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateError {
    InvalidSignature,
    WrongTarget,
    WrongArch,
    VersionNotAllowed,
    InsufficientSpace,
    StagingFailed,
    VerificationFailed,
    ActivationFailed,
    NotPending,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::InvalidSignature => f.write_str("invalid update signature"),
            UpdateError::WrongTarget => f.write_str("wrong update target"),
            UpdateError::WrongArch => f.write_str("wrong architecture"),
            UpdateError::VersionNotAllowed => f.write_str("version not allowed"),
            UpdateError::InsufficientSpace => f.write_str("insufficient space"),
            UpdateError::StagingFailed => f.write_str("staging failed"),
            UpdateError::VerificationFailed => f.write_str("verification failed"),
            UpdateError::ActivationFailed => f.write_str("activation failed"),
            UpdateError::NotPending => f.write_str("no pending update"),
        }
    }
}
