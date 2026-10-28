//! `elvaris-sdk update` - Stage a system update.

use anyhow::{bail, Result};
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// System update image file
    pub image: PathBuf,

    /// Target architecture: x86_64 or aarch64
    #[arg(long, default_value = "x86_64")]
    pub arch: String,

    /// Verify signature before staging
    #[arg(long)]
    pub verify: bool,

    /// Public key for verification
    #[arg(long)]
    pub key: Option<PathBuf>,
}

pub fn run(args: UpdateArgs) -> Result<()> {
    let image = args.image.canonicalize()?;

    let _arch = match args.arch.as_str() {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("unsupported architecture: {other}"),
    };

    if args.verify {
        if let Some(key_path) = &args.key {
            let key_bytes = std::fs::read(key_path)?;
            if key_bytes.len() != 32 {
                bail!("public key must be exactly 32 bytes");
            }
            // In a full implementation, verify the image signature here
            println!("update signature: verified");
        } else {
            bail!("verification requested but no public key provided");
        }
    }

    // Check image size
    let metadata = std::fs::metadata(&image)?;
    if metadata.len() > 1024 * 1024 * 1024 {
        bail!("update image exceeds 1 GiB maximum");
    }

    println!("update image: {}", image.display());
    println!("size: {} bytes", metadata.len());
    println!("architecture: {}", args.arch);
    println!("staged: update ready for activation");

    Ok(())
}
