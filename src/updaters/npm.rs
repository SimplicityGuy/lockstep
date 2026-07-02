//! npm: bump package.json with npm-check-updates, then `npm install` to
//! refresh the lockfile. `--major` lets ncu target the latest major.

use std::path::Path;

use crate::context::{Ecosystem, RunCtx};
use crate::report::Status;
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct Npm;

pub fn ncu_args(major: bool) -> Vec<&'static str> {
    if major {
        vec!["npm-check-updates", "-u"]
    } else {
        vec!["npm-check-updates", "-u", "--target", "minor"]
    }
}

impl Updater for Npm {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Npm
    }

    fn detect(&self, root: &Path) -> Detection {
        let pkg = root.join("package.json");
        if pkg.is_file() {
            Detection::Present { targets: vec![pkg] }
        } else {
            Detection::Absent {
                reason: "no package.json".into(),
            }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        if matches!(det, Detection::Absent { .. }) {
            return UpdateOutcome::not_present("no package.json");
        }
        if !ctx.runner.which("npx") {
            return UpdateOutcome::warned("`npx` not on PATH — skipped");
        }
        if ctx.dry_run {
            return UpdateOutcome::new(Status::WouldChange);
        }
        let argv = ncu_args(ctx.major);
        match ctx.runner.run("npx", &argv, &ctx.repo_root) {
            Ok(o) if o.ok() => {}
            Ok(o) => {
                return UpdateOutcome::errored(format!(
                    "npm-check-updates failed: {}",
                    o.stderr.trim()
                ));
            }
            Err(e) => return UpdateOutcome::errored(e.to_string()),
        }
        let mut out = UpdateOutcome::applied(vec![]);
        if ctx.runner.which("npm") {
            match ctx.runner.run("npm", &["install"], &ctx.repo_root) {
                Ok(o) if o.ok() => {}
                Ok(o) => out
                    .warnings
                    .push(format!("npm install failed: {}", o.stderr.trim())),
                Err(e) => out.warnings.push(e.to_string()),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncu_targets_minor_unless_major() {
        assert_eq!(
            ncu_args(false),
            vec!["npm-check-updates", "-u", "--target", "minor"]
        );
        assert_eq!(ncu_args(true), vec!["npm-check-updates", "-u"]);
    }

    struct FakeRunner {
        fail_on: &'static str,
    }
    impl crate::context::Runner for FakeRunner {
        fn which(&self, _p: &str) -> bool {
            true
        }
        fn run(
            &self,
            _p: &str,
            args: &[&str],
            _cwd: &std::path::Path,
        ) -> anyhow::Result<crate::context::CmdOutput> {
            let status = if args.iter().any(|a| *a == self.fail_on) {
                1
            } else {
                0
            };
            Ok(crate::context::CmdOutput {
                status,
                stdout: String::new(),
                stderr: "boom".into(),
            })
        }
    }

    #[test]
    fn npm_install_failure_surfaces_as_warning_not_swallowed() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}\n").unwrap();
        let ctx = RunCtx {
            repo_root: tmp.path().to_path_buf(),
            dry_run: false,
            freeze: std::collections::HashSet::new(),
            major: false,
            only: Vec::new(),
            skip: Vec::new(),
            runner: Box::new(FakeRunner { fail_on: "install" }),
        };
        let det = Detection::Present {
            targets: vec![tmp.path().join("package.json")],
        };
        let out = Npm.run(&ctx, &det);
        // `UpdateOutcome::applied(vec![])` maps to `UpToDate` when the
        // changes list is empty (see `UpdateOutcome::applied`), and this
        // delegation-style updater never populates structured `Change`
        // entries (ncu's own diff isn't parsed). So a successful ncu run
        // followed by a failed `npm install` still reports `UpToDate`,
        // carrying the failure as a warning rather than silently dropping
        // it (the bug this test guards against).
        assert_eq!(out.status, Status::UpToDate);
        assert!(
            !out.warnings.is_empty(),
            "failed npm install should surface as a warning"
        );
        assert!(out.errors.is_empty());
    }
}
