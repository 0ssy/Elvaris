//! `elvaris-sdk sign` - Sign a package.

use anyhow::{bail, Result};
use clap::Args;
use packages::{sign_package, KeyPair};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct SignArgs {
    /// Package file to sign
    pub package: PathBuf,

    /// Output signed package path
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Secret key file (32 bytes raw)
    #[arg(long)]
    pub key: PathBuf,

    /// Generate a new key pair and save public key
    #[arg(long)]
    pub generate_key: bool,
}

pub fn run(args: SignArgs) -> Result<()> {
    let dirs = super::ElvarisDirs::default();
    dirs.ensure_dirs()?;

    let key_pair = if args.generate_key {
        let kp = KeyPair::generate();
        let pub_key_path = dirs.keys.join("development.pub");
        std::fs::write(&pub_key_path, kp.public_key().as_bytes())?;
        println!("generated key: public key saved to {}", pub_key_path.display());
        kp
    } else {
        let key_bytes = std::fs::read(&args.key)?;
        if key_bytes.len() != 32 {
            bail!("secret key must be exactly 32 bytes");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&key_bytes);
        KeyPair::from_bytes(arr)
    };

    let package_bytes = std::fs::read(&args.package)?;
    let (signed, _) = sign_package(&key_pair, &package_bytes);

    let output = args.output.unwrap_or_else(|| {
        args.package.with_extension("signed")
    });
    std::fs::write(&output, &signed)?;
    println!("signed: {}", output.display());

    Ok(())
}
