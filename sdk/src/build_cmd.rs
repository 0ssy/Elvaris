//! `elvaris-sdk build` - Build a native Elvaris application.

use anyhow::{bail, Result};
use clap::Args;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct BuildArgs {
    /// Source directory or Cargo.toml path
    #[arg(default_value = ".")]
    pub source: PathBuf,

    /// Target architecture: x86_64 or aarch64
    #[arg(long, default_value = "x86_64")]
    pub arch: String,

    /// Build profile: debug, release, test
    #[arg(long, default_value = "release")]
    pub profile: String,

    /// Output directory
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Application name (for Cargo projects)
    #[arg(long)]
    pub name: Option<String>,
}

pub fn run(args: BuildArgs) -> Result<()> {
    let arch = match args.arch.as_str() {
        "x86_64" => "x86_64-unknown-none",
        "aarch64" => "aarch64-unknown-none",
        other => bail!("unsupported architecture: {other}"),
    };

    let profile = match args.profile.as_str() {
        "debug" | "release" | "test" => args.profile.clone(),
        other => bail!("unsupported profile: {other}"),
    };

    let source = args.source.canonicalize()?;
    let cargo_toml = source.join("Cargo.toml");

    if cargo_toml.exists() {
        build_rust_app(&source, arch, &profile, args.out.as_deref(), args.name.as_deref())?;
    } else {
        // Check for C source files
        let has_c = std::fs::read_dir(&source)?
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map_or(false, |ext| ext == "c"));
        if has_c {
            build_c_app(&source, arch, &profile, args.out.as_deref())?;
        } else {
            bail!("no Cargo.toml or C source files found in {source:?}");
        }
    }

    Ok(())
}

fn build_rust_app(
    source: &Path,
    target: &str,
    profile: &str,
    out: Option<&Path>,
    name: Option<&str>,
) -> Result<()> {
    let target_dir: PathBuf = out.map(|p| p.to_path_buf()).unwrap_or_else(|| source.join("target"));

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(source);
    cmd.arg("build");
    cmd.arg("--target").arg(target);
    cmd.arg(format!("--profile={profile}"));

    if let Some(n) = name {
        cmd.arg("--bin").arg(n);
    }

    let status = cmd.status()?;
    if !status.success() {
        bail!("cargo build failed with exit code {:?}", status.code());
    }

    // Determine the output binary path
    let profile_dir = if profile == "debug" { "debug" } else { "release" };
    let bin_path = target_dir.join(target).join(profile_dir);

    if let Some(n) = name {
        let bin = bin_path.join(n);
        if bin.exists() {
            println!("built: {}", bin.display());
            return Ok(());
        }
    }

    // Find the first binary in the output directory
    if bin_path.is_dir() {
        for entry in std::fs::read_dir(&bin_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext.is_empty()) {
                println!("built: {}", path.display());
                return Ok(());
            }
        }
    }

    bail!("no binary found in {bin_path:?}");
}

fn build_c_app(
    _source: &Path,
    _target: &str,
    _profile: &str,
    _out: Option<&Path>,
) -> Result<()> {
    bail!("C application building requires a cross-compiler; not yet configured");
}
