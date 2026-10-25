//! Security tests for Milestone 14.

#[cfg(test)]
mod tests {
    use crate::security::{Credentials, ROOT_UID, USER_UID};
    use crate::security::authz::{authorize, AccessMode, Resource, ResourceType};
    use crate::security::error::SecurityError;
    use crate::fs::{Permissions, Access};

    #[test]
    fn root_is_privileged() {
        let creds = Credentials::root();
        assert!(creds.is_privileged());
        assert_eq!(creds.uid, ROOT_UID);
    }

    #[test]
    fn user_is_not_privileged() {
        let creds = Credentials::user();
        assert!(!creds.is_privileged());
        assert_eq!(creds.uid, USER_UID);
    }

    #[test]
    fn credentials_inherit() {
        let parent = Credentials::root();
        let child = Credentials::inherit_from(&parent);
        assert_eq!(child.uid, parent.uid);
        assert_eq!(child.privileged, parent.privileged);
    }

    #[test]
    fn root_can_access_kernel() {
        let creds = Credentials::root();
        let resource = Resource {
            resource_type: ResourceType::Kernel,
            owner_uid: 0,
            owner_gid: 0,
            permissions: None,
            protected: true,
        };
        assert!(authorize(&creds, &resource, AccessMode::Admin).is_ok());
    }

    #[test]
    fn user_cannot_access_kernel() {
        let creds = Credentials::user();
        let resource = Resource {
            resource_type: ResourceType::Kernel,
            owner_uid: 0,
            owner_gid: 0,
            permissions: None,
            protected: true,
        };
        assert_eq!(authorize(&creds, &resource, AccessMode::Admin), Err(SecurityError::PermissionDenied));
    }
}
