//! Compatibility subsystem (Milestone 21)
//! Service layer above kernel for foreign runtimes.
use alloc::string::String;
use alloc::vec::Vec;
use spin::{Mutex, Once};

#[derive(Debug, Clone)]
pub struct CompatibilityRuntime {
    pub name: String,
    pub version: String,
    pub enabled: bool,
}

static RUNTIMES: Once<Mutex<Vec<CompatibilityRuntime>>> = Once::new();

pub fn init() {
    RUNTIMES.call_once(|| Mutex::new(Vec::new()));
}

pub fn register(runtime: CompatibilityRuntime) {
    let runtimes = RUNTIMES.call_once(|| Mutex::new(Vec::new()));
    let mut rt = runtimes.lock();
    rt.push(runtime);
}

pub fn list() -> Vec<CompatibilityRuntime> {
    let runtimes = RUNTIMES.call_once(|| Mutex::new(Vec::new()));
    let rt = runtimes.lock();
    rt.clone()
}
pub mod runtimes;

#[cfg(test)]
mod tests;

