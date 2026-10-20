//! Filesystem interface (stub).

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    InvalidPath,
    PermissionDenied,
    IoError,
}

pub trait FileSystem {
    fn mount(&mut self) -> Result<(), FsError>;
}
