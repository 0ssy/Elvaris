//! Security and identity foundation for Elvaris.

use alloc::vec::Vec;

pub mod identity;
pub mod authz;
pub mod error;
pub mod log;

pub use identity::{Credentials, GroupId, Identity, IdentityDb, UserId, ROOT_GID, ROOT_UID, USER_GID, USER_UID, SERVICE_UID};
pub use authz::{authorize, AccessMode, Resource, ResourceType};
pub use error::SecurityError;
pub use log::{log_event, SecurityEvent};

/// Global identity database
static mut IDENTITY_DB: Option<IdentityDb> = None;

/// Initialize the security subsystem
pub fn init() {
    unsafe {
        IDENTITY_DB = Some(IdentityDb::new());
    }
    log_event(SecurityEvent {
        event_type: "security_init",
        pid: None,
        uid: Some(ROOT_UID),
        resource_type: None,
        operation: "init",
        result: "allowed",
    });
}

/// Get the global identity database
pub fn identity_db() -> &'static IdentityDb {
    unsafe {
        IDENTITY_DB.as_ref().expect("Security not initialized")
    }
}
