//! User and group identities.

use alloc::vec::Vec;

pub const ROOT_UID: u64 = 0;
pub const ROOT_GID: u64 = 0;
pub const USER_UID: u64 = 1000;
pub const USER_GID: u64 = 1000;
pub const SERVICE_UID: u64 = 65534;
pub const SERVICE_GID: u64 = 65534;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub uid: u64,
    pub gid: u64,
}

impl Identity {
    pub const fn root() -> Self {
        Identity { uid: ROOT_UID, gid: ROOT_GID }
    }

    pub const fn user() -> Self {
        Identity { uid: USER_UID, gid: USER_GID }
    }

    pub const fn service() -> Self {
        Identity { uid: SERVICE_UID, gid: SERVICE_GID }
    }

    pub const fn system() -> Self {
        Identity { uid: ROOT_UID, gid: ROOT_GID }
    }

    pub fn is_privileged(&self) -> bool {
        self.uid == ROOT_UID
    }

    pub fn is_root(&self) -> bool {
        self.uid == ROOT_UID
    }
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub uid: u64,
    pub gid: u64,
    pub supplementary_groups: Vec<u64>,
    pub privileged: bool,
}

impl Credentials {
    pub const fn root() -> Self {
        Credentials {
            uid: ROOT_UID,
            gid: ROOT_GID,
            supplementary_groups: Vec::new(),
            privileged: true,
        }
    }

    pub const fn user() -> Self {
        Credentials {
            uid: USER_UID,
            gid: USER_GID,
            supplementary_groups: Vec::new(),
            privileged: false,
        }
    }

    pub const fn service() -> Self {
        Credentials {
            uid: SERVICE_UID,
            gid: SERVICE_GID,
            supplementary_groups: Vec::new(),
            privileged: false,
        }
    }

    pub fn is_privileged(&self) -> bool {
        self.privileged || self.uid == ROOT_UID
    }

    pub fn is_root(&self) -> bool {
        self.uid == ROOT_UID
    }

    pub fn inherit_from(parent: &Credentials) -> Self {
        Credentials {
            uid: parent.uid,
            gid: parent.gid,
            supplementary_groups: parent.supplementary_groups.clone(),
            privileged: parent.privileged,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IdentityDb {
    pub initialized: bool,
}

impl IdentityDb {
    pub fn new() -> Self {
        IdentityDb { initialized: true }
    }

    pub fn validate_uid(&self, uid: u64) -> bool {
        uid <= 65535 || uid == u64::MAX // allow valid ranges
    }

    pub fn validate_gid(&self, gid: u64) -> bool {
        gid <= 65535 || gid == u64::MAX
    }
}

impl Default for IdentityDb {
    fn default() -> Self {
        Self::new()
    }
}
