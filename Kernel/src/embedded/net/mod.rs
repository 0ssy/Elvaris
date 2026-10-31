//! Embedded networking profile.

pub mod profile;

pub use profile::EmbeddedNetProfile;

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetProfileError {
    NotConfigured,
    LinkDown,
    BindFailed,
    PermissionDenied,
    RequiredNetworkMissing,
}

impl fmt::Display for NetProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetProfileError::NotConfigured => f.write_str("network not configured"),
            NetProfileError::LinkDown => f.write_str("network link down"),
            NetProfileError::BindFailed => f.write_str("bind failed"),
            NetProfileError::PermissionDenied => f.write_str("network permission denied"),
            NetProfileError::RequiredNetworkMissing => f.write_str("required network missing"),
        }
    }
}
