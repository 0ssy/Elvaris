//! ed25519 signing and verification for packages.

use crate::error::{PackageError, Result};
use crate::package::SIG_ED25519;
use ed25519_dalek::Signer;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;

/// A key pair for signing packages.
#[derive(Clone)]
pub struct KeyPair {
    inner: SigningKey,
}

impl KeyPair {
    /// Generate a new random key pair.
    pub fn generate() -> Self {
        let mut key_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key_bytes);
        let inner = SigningKey::from_bytes(&key_bytes);
        Self { inner }
    }

    /// Create a key pair from raw signing key bytes (32 bytes).
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let inner = SigningKey::from_bytes(&bytes);
        Self { inner }
    }

    /// Get the public key.
    pub fn public_key(&self) -> PublicKey {
        PublicKey {
            inner: self.inner.verifying_key(),
        }
    }

    /// Sign a message and return the signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.inner.sign(message);
        Signature::from_bytes(sig.to_bytes())
    }
}

/// A public key for verifying package signatures.
#[derive(Clone, Debug)]
pub struct PublicKey {
    inner: VerifyingKey,
}

impl PublicKey {
    /// Create a public key from raw bytes (32 bytes).
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        let inner = VerifyingKey::from_bytes(&bytes)
            .map_err(|_| PackageError::UnknownSigningKey("invalid public key".to_string()))?;
        Ok(Self { inner })
    }

    /// Get the raw public key bytes.
    pub fn as_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }

    /// Compute the key identifier (first 8 bytes of SHA-256 of the public key, hex-encoded).
    pub fn key_id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.as_bytes());
        let hash = hasher.finalize();
        hex::encode(&hash[..8])
    }
}

/// A package signature.
#[derive(Clone, Debug)]
pub struct Signature {
    inner: [u8; 64],
}

impl Signature {
    /// Create from raw 64-byte signature.
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self { inner: bytes }
    }

    /// Get raw signature bytes.
    pub fn as_bytes(&self) -> [u8; 64] {
        self.inner
    }
}

/// Sign a package buffer with a key pair.
pub fn sign_package(key_pair: &KeyPair, package_bytes: &[u8]) -> (Vec<u8>, u8) {
    let signature = key_pair.sign(package_bytes);
    let mut result = package_bytes.to_vec();
    result.extend_from_slice(&signature.as_bytes());
    result.push(SIG_ED25519);
    (result, SIG_ED25519)
}

/// Verify a package signature against a public key.
pub fn verify_package_signature(
    package_bytes: &[u8],
    signature: &[u8],
    public_key: &PublicKey,
) -> Result<()> {
    if signature.len() != 64 {
        return Err(PackageError::SignatureVerificationFailed);
    }
    let sig_arr: [u8; 64] = signature.try_into().map_err(|_| {
        PackageError::SignatureVerificationFailed
    })?;
    let ed_sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
    public_key
        .inner
        .verify_strict(package_bytes, &ed_sig)
        .map_err(|_| PackageError::SignatureVerificationFailed)
}
