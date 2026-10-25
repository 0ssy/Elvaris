//! Security errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityError {
    InvalidIdentity,
    InvalidResource,
    InvalidPermission,
    PermissionDenied,
    OperationNotSupported,
    NotOwner,
    InsufficientPrivilege,
    ResourceUnavailable,
    BadCredential,
}

impl SecurityError {
    pub fn as_u64(&self) -> u64 {
        match self {
            SecurityError::InvalidIdentity => 1,
            SecurityError::InvalidResource => 2,
            SecurityError::InvalidPermission => 3,
            SecurityError::PermissionDenied => 4,
            SecurityError::OperationNotSupported => 5,
            SecurityError::NotOwner => 6,
            SecurityError::InsufficientPrivilege => 7,
            SecurityError::ResourceUnavailable => 8,
            SecurityError::BadCredential => 9,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SecurityError::InvalidIdentity => "invalid identity",
            SecurityError::InvalidResource => "invalid resource",
            SecurityError::InvalidPermission => "invalid permission",
            SecurityError::PermissionDenied => "permission denied",
            SecurityError::OperationNotSupported => "operation not supported",
            SecurityError::NotOwner => "not owner",
            SecurityError::InsufficientPrivilege => "insufficient privilege",
            SecurityError::ResourceUnavailable => "resource unavailable",
            SecurityError::BadCredential => "bad credential",
        }
    }
}
