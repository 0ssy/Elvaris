//! Linux compatibility runtime foundation (Milestone 21.4)
#![allow(dead_code)]
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub enum LinuxExecModel {
    Recompile,
    SyscallTranslation,
    Virtualized,
}

pub struct LinuxRuntime {
    pub model: LinuxExecModel,
}

impl LinuxRuntime {
    pub fn new() -> Self {
        Self { model: LinuxExecModel::Recompile }
    }
    pub fn syscall_policy() -> Vec<(String, String)> {
        vec![
            ("read".into(), "Implemented".into()),
            ("write".into(), "Implemented".into()),
            ("open".into(), "Implemented".into()),
            ("close".into(), "Implemented".into()),
            ("exit".into(), "Implemented".into()),
            ("brk".into(), "Translated".into()),
            ("mmap".into(), "Translated".into()),
        ]
    }
}
