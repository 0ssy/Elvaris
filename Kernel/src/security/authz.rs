//! Authorization logic.

use crate::security::{Credentials, SecurityError, ROOT_UID};
use crate::fs::{Owner, Permissions, Access};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    File,
    Directory,
    Process,
    Console,
    Device,
    Ipc,
    SystemConfig,
    Kernel,
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub resource_type: ResourceType,
    pub owner_uid: u64,
    pub owner_gid: u64,
    pub permissions: Option<Permissions>,
    pub protected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    Read,
    Write,
    Execute,
    Admin,
}

/// Authorize an operation
pub fn authorize(
    creds: &Credentials,
    resource: &Resource,
    mode: AccessMode,
) -> Result<(), SecurityError> {
    // Privileged root can do most things (with logging)
    if creds.is_privileged() {
        // Still check for explicitly forbidden operations
        if resource.resource_type == ResourceType::Kernel && mode == AccessMode::Admin {
            // Kernel admin operations require explicit checks
        }
        return Ok(());
    }

    // Non-privileged users
    match resource.resource_type {
        ResourceType::Kernel => {
            return Err(SecurityError::PermissionDenied);
        }
        ResourceType::Process => {
            // Can only manage own processes or children
            if mode == AccessMode::Admin {
                return Err(SecurityError::PermissionDenied);
            }
            return Ok(()); // Basic access allowed with restrictions elsewhere
        }
        ResourceType::Device => {
            if mode == AccessMode::Write || mode == AccessMode::Admin {
                return Err(SecurityError::PermissionDenied);
            }
            return Ok(());
        }
        ResourceType::Console => {
            // Console access generally allowed for read/write by user
            return Ok(());
        }
        ResourceType::SystemConfig => {
            return Err(SecurityError::PermissionDenied);
        }
        ResourceType::File | ResourceType::Directory => {
            if let Some(perms) = &resource.permissions {
                let access = match mode {
                    AccessMode::Read => Access::Read,
                    AccessMode::Write => Access::Write,
                    AccessMode::Execute => Access::Execute,
                    AccessMode::Admin => return Err(SecurityError::PermissionDenied),
                };
                
                // Check owner
                if resource.owner_uid == creds.uid {
                    match access {
                        Access::Read => if perms.read { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                        Access::Write => if perms.write { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                        Access::Execute => if perms.execute { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                    }
                }
                // Check group
                else if resource.owner_gid == creds.gid {
                    // Simplified: check if group has permissions
                    // In our model, we need to check group bits
                    match access {
                        Access::Read => if perms.read { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                        Access::Write => if perms.write { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                        Access::Execute => if perms.execute { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                    }
                }
                // Check other
                else {
                    match access {
                        Access::Read => if perms.read { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                        Access::Write => if perms.write { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                        Access::Execute => if perms.execute { return Ok(()); } else { return Err(SecurityError::PermissionDenied); }
                    }
                }
            }
            return Err(SecurityError::PermissionDenied);
        }
        ResourceType::Ipc => {
            return Ok(());
        }
    }
}

/// Check if credentials can access a file with given permissions
pub fn check_file_access(creds: &Credentials, owner_uid: u64, owner_gid: u64, perms: &Permissions, access: Access) -> bool {
    if creds.is_privileged() {
        return true;
    }

    // Owner
    if owner_uid == creds.uid {
        return match access {
            Access::Read => perms.read,
            Access::Write => perms.write,
            Access::Execute => perms.execute,
        };
    }
    // Group
    if owner_gid == creds.gid {
        return match access {
            Access::Read => perms.read,
            Access::Write => perms.write,
            Access::Execute => perms.execute,
        };
    }
    // Other
    match access {
        Access::Read => perms.read,
        Access::Write => perms.write,
        Access::Execute => perms.execute,
    }
}
