//! POSIX compatibility runtime (Milestone 21.3)
#![allow(dead_code)]
use alloc::string::String;
use alloc::vec::Vec;

pub struct PosixRuntime;

impl PosixRuntime {
    pub fn new() -> Self {
        Self
    }
    pub fn supported_calls() -> Vec<String> {
        vec![
            "open".into(), "read".into(), "write".into(), "close".into(),
            "getdents".into(), "stat".into(), "exit".into(), "getenv".into(),
            "time".into(), "socket".into(), "connect".into(), "send".into(), "recv".into(),
        ]
    }
}
