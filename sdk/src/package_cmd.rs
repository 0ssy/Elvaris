//! `elvaris-sdk package` - Create a .elvpkg package.

use anyhow::{bail, Result};
use clap::Args;
use packages::{
    Architecture, Dependency, Manifest, PackageBuilder, Permission,
};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct PackageArgs {
    /// Directory containing application files
    #[arg(default_value = ".")]
    pub input: PathBuf,

    /// Output package path (.elvpkg)
    #[arg(short = 'o', long)]
    pub output: PathBuf,

    /// Package name
    #[arg(long)]
    pub name: Option<String>,

    /// Package identifier (reverse-domain)
    #[arg(long)]
    pub identifier: Option<String>,

    /// Version (major.minor.patch)
    #[arg(long, default_value = "1.0.0")]
    pub version: String,

    /// Architecture: x86_64 or aarch64
    #[arg(long, default_value = "x86_64")]
    pub arch: String,

    /// Entrypoint path
    #[arg(long)]
    pub entrypoint: Option<String>,

    /// Permission (can be repeated)
    #[arg(long)]
    pub permission: Vec<String>,

    /// Dependency in format id=constraint (can be repeated)
    #[arg(long)]
    pub dependency: Vec<String>,
}

pub fn run(args: PackageArgs) -> Result<()> {
    let input = args.input.canonicalize()?;

    let architecture = match args.arch.as_str() {
        "x86_64" => Architecture::X86_64,
        "aarch64" => Architecture::AArch64,
        other => bail!("unsupported architecture: {other}"),
    };

    let entrypoint = args
        .entrypoint
        .unwrap_or_else(|| "/bin/app".to_string());

    let name = args
        .name
        .unwrap_or_else(|| input.file_name().unwrap_or_default().to_string_lossy().to_string());

    let identifier = args.identifier.unwrap_or_else(|| {
        format!("org.elvaris.{}", name.to_lowercase().replace(' ', "-"))
    });

    let mut permissions = Vec::new();
    for p in &args.permission {
        permissions.push(parse_permission(p)?);
    }

    let mut dependencies = Vec::new();
    for d in &args.dependency {
        dependencies.push(parse_dependency(d)?);
    }

    // Collect files from input directory
    let mut files: Vec<(String, Vec<u8>, u32, u8)> = Vec::new();
    collect_files(&input, &input, &mut files)?;

    if files.is_empty() {
        bail!("no files found in {input:?}");
    }

    // Build manifest
    let manifest = Manifest {
        name,
        identifier,
        version: args.version,
        architecture,
        entrypoint,
        runtime: "none".to_string(),
        files: files.iter().map(|(p, _, _, _)| p.clone()).collect(),
        permissions,
        dependencies,
        manifest_version: 1,
    };

    // Build package
    let mut builder = PackageBuilder::new(manifest);
    for (path, data, permissions, file_type) in files {
        builder = builder.add_file(path, data, permissions, file_type);
    }

    let package_bytes = builder.build()?;

    // Write output
    std::fs::write(&args.output, &package_bytes)?;
    println!("package: {}", args.output.display());

    Ok(())
}

fn parse_permission(s: &str) -> Result<Permission> {
    match s {
        "console.input" => Ok(Permission::ConsoleInput),
        "console.output" => Ok(Permission::ConsoleOutput),
        "filesystem.read" => Ok(Permission::FilesystemRead),
        "filesystem.write" => Ok(Permission::FilesystemWrite),
        "network.client" => Ok(Permission::NetworkClient),
        "network.server" => Ok(Permission::NetworkServer),
        "process.inspect" => Ok(Permission::ProcessInspect),
        "device.access" => Ok(Permission::DeviceAccess),
        "system.shutdown" => Ok(Permission::SystemShutdown),
        other => bail!("unknown permission: {other}"),
    }
}

fn parse_dependency(s: &str) -> Result<Dependency> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        bail!("invalid dependency format (expected id=constraint): {s}");
    }
    Ok(Dependency {
        identifier: parts[0].to_string(),
        constraint: parts[1].to_string(),
    })
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(String, Vec<u8>, u32, u8)>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            collect_files(root, &path, files)?;
        } else if metadata.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .to_string_lossy()
                .replace('\\', "/");
            let package_path = format!("/{rel}");
            let data = std::fs::read(&path)?;
            let is_exec = path.extension().map_or(false, |ext| ext == "bin")
                || path.file_name().map_or(false, |n| n == "app");
            let file_type = if is_exec { 2 } else { 0 }; // 2 = executable, 0 = regular
            let permissions = if is_exec { 0o755 } else { 0o644 };
            files.push((package_path, data, permissions, file_type));
        }
    }
    Ok(())
}
