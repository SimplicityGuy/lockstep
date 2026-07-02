//! Command-line surface (clap derive) and its mapping into a RunCtx.

use std::collections::HashSet;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::context::{Ecosystem, RunCtx, SystemRunner};

#[derive(Parser)]
#[command(
    name = "lockstep",
    version,
    about = "Keep every pinned dependency in a repo in step with upstream."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Detect ecosystems and apply updates (default).
    Run(RunArgs),
    /// Report available updates — never writes.
    Check(RunArgs),
    /// Show detected ecosystems and toolchain availability.
    List(PathArg),
    /// Emit shell completions.
    Completions { shell: clap_complete::Shell },
}

#[derive(clap::Args)]
pub struct PathArg {
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

#[derive(clap::Args, Default)]
pub struct RunArgs {
    #[arg(short, long)]
    pub path: Option<PathBuf>,
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    /// Opt specific tools into freezing (SHA/digest pins).
    #[arg(long, value_enum, num_args = 1.., value_delimiter = ' ')]
    pub freeze: Vec<FreezeKey>,
    #[arg(long, value_enum, num_args = 1.., value_delimiter = ' ')]
    pub only: Vec<EcoArg>,
    #[arg(long, value_enum, num_args = 1.., value_delimiter = ' ')]
    pub skip: Vec<EcoArg>,
    #[arg(long)]
    pub major: bool,
    #[arg(long, value_name = "FILE")]
    pub log_json: Option<PathBuf>,
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(short, long)]
    pub quiet: bool,
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum FreezeKey {
    Actions,
    #[value(name = "pre-commit")]
    PreCommit,
    Docker,
    All,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum EcoArg {
    Uv,
    Requirements,
    Npm,
    Cargo,
    Docker,
    Actions,
    #[value(name = "pre-commit")]
    PreCommit,
}

impl EcoArg {
    pub fn to_ecosystem(self) -> Ecosystem {
        match self {
            EcoArg::Uv => Ecosystem::Uv,
            EcoArg::Requirements => Ecosystem::Requirements,
            EcoArg::Npm => Ecosystem::Npm,
            EcoArg::Cargo => Ecosystem::Cargo,
            EcoArg::Docker => Ecosystem::Docker,
            EcoArg::Actions => Ecosystem::Actions,
            EcoArg::PreCommit => Ecosystem::PreCommit,
        }
    }
}

impl RunArgs {
    pub fn freeze_set(&self) -> HashSet<Ecosystem> {
        let mut set = HashSet::new();
        for f in &self.freeze {
            match f {
                FreezeKey::Actions => {
                    set.insert(Ecosystem::Actions);
                }
                FreezeKey::PreCommit => {
                    set.insert(Ecosystem::PreCommit);
                }
                FreezeKey::Docker => {
                    set.insert(Ecosystem::Docker);
                }
                FreezeKey::All => {
                    set.insert(Ecosystem::Actions);
                    set.insert(Ecosystem::PreCommit);
                    set.insert(Ecosystem::Docker);
                }
            }
        }
        set
    }

    pub fn into_ctx(self, force_dry_run: bool) -> RunCtx {
        let repo_root = resolve_root(self.path.as_deref());
        RunCtx {
            repo_root,
            dry_run: self.dry_run || force_dry_run,
            freeze: self.freeze_set(),
            major: self.major,
            only: self.only.iter().map(|e| e.to_ecosystem()).collect(),
            skip: self.skip.iter().map(|e| e.to_ecosystem()).collect(),
            runner: Box::new(SystemRunner),
        }
    }
}

/// Repo root: `--path`, else the git top-level of cwd, else cwd.
pub fn resolve_root(path: Option<&std::path::Path>) -> PathBuf {
    if let Some(p) = path {
        return p.to_path_buf();
    }
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();
    if let Ok(o) = out {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
