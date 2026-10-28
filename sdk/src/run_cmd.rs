//! `elvaris-sdk run` - Run a packaged application in QEMU.

use anyhow::{bail, Result};
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Package file or binary to run
    pub target: PathBuf,

    /// Architecture: x86_64 or aarch64
    #[arg(long, default_value = "x86_64")]
    pub arch: String,

    /// QEMU machine type
    #[arg(long, default_value = "q35")]
    pub machine: String,
}

pub fn run(args: RunArgs) -> Result<()> {
    let qemu = match args.arch.as_str() {
        "x86_64" => "qemu-system-x86_64",
        "aarch64" => "qemu-system-aarch64",
        other => bail!("unsupported architecture: {other}"),
    };

    // Check if QEMU is available
    if std::process::Command::new(qemu)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        bail!("{qemu} not found; install QEMU to run applications");
    }

    let target = args.target.canonicalize()?;

    if target.extension().map_or(false, |ext| ext == "elvpkg") {
        run_package(qemu, &target, &args.machine)?;
    } else {
        run_binary(qemu, &target, &args.machine)?;
    }

    Ok(())
}

fn run_package(_qemu: &str, _package: &PathBuf, _machine: &str) -> Result<()> {
    bail!("running packages directly in QEMU requires kernel package support; not yet implemented");
}

fn run_binary(_qemu: &str, _binary: &PathBuf, _machine: &str) -> Result<()> {
    bail!("running bare binaries in QEMU requires a bootable image; use elvaris-sdk update to create one");
}
