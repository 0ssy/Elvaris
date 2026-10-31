use alloc::string::{String, ToString};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootMode {
    Normal,
    Safe,
    Recovery,
    FactoryReset,
}

#[derive(Debug, Clone)]
pub struct RecoveryManager {
    mode: BootMode,
    boot_attempts: u32,
    boot_attempt_threshold: u32,
    has_previous_image: bool,
    last_reset_reason: String,
}

impl RecoveryManager {
    pub fn new() -> Self {
        Self {
            mode: BootMode::Normal,
            boot_attempts: 0,
            boot_attempt_threshold: 3,
            has_previous_image: true,
            last_reset_reason: "clean".to_string(),
        }
    }

    pub fn mode(&self) -> BootMode {
        self.mode
    }

    pub fn boot_attempts(&self) -> u32 {
        self.boot_attempts
    }

    pub fn record_boot_attempt(&mut self) {
        self.boot_attempts += 1;
        if self.boot_attempts >= self.boot_attempt_threshold {
            self.enter_recovery();
        }
    }

    pub fn record_successful_boot(&mut self) {
        self.boot_attempts = 0;
        self.mode = BootMode::Normal;
    }

    pub fn enter_safe_mode(&mut self) {
        self.mode = BootMode::Safe;
    }

    pub fn enter_recovery(&mut self) {
        self.mode = BootMode::Recovery;
    }

    pub fn can_rollback(&self) -> bool {
        self.has_previous_image && matches!(self.mode, BootMode::Safe | BootMode::Recovery)
    }

    pub fn rollback_to_previous(&mut self) -> Result<(), super::RecoveryError> {
        if !self.can_rollback() {
            return Err(super::RecoveryError::NoPreviousImage);
        }
        self.mode = BootMode::Recovery;
        Ok(())
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_loop_detection() {
        let mut rm = RecoveryManager::new();
        assert_eq!(rm.mode(), BootMode::Normal);
        rm.record_boot_attempt();
        rm.record_boot_attempt();
        assert_eq!(rm.mode(), BootMode::Normal);
        rm.record_boot_attempt();
        assert_eq!(rm.mode(), BootMode::Recovery);
    }

    #[test]
    fn successful_boot_resets() {
        let mut rm = RecoveryManager::new();
        rm.record_boot_attempt();
        rm.record_boot_attempt();
        rm.record_successful_boot();
        assert_eq!(rm.boot_attempts(), 0);
        assert_eq!(rm.mode(), BootMode::Normal);
    }
}
