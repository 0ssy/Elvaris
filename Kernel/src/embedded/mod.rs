//! Embedded system support for Elvaris.
//!
//! Provides the resource-constrained embedded runtime: read-only system and
//! writable state separation, embedded storage behavior, and related
//! subsystems.

pub mod state;
pub mod storage;
pub mod watchdog;
pub mod app;
pub mod update;
pub mod recovery;
pub mod net;
pub mod measure;
