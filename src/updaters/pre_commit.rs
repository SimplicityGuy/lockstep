//! pre-commit hooks via `pre-commit autoupdate` (+ `--freeze` when frozen).

use std::path::Path;

use crate::context::{Ecosystem, RunCtx};
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct PreCommit;

pub fn autoupdate_args(frozen: bool) -> Vec<&'static str> {
    if frozen {
        vec!["autoupdate", "--freeze"]
    } else {
        vec!["autoupdate"]
    }
}

impl Updater for PreCommit {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::PreCommit
    }

    fn detect(&self, root: &Path) -> Detection {
        let cfg = root.join(".pre-commit-config.yaml");
        if cfg.is_file() {
            Detection::Present { targets: vec![cfg] }
        } else {
            Detection::Absent {
                reason: "no .pre-commit-config.yaml".into(),
            }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        if matches!(det, Detection::Absent { .. }) {
            return UpdateOutcome::not_present("no .pre-commit-config.yaml");
        }
        if !ctx.runner.which("pre-commit") {
            return UpdateOutcome::warned("`pre-commit` not on PATH — skipped");
        }
        let args = autoupdate_args(ctx.is_frozen(Ecosystem::PreCommit));
        if ctx.dry_run {
            return UpdateOutcome::new(crate::report::Status::WouldChange);
        }
        match ctx.runner.run("pre-commit", &args, &ctx.repo_root) {
            Ok(o) if o.ok() => UpdateOutcome::applied(vec![]),
            Ok(o) => {
                UpdateOutcome::errored(format!("pre-commit autoupdate failed: {}", o.stderr.trim()))
            }
            Err(e) => UpdateOutcome::errored(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_include_freeze_only_when_frozen() {
        assert_eq!(autoupdate_args(false), vec!["autoupdate"]);
        assert_eq!(autoupdate_args(true), vec!["autoupdate", "--freeze"]);
    }
}
