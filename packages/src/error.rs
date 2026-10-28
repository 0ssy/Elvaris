//! Package error types.

use thiserror::Error;

/// Errors that can occur when working with Elvaris packages.
#[derive(Debug, Error)]
pub enum PackageError {
    #[error("invalid magic bytes: expected ELVPKG, found {0:?}")]
    InvalidMagic(Vec<u8>),
    #[error("unsupported package format version: {0}")]
    UnsupportedFormatVersion(u32),
    #[error("unsupported architecture: {0}")]
    UnsupportedArchitecture(String),
    #[error("manifest too large: {0} bytes (max {1})")]
    ManifestTooLarge(usize, usize),
    #[error("invalid manifest JSON: {0}")]
    InvalidManifestJson(#[from] serde_json::Error),
    #[error("manifest validation failed: {0}")]
    ManifestValidation(String),
    #[error("file count exceeds maximum: {0} (max {1})")]
    FileCountExceedsMaximum(usize, usize),
    #[error("duplicate file path: {0}")]
    DuplicateFilePath(String),
    #[error("invalid file path: {0}")]
    InvalidFilePath(String),
    #[error("path traversal detected: {0}")]
    PathTraversal(String),
    #[error("file size exceeds maximum: {0} bytes")]
    FileSizeExceedsMaximum(u64),
    #[error("checksum mismatch for file: {0}")]
    ChecksumMismatch(String),
    #[error("package too large: {0} bytes (max {1})")]
    PackageTooLarge(u64, u64),
    #[error("package truncated: expected {0} bytes, found {1}")]
    PackageTruncated(usize, usize),
    #[error("signature missing")]
    SignatureMissing,
    #[error("invalid signature algorithm: {0}")]
    InvalidSignatureAlgorithm(u8),
    #[error("signature verification failed")]
    SignatureVerificationFailed,
    #[error("unknown signing key: {0}")]
    UnknownSigningKey(String),
    #[error("dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

/// Result type for package operations.
pub type Result<T> = std::result::Result<T, PackageError>;
