//! `elvaris-sdk verify` - Verify a package signature.

use anyhow::{bail, Result};
use clap::Args;
use packages::{verify_package_signature, PackageReader, PublicKey};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct VerifyArgs {
    /// Package file to verify
    pub package: PathBuf,

    /// Public key file (32 bytes raw)
    #[arg(long)]
    pub key: PathBuf,
}

pub fn run(args: VerifyArgs) -> Result<()> {
    let package_bytes = std::fs::read(&args.package)?;
    let key_bytes = std::fs::read(&args.key)?;

    if key_bytes.len() != 32 {
        bail!("public key must be exactly 32 bytes");
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&key_bytes);
    let public_key = PublicKey::from_bytes(arr)?;

    let package = PackageReader::read(&package_bytes)?;

    match &package.signature {
        Some(sig) => {
            let sig_bytes = sig.as_bytes();
            verify_package_signature(&package_bytes[..package_bytes.len() - 65], &sig_bytes, &public_key)?;
            println!("signature: valid");
            println!("package: {}", package.manifest.identifier);
            println!("version: {}", package.manifest.version);
            println!("architecture: {:?}", package.manifest.architecture);
        }
        None => {
            bail!("package is unsigned");
        }
    }

    Ok(())
}
