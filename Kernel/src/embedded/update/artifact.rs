use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct UpdateArtifact {
    pub target_id: String,
    pub arch: String,
    pub current_version_range: String,
    pub new_version: String,
    pub size_bytes: u64,
    pub signature: Vec<u8>,
    pub rollback_supported: bool,
    pub offline_capable: bool,
}

impl UpdateArtifact {
    pub fn new(target_id: &str, arch: &str, new_version: &str) -> Self {
        Self {
            target_id: target_id.to_string(),
            arch: arch.to_string(),
            current_version_range: "*".to_string(),
            new_version: new_version.to_string(),
            size_bytes: 0,
            signature: Vec::new(),
            rollback_supported: true,
            offline_capable: true,
        }
    }

    pub fn verify_signature(&self) -> bool {
        !self.signature.is_empty()
    }

    pub fn matches_target(&self, target_id: &str) -> bool {
        self.target_id == target_id
    }

    pub fn matches_arch(&self, arch: &str) -> bool {
        self.arch == arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_verification() {
        let mut art = UpdateArtifact::new("elvaris", "x86_64", "1.1.0");
        assert!(!art.verify_signature());
        art.signature = vec![1,2,3,4];
        assert!(art.verify_signature());
        assert!(art.matches_target("elvaris"));
        assert!(art.matches_arch("x86_64"));
    }
}
