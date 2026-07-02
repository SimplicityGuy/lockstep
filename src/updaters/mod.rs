//! The Updater trait and its shared result types.

use std::path::{Path, PathBuf};

use crate::context::{Ecosystem, RunCtx};
use crate::report::{Change, HeldBackMajor, Status};

pub mod cargo;
pub mod docker;
pub mod github_actions;
pub mod npm;
pub mod pre_commit;
pub mod python_requirements;
pub mod python_uv;

/// Result of scanning a repo for one ecosystem's manifests.
pub enum Detection {
    Present { targets: Vec<PathBuf> },
    Absent { reason: String },
}

/// What an updater did (or would do).
pub struct UpdateOutcome {
    pub status: Status,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub held_back: Vec<HeldBackMajor>,
}

impl UpdateOutcome {
    pub fn new(status: Status) -> Self {
        UpdateOutcome {
            status,
            changes: vec![],
            warnings: vec![],
            errors: vec![],
            held_back: vec![],
        }
    }
    pub fn applied(changes: Vec<Change>) -> Self {
        let status = if changes.is_empty() {
            Status::UpToDate
        } else {
            Status::Applied
        };
        UpdateOutcome {
            changes,
            ..UpdateOutcome::new(status)
        }
    }
    pub fn not_present(reason: impl Into<String>) -> Self {
        let mut o = UpdateOutcome::new(Status::NotPresent);
        o.warnings.push(reason.into());
        o
    }
    pub fn warned(msg: impl Into<String>) -> Self {
        let mut o = UpdateOutcome::new(Status::Warned);
        o.warnings.push(msg.into());
        o
    }
    pub fn errored(msg: impl Into<String>) -> Self {
        let mut o = UpdateOutcome::new(Status::Errored);
        o.errors.push(msg.into());
        o
    }
}

/// One ecosystem updater.
pub trait Updater {
    fn ecosystem(&self) -> Ecosystem;
    fn detect(&self, root: &Path) -> Detection;
    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome;
}

/// All updaters, in run order.
pub fn all() -> Vec<Box<dyn Updater>> {
    vec![
        Box::new(python_uv::PythonUv),
        Box::new(python_requirements::PythonRequirements),
        Box::new(npm::Npm),
        Box::new(cargo::Cargo),
        Box::new(docker::Docker),
        Box::new(github_actions::GithubActions),
        Box::new(pre_commit::PreCommit),
    ]
}
