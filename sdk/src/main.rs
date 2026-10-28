//! Elvaris SDK command-line tool.
//!
//! Subcommands:
//!   build     - Build a native Elvaris application
//!   package   - Create a .elvpkg package
//!   sign      - Sign a package
//!   verify    - Verify a package signature
//!   install   - Install a package into a local repository
//!   run       - Run a packaged application in QEMU
//!   repo      - Manage a local package repository
//!   update    - Stage a system update

use clap::Parser;
use std::path::PathBuf;

mod build_cmd;
mod install_cmd;
mod package_cmd;
mod repo_cmd;
mod run_cmd;
mod sign_cmd;
mod update_cmd;
mod verify_cmd;

use build_cmd::BuildArgs;
use install_cmd::InstallArgs;
use package_cmd::PackageArgs;
use repo_cmd::RepoArgs;
use run_cmd::RunArgs;
use sign_cmd::SignArgs;
use update_cmd::UpdateArgs;
use verify_cmd::VerifyArgs;

#[derive(Parser, Debug)]
#[command(name = "elvaris-sdk", about = "Elvaris SDK tooling")]
enum Commands {
    Build(BuildArgs),
    Package(PackageArgs),
    Sign(SignArgs),
    Verify(VerifyArgs),
    Install(InstallArgs),
    Run(RunArgs),
    Repo(RepoArgs),
    Update(UpdateArgs),
}

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build(args) => build_cmd::run(args),
        Commands::Package(args) => package_cmd::run(args),
        Commands::Sign(args) => sign_cmd::run(args),
        Commands::Verify(args) => verify_cmd::run(args),
        Commands::Install(args) => install_cmd::run(args),
        Commands::Run(args) => run_cmd::run(args),
        Commands::Repo(args) => repo_cmd::run(args),
        Commands::Update(args) => update_cmd::run(args),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Standard Elvaris directories.
#[derive(Debug, Clone)]
pub struct ElvarisDirs {
    pub packages: PathBuf,
    pub keys: PathBuf,
    pub repos: PathBuf,
}

impl ElvarisDirs {
    pub fn default() -> Self {
        let base = std::env::var("ELVARIS_HOME")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap().join(".elvaris"));
        Self {
            packages: base.join("packages"),
            keys: base.join("keys"),
            repos: base.join("repos"),
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.packages)?;
        std::fs::create_dir_all(&self.keys)?;
        std::fs::create_dir_all(&self.repos)?;
        Ok(())
    }
}
