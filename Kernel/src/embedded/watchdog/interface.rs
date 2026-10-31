use super::WatchdogError;
use core::fmt;

/// Watchdog state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogState {
    Unavailable,
    Stopped,
    Armed,
    Healthy,
    Warning,
    Expired,
    Recovering,
}

impl fmt::Display for WatchdogState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchdogState::Unavailable => f.write_str("unavailable"),
            WatchdogState::Stopped => f.write_str("stopped"),
            WatchdogState::Armed => f.write_str("armed"),
            WatchdogState::Healthy => f.write_str("healthy"),
            WatchdogState::Warning => f.write_str("warning"),
            WatchdogState::Expired => f.write_str("expired"),
            WatchdogState::Recovering => f.write_str("recovering"),
        }
    }
}

/// Watchdog interface.
pub struct Watchdog {
    state: WatchdogState,
    timeout_seconds: u64,
    armed: bool,
}

impl Watchdog {
    pub fn new() -> Self {
        Self {
            state: WatchdogState::Stopped,
            timeout_seconds: 30,
            armed: false,
        }
    }

    pub fn state(&self) -> WatchdogState {
        self.state
    }

    pub fn arm(&mut self, timeout_seconds: u64) -> Result<(), WatchdogError> {
        if self.armed {
            return Err(WatchdogError::AlreadyArmed);
        }
        self.timeout_seconds = timeout_seconds;
        self.armed = true;
        self.state = WatchdogState::Armed;
        Ok(())
    }

    pub fn feed(&mut self) -> Result<(), WatchdogError> {
        if !self.armed {
            return Err(WatchdogError::NotArmed);
        }
        self.state = WatchdogState::Healthy;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), WatchdogError> {
        if !self.armed {
            return Err(WatchdogError::NotArmed);
        }
        self.armed = false;
        self.state = WatchdogState::Stopped;
        Ok(())
    }

    pub fn timeout_seconds(&self) -> u64 {
        self.timeout_seconds
    }
}

impl Default for Watchdog {
    fn default() -> Self {
        Self::new()
    }
}
