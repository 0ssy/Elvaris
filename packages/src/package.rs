//! Package archive creation and reading.

use crate::error::{PackageError, Result};
use crate::manifest::{Architecture, Manifest};
use crate::signing::{KeyPair, PublicKey, Signature};
use sha2::{Digest, Sha256};

/// Package format version.
pub const PACKAGE_FORMAT_VERSION: u32 = 1;

/// Package magic bytes.
pub const PACKAGE_MAGIC: &[u8; 6] = b"ELVPKG";

/// Signature algorithm identifiers.
pub const SIG_NONE: u8 = 0;
pub const SIG_ED25519: u8 = 1;

/// Maximum package size (64 MiB).
pub const MAX_PACKAGE_SIZE: u64 = 64 * 1024 * 1024;

/// Maximum manifest size (64 KiB).
pub const MAX_MANIFEST_SIZE: usize = 64 * 1024;

/// Maximum file count.
pub const MAX_FILE_COUNT: usize = 1024;

/// Maximum path length.
pub const MAX_PATH_LENGTH: usize = 4096;

/// A file entry within a package.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub checksum: u64,
    pub permissions: u32,
    pub file_type: u8,
}

/// A parsed package.
#[derive(Debug, Clone)]
pub struct Package {
    pub manifest: Manifest,
    pub files: Vec<FileEntry>,
    pub file_contents: Vec<(String, Vec<u8>)>,
    pub signature: Option<Signature>,
    pub signature_algorithm: u8,
    pub public_key: Option<PublicKey>,
}

impl Package {
    /// Return the total size of all file contents.
    pub fn content_size(&self) -> u64 {
        self.file_contents.iter().map(|(_, c)| c.len() as u64).sum()
    }

    /// Find a file entry by path.
    pub fn find_file(&self, path: &str) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.path == path)
    }

    /// Get the contents of a file by path.
    pub fn file_data(&self, path: &str) -> Option<&[u8]> {
        self.file_contents.iter().find_map(|(p, data)| {
            if p == path {
                Some(data.as_slice())
            } else {
                None
            }
        })
    }
}

/// Builder for creating packages.
pub struct PackageBuilder {
    manifest: Manifest,
    files: Vec<(String, Vec<u8>, u32, u8)>, // path, data, permissions, type
    key_pair: Option<KeyPair>,
}

impl PackageBuilder {
    pub fn new(manifest: Manifest) -> Self {
        Self {
            manifest,
            files: Vec::new(),
            key_pair: None,
        }
    }

    /// Add a file to the package.
    pub fn add_file(
        mut self,
        path: String,
        data: Vec<u8>,
        permissions: u32,
        file_type: u8,
    ) -> Self {
        self.files.push((path, data, permissions, file_type));
        self
    }

    /// Set the signing key pair.
    pub fn sign_with(mut self, key_pair: KeyPair) -> Self {
        self.key_pair = Some(key_pair);
        self
    }

    /// Build the package bytes.
    pub fn build(mut self) -> Result<Vec<u8>> {
        self.manifest.validate()?;

        // Sort files deterministically by path
        self.files.sort_by(|a, b| a.0.cmp(&b.0));

        // Validate paths
        for (path, ..) in &self.files {
            validate_path(path)?;
        }

        // Serialize manifest
        let manifest_bytes =
            serde_json::to_vec(&self.manifest).map_err(|e| PackageError::InvalidManifestJson(e))?;
        if manifest_bytes.len() > MAX_MANIFEST_SIZE {
            return Err(PackageError::ManifestTooLarge(
                manifest_bytes.len(),
                MAX_MANIFEST_SIZE,
            ));
        }

        if self.files.len() > MAX_FILE_COUNT {
            return Err(PackageError::FileCountExceedsMaximum(
                self.files.len(),
                MAX_FILE_COUNT,
            ));
        }

        // Build the package
        let mut buf = Vec::new();

        // Magic
        buf.extend_from_slice(PACKAGE_MAGIC);
        // Format version
        buf.extend_from_slice(&PACKAGE_FORMAT_VERSION.to_le_bytes());
        // Architecture
        let arch_val = match self.manifest.architecture {
            Architecture::X86_64 => 0u32,
            Architecture::AArch64 => 1u32,
        };
        buf.extend_from_slice(&arch_val.to_le_bytes());
        // Manifest length
        buf.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
        // Manifest
        buf.extend_from_slice(&manifest_bytes);
        // File count
        buf.extend_from_slice(&(self.files.len() as u32).to_le_bytes());

        // File entries and contents
        let mut entries_buf = Vec::new();
        let mut contents_buf = Vec::new();

        for (path, data, permissions, file_type) in &self.files {
            if data.len() > MAX_PACKAGE_SIZE as usize {
                return Err(PackageError::FileSizeExceedsMaximum(data.len() as u64));
            }

            let path_bytes = path.as_bytes();
            if path_bytes.len() > MAX_PATH_LENGTH {
                return Err(PackageError::InvalidFilePath(format!(
                    "path exceeds {MAX_PATH_LENGTH} bytes: {path}"
                )));
            }

            // Checksum: first 8 bytes of SHA-256
            let mut hasher = Sha256::new();
            hasher.update(data);
            let hash = hasher.finalize();
            let checksum = u64::from_le_bytes([hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7]]);

            entries_buf.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            entries_buf.extend_from_slice(path_bytes);
            entries_buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
            entries_buf.extend_from_slice(&checksum.to_le_bytes());
            entries_buf.extend_from_slice(&permissions.to_le_bytes());
            entries_buf.extend_from_slice(&file_type.to_le_bytes());

            contents_buf.extend_from_slice(data);
        }

        buf.extend_from_slice(&entries_buf);
        buf.extend_from_slice(&contents_buf);

        // Sign if key pair provided
        if let Some(key_pair) = &self.key_pair {
            let signature = key_pair.sign(&buf);
            buf.extend_from_slice(&signature.as_bytes());
            buf.push(SIG_ED25519);
        } else {
            // No signature: 64 zero bytes + algorithm 0
            buf.extend_from_slice(&[0u8; 64]);
            buf.push(SIG_NONE);
        }

        if buf.len() as u64 > MAX_PACKAGE_SIZE {
            return Err(PackageError::PackageTooLarge(
                buf.len() as u64,
                MAX_PACKAGE_SIZE,
            ));
        }

        Ok(buf)
    }
}

/// Reader for parsing packages from bytes.
pub struct PackageReader;

impl PackageReader {
    /// Parse a package from a byte slice.
    pub fn read(data: &[u8]) -> Result<Package> {
        if data.len() < 18 {
            return Err(PackageError::PackageTruncated(18, data.len()));
        }

        // Magic
        if &data[0..6] != PACKAGE_MAGIC {
            return Err(PackageError::InvalidMagic(data[0..6].to_vec()));
        }

        // Format version
        let format_version = u32::from_le_bytes([data[6], data[7], data[8], data[9]]);
        if format_version != PACKAGE_FORMAT_VERSION {
            return Err(PackageError::UnsupportedFormatVersion(format_version));
        }

        // Architecture
        let arch_val = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
        let _ = match arch_val {
            0 => Architecture::X86_64,
            1 => Architecture::AArch64,
            _ => return Err(PackageError::UnsupportedArchitecture(arch_val.to_string())),
        };

        // Manifest length
        let manifest_len = u32::from_le_bytes([data[14], data[15], data[16], data[17]]) as usize;
        if manifest_len > MAX_MANIFEST_SIZE {
            return Err(PackageError::ManifestTooLarge(manifest_len, MAX_MANIFEST_SIZE));
        }

        let manifest_start = 18;
        let manifest_end = manifest_start + manifest_len;
        if data.len() < manifest_end {
            return Err(PackageError::PackageTruncated(manifest_end, data.len()));
        }

        let manifest_bytes = &data[manifest_start..manifest_end];
        let manifest: Manifest = serde_json::from_slice(manifest_bytes)
            .map_err(|e| PackageError::InvalidManifestJson(e))?;

        // File count
        let file_count_offset = manifest_end;
        if data.len() < file_count_offset + 4 {
            return Err(PackageError::PackageTruncated(file_count_offset + 4, data.len()));
        }
        let file_count = u32::from_le_bytes([
            data[file_count_offset],
            data[file_count_offset + 1],
            data[file_count_offset + 2],
            data[file_count_offset + 3],
        ]) as usize;

        if file_count > MAX_FILE_COUNT {
            return Err(PackageError::FileCountExceedsMaximum(file_count, MAX_FILE_COUNT));
        }

        // Parse file entries
        let mut offset = file_count_offset + 4;
        let mut files = Vec::with_capacity(file_count);

        for _ in 0..file_count {
            if offset + 4 > data.len() {
                return Err(PackageError::PackageTruncated(offset + 4, data.len()));
            }
            let path_len = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + path_len > data.len() {
                return Err(PackageError::PackageTruncated(offset + path_len, data.len()));
            }
            let path = String::from_utf8_lossy(&data[offset..offset + path_len]).to_string();
            offset += path_len;

            validate_path(&path)?;

            if offset + 8 + 8 + 4 + 4 > data.len() {
                return Err(PackageError::PackageTruncated(offset + 24, data.len()));
            }
            let size = u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            offset += 8;
            let checksum = u64::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
            ]);
            offset += 8;
            let permissions = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]);
            offset += 4;
            let file_type = data[offset];
            offset += 1;

            files.push(FileEntry {
                path,
                size,
                checksum,
                permissions,
                file_type,
            });
        }

        let content_start = offset;

        // Read file contents
        let mut file_contents = Vec::with_capacity(file_count);
        let mut content_offset = content_start;

        for entry in &files {
            if content_offset + entry.size as usize > data.len() - 65 {
                // Reserve 65 bytes for signature + algorithm
                return Err(PackageError::PackageTruncated(
                    content_offset + entry.size as usize,
                    data.len(),
                ));
            }
            let data_slice = data[content_offset..content_offset + entry.size as usize].to_vec();

            // Verify checksum
            let mut hasher = Sha256::new();
            hasher.update(&data_slice);
            let hash = hasher.finalize();
            let expected_checksum = u64::from_le_bytes([
                hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
            ]);
            if expected_checksum != entry.checksum {
                return Err(PackageError::ChecksumMismatch(entry.path.clone()));
            }

            file_contents.push((entry.path.clone(), data_slice));
            content_offset += entry.size as usize;
        }

        // Read signature and algorithm
        let sig_start = content_offset;
        if data.len() < sig_start + 65 {
            return Err(PackageError::PackageTruncated(sig_start + 65, data.len()));
        }

        let signature_bytes: [u8; 64] = data[sig_start..sig_start + 64]
            .try_into()
            .map_err(|_| PackageError::PackageTruncated(sig_start + 64, data.len()))?;
        let sig_algorithm = data[sig_start + 64];

        let signature = if sig_algorithm == SIG_ED25519 {
            Some(Signature::from_bytes(signature_bytes))
        } else if sig_algorithm == SIG_NONE {
            None
        } else {
            return Err(PackageError::InvalidSignatureAlgorithm(sig_algorithm));
        };

        let public_key = None; // Key is looked up separately during verification

        Ok(Package {
            manifest,
            files,
            file_contents,
            signature,
            signature_algorithm: sig_algorithm,
            public_key,
        })
    }
}

/// Validate a file path within a package.
fn validate_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(PackageError::InvalidFilePath("empty path".to_string()));
    }
    if !path.starts_with('/') {
        return Err(PackageError::InvalidFilePath(format!(
            "path must be absolute: {path}"
        )));
    }
    if path.contains("..") {
        return Err(PackageError::PathTraversal(path.to_string()));
    }
    if path.len() > MAX_PATH_LENGTH {
        return Err(PackageError::InvalidFilePath(format!(
            "path exceeds {MAX_PATH_LENGTH} bytes: {path}"
        )));
    }
    Ok(())
}
