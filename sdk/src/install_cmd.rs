//! `elvaris-sdk install` - Install a package into a local repository.

use anyhow::{bail, Result};
use clap::Args;
use packages::{PackageReader, PublicKey};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Package file to install
    pub package: PathBuf,

    /// Repository directory
    #[arg(long)]
    pub repo: Option<PathBuf>,

    /// Public key for verification
    #[arg(long)]
    pub key: Option<PathBuf>,
}

pub fn run(args: InstallArgs) -> Result<()> {
    let dirs = super::ElvarisDirs::default();
    dirs.ensure_dirs()?;

    let repo = args.repo.unwrap_or_else(|| dirs.repos.join("local"));
    std::fs::create_dir_all(&repo)?;

    let package_bytes = std::fs::read(&args.package)?;
    let package = PackageReader::read(&package_bytes)?;

    // Verify signature if key provided
    if let Some(key_path) = &args.key {
        let key_bytes = std::fs::read(key_path)?;
        if key_bytes.len() != 32 {
            bail!("public key must be exactly 32 bytes");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes);
        let public_key = PublicKey::from_bytes(arr)?;

        match &package.signature {
            Some(sig) => {
        let sig_bytes = sig.as_bytes();
        verify_package_signature(&package_bytes[..package_bytes.len() - 65], &sig_bytes, &public_key)?;
                println!("signature: verified");
            }
            None => {
                bail!("package is unsigned; refusing to install without signature verification");
            }
        }
    } else {
        println!("warning: installing unsigned package (development only)");
    }

    // Copy package to repository
    let pkg_name = format!("{}-{}.elvpkg", package.manifest.identifier, package.manifest.version);
    let dest = repo.join(&pkg_name);
    std::fs::copy(&args.package, &dest)?;

    // Update repository index
    update_repo_index(&repo)?;

    println!("installed: {} into {}", pkg_name, repo.display());

    Ok(())
}

fn verify_package_signature(
    package_bytes: &[u8],
    signature: &[u8; 64],
    public_key: &PublicKey,
) -> Result<()> {
    packages::verify_package_signature(package_bytes, signature, public_key)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn update_repo_index(repo: &std::path::Path) -> Result<()> {
    let mut packages = Vec::new();
    for entry in std::fs::read_dir(repo)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "elvpkg") {
            let data = std::fs::read(&path)?;
            if let Ok(pkg) = PackageReader::read(&data) {
                packages.push(serde_json::json!({
                    "identifier": pkg.manifest.identifier,
                    "version": pkg.manifest.version,
                    "architecture": pkg.manifest.architecture.as_str(),
                    "file": path.file_name().unwrap().to_string_lossy(),
                }));
            }
        }
    }

    let index = serde_json::json!({
        "packages": packages,
    });

    let index_path = repo.join("index.json");
    std::fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;

    Ok(())
}
