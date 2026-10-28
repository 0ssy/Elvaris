//! Security and identity foundation for Elvaris.

pub mod identity;
pub mod authz;
pub mod error;
pub mod log;

pub use identity::{Credentials, IdentityDb, ROOT_GID, ROOT_UID, USER_GID, USER_UID};
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
        match &raw const IDENTITY_DB {
            ptr => match *ptr {
                Some(ref db) => db,
                None => panic!("Security not initialized"),
            },
        }
    }
}
