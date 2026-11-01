//! Windows compatibility research (Milestone 21.8)
#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum WindowsStrategy {
    Porting,
    ApiTranslation,
    ExecutableLoading,
    VirtualMachine,
}

pub struct WindowsResearch {
    pub strategy: WindowsStrategy,
}

impl WindowsResearch {
    pub fn new() -> Self {
        Self { strategy: WindowsStrategy::Porting }
    }
}
