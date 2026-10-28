//! `elvaris-sdk repo` - Manage a local package repository.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct RepoArgs {
    /// Repository directory
    #[arg(long)]
    pub repo: Option<PathBuf>,

    /// List packages in repository
    #[arg(long)]
    pub list: bool,
}

pub fn run(args: RepoArgs) -> Result<()> {
    let dirs = super::ElvarisDirs::default();
    let repo = args.repo.unwrap_or_else(|| dirs.repos.join("local"));

    if !repo.exists() {
        println!("repository does not exist: {}", repo.display());
        return Ok(());
    }

    if args.list {
        list_packages(&repo)?;
    } else {
        println!("repository: {}", repo.display());
        list_packages(&repo)?;
    }

    Ok(())
}

fn list_packages(repo: &std::path::Path) -> Result<()> {
    let index_path = repo.join("index.json");
    if index_path.exists() {
        let data = std::fs::read_to_string(&index_path)?;
        let index: serde_json::Value = serde_json::from_str(&data)?;
        if let Some(packages) = index.get("packages").and_then(|v| v.as_array()) {
            for pkg in packages {
                let id = pkg.get("identifier").and_then(|v| v.as_str()).unwrap_or("?");
                let ver = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                let arch = pkg.get("architecture").and_then(|v| v.as_str()).unwrap_or("?");
                let file = pkg.get("file").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {id} {ver} [{arch}] - {file}");
            }
        }
    } else {
        let mut count = 0;
        for entry in std::fs::read_dir(repo)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "elvpkg") {
                count += 1;
                println!("  {}", path.file_name().unwrap().to_string_lossy());
            }
        }
        if count == 0 {
            println!("  (empty)");
        }
    }
    Ok(())
}
