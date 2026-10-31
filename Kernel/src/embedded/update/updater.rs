use super::{UpdateArtifact, UpdateError};
use alloc::string::{String, ToString};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePhase {
    Idle,
    Received,
    Verified,
    Staging,
    Staged,
    PendingActivation,
    Activating,
    HealthCheck,
    Confirmed,
    Failed,
    RolledBack,
}

pub struct UpdateManager {
    phase: UpdatePhase,
    pending_version: Option<String>,
    active_version: String,
    previous_known_good: String,
}

impl UpdateManager {
    pub fn new(active_version: &str) -> Self {
        Self {
            phase: UpdatePhase::Idle,
            pending_version: None,
            active_version: active_version.to_string(),
            previous_known_good: active_version.to_string(),
        }
    }

    pub fn phase(&self) -> UpdatePhase {
        self.phase
    }

    pub fn receive(&mut self, artifact: &UpdateArtifact) -> Result<(), UpdateError> {
        if !artifact.verify_signature() {
            return Err(UpdateError::InvalidSignature);
        }
        if !artifact.matches_target("elvaris") {
            return Err(UpdateError::WrongTarget);
        }
        if !artifact.matches_arch("x86_64") {
            return Err(UpdateError::WrongArch);
        }
        self.pending_version = Some(artifact.new_version.clone());
        self.phase = UpdatePhase::Verified;
        Ok(())
    }

    pub fn stage(&mut self) -> Result<(), UpdateError> {
        if self.phase != UpdatePhase::Verified {
            return Err(UpdateError::StagingFailed);
        }
        self.phase = UpdatePhase::Staged;
        Ok(())
    }

    pub fn mark_pending(&mut self) -> Result<(), UpdateError> {
        if self.phase != UpdatePhase::Staged {
            return Err(UpdateError::ActivationFailed);
        }
        self.phase = UpdatePhase::PendingActivation;
        Ok(())
    }

    pub fn confirm(&mut self) -> Result<(), UpdateError> {
        if self.phase != UpdatePhase::HealthCheck {
            return Err(UpdateError::NotPending);
        }
        if let Some(v) = self.pending_version.take() {
            self.previous_known_good = self.active_version.clone();
            self.active_version = v;
        }
        self.phase = UpdatePhase::Confirmed;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<(), UpdateError> {
        if self.phase == UpdatePhase::Idle {
            return Err(UpdateError::NotPending);
        }
        self.active_version = self.previous_known_good.clone();
        self.pending_version = None;
        self.phase = UpdatePhase::RolledBack;
        Ok(())
    }

    pub fn active_version(&self) -> &str {
        &self.active_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_flow() {
        let mut mgr = UpdateManager::new("1.0.0");
        let mut art = UpdateArtifact::new("elvaris", "x86_64", "1.1.0");
        art.signature = vec![1,2,3];
        mgr.receive(&art).unwrap();
        assert_eq!(mgr.phase(), UpdatePhase::Verified);
        mgr.stage().unwrap();
        mgr.mark_pending().unwrap();
        mgr.phase = UpdatePhase::HealthCheck;
        mgr.confirm().unwrap();
        assert_eq!(mgr.active_version(), "1.1.0");
    }

    #[test]
    fn rollback_on_failure() {
        let mut mgr = UpdateManager::new("1.0.0");
        let mut art = UpdateArtifact::new("elvaris", "x86_64", "1.1.0");
        art.signature = vec![1,2,3];
        mgr.receive(&art).unwrap();
        mgr.stage().unwrap();
        mgr.mark_pending().unwrap();
        mgr.rollback().unwrap();
        assert_eq!(mgr.phase(), UpdatePhase::RolledBack);
        assert_eq!(mgr.active_version(), "1.0.0");
    }
}
