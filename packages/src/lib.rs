//! Elvaris package format library.
//!
//! Provides manifest types, package creation/reading, checksums, and
//! ed25519 signing and verification for `.elvpkg` packages.

pub mod error;
pub mod manifest;
pub mod package;
pub mod signing;

pub use error::{PackageError, Result};
pub use manifest::{Architecture, Dependency, Manifest, Permission};
pub use package::{FileEntry, Package, PackageBuilder, PackageReader};
pub use signing::{KeyPair, PublicKey, Signature, sign_package, verify_package_signature};
