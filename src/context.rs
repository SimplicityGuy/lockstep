//! Run context, the ecosystem enum, and the injectable process Runner.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Every ecosystem lockstep can update, in run order.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum Ecosystem {
    Uv,
    Requirements,
    Npm,
    Cargo,
    Docker,
    Actions,
    PreCommit,
}

impl Ecosystem {
    pub const ALL: [Ecosystem; 7] = [
        Ecosystem::Uv,
        Ecosystem::Requirements,
        Ecosystem::Npm,
        Ecosystem::Cargo,
        Ecosystem::Docker,
        Ecosystem::Actions,
        Ecosystem::PreCommit,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Ecosystem::Uv => "uv",
            Ecosystem::Requirements => "requirements",
            Ecosystem::Npm => "npm",
            Ecosystem::Cargo => "cargo",
            Ecosystem::Docker => "docker",
            Ecosystem::Actions => "actions",
            Ecosystem::PreCommit => "pre-commit",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Ecosystem::Uv | Ecosystem::Requirements => "🐍",
            Ecosystem::Npm => "📦",
            Ecosystem::Cargo => "🦀",
            Ecosystem::Docker => "🐳",
            Ecosystem::Actions => "🎬",
            Ecosystem::PreCommit => "🪝",
        }
    }

    pub fn is_freezable(self) -> bool {
        matches!(
            self,
            Ecosystem::Actions | Ecosystem::PreCommit | Ecosystem::Docker
        )
    }

    // reserved: round-trip counterpart to `key()`, for future config/JSON-driven
    // ecosystem selection; the CLI currently maps flags via `cli::EcoArg` instead.
    #[allow(dead_code)]
    pub fn from_key(s: &str) -> Option<Ecosystem> {
        Ecosystem::ALL.into_iter().find(|e| e.key() == s)
    }
}

/// Captured result of a delegated command.
pub struct CmdOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CmdOutput {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

/// Abstraction over process execution so updaters are testable without running tools.
pub trait Runner {
    fn which(&self, program: &str) -> bool;
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> anyhow::Result<CmdOutput>;
}

/// The real runner: `which` for detection, `std::process::Command` for execution.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn which(&self, program: &str) -> bool {
        which::which(program).is_ok()
    }

    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> anyhow::Result<CmdOutput> {
        let out = Command::new(program).args(args).current_dir(cwd).output()?;
        Ok(CmdOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

/// Everything an updater needs to decide what to do and how to report it.
pub struct RunCtx {
    pub repo_root: PathBuf,
    pub dry_run: bool,
    pub freeze: HashSet<Ecosystem>,
    pub major: bool,
    pub only: Vec<Ecosystem>,
    pub skip: Vec<Ecosystem>,
    pub runner: Box<dyn Runner>,
}

impl RunCtx {
    pub fn is_frozen(&self, e: Ecosystem) -> bool {
        self.freeze.contains(&e)
    }

    /// `--only` (if non-empty) is an allowlist; `--skip` always removes.
    pub fn selected(&self, e: Ecosystem) -> bool {
        if self.skip.contains(&e) {
            return false;
        }
        self.only.is_empty() || self.only.contains(&e)
    }

    #[cfg(test)]
    pub fn for_test(only: Vec<Ecosystem>, skip: Vec<Ecosystem>) -> RunCtx {
        RunCtx {
            repo_root: PathBuf::from("."),
            dry_run: true,
            freeze: HashSet::new(),
            major: false,
            only,
            skip,
            runner: Box::new(SystemRunner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_keys_roundtrip() {
        for e in Ecosystem::ALL {
            assert_eq!(Ecosystem::from_key(e.key()), Some(e));
        }
        assert_eq!(Ecosystem::from_key("nope"), None);
    }

    #[test]
    fn only_freezable_are_freezable() {
        assert!(Ecosystem::Actions.is_freezable());
        assert!(Ecosystem::PreCommit.is_freezable());
        assert!(Ecosystem::Docker.is_freezable());
        assert!(!Ecosystem::Uv.is_freezable());
        assert!(!Ecosystem::Cargo.is_freezable());
    }

    #[test]
    fn selection_respects_only_then_skip() {
        let ctx = RunCtx::for_test(
            vec![Ecosystem::Uv, Ecosystem::Cargo],
            vec![Ecosystem::Cargo],
        );
        assert!(ctx.selected(Ecosystem::Uv));
        assert!(!ctx.selected(Ecosystem::Cargo)); // skipped even though in only
        assert!(!ctx.selected(Ecosystem::Npm)); // not in only
    }

    #[test]
    fn empty_only_selects_all_but_skip() {
        let ctx = RunCtx::for_test(vec![], vec![Ecosystem::Docker]);
        assert!(ctx.selected(Ecosystem::Uv));
        assert!(!ctx.selected(Ecosystem::Docker));
    }
}
