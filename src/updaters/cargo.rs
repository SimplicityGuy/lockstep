//! Cargo: `cargo upgrade` (from cargo-edit) bumps Cargo.toml; `cargo update`
//! refreshes Cargo.lock within the new bounds. `--major` allows incompatible bumps.

use std::path::Path;

use crate::context::{Ecosystem, RunCtx};
use crate::report::Status;
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct Cargo;

pub fn upgrade_args(major: bool) -> Vec<&'static str> {
    if major {
        vec!["upgrade", "--incompatible"]
    } else {
        vec!["upgrade"]
    }
}

impl Updater for Cargo {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Cargo
    }

    fn detect(&self, root: &Path) -> Detection {
        let manifest = root.join("Cargo.toml");
        if manifest.is_file() {
            Detection::Present {
                targets: vec![manifest],
            }
        } else {
            Detection::Absent {
                reason: "no Cargo.toml".into(),
            }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        if matches!(det, Detection::Absent { .. }) {
            return UpdateOutcome::not_present("no Cargo.toml");
        }
        if !ctx.runner.which("cargo") {
            return UpdateOutcome::warned("`cargo` not on PATH — skipped");
        }
        if ctx.dry_run {
            return UpdateOutcome::new(Status::WouldChange);
        }
        // `cargo upgrade` requires cargo-edit; if it's absent cargo returns a
        // "no such subcommand" error — surface that as a warn, not a hard error.
        match ctx
            .runner
            .run("cargo", &upgrade_args(ctx.major), &ctx.repo_root)
        {
            Ok(o) if o.ok() => {}
            Ok(o) if o.stderr.contains("no such") || o.stderr.contains("not installed") => {
                return UpdateOutcome::warned(
                    "`cargo upgrade` unavailable (install cargo-edit) — skipped",
                );
            }
            Ok(o) => {
                return UpdateOutcome::errored(format!(
                    "cargo upgrade failed: {}",
                    o.stderr.trim()
                ));
            }
            Err(e) => return UpdateOutcome::errored(e.to_string()),
        }
        let mut out = UpdateOutcome::applied(vec![]);
        match ctx.runner.run("cargo", &["update"], &ctx.repo_root) {
            Ok(o) if o.ok() => {}
            Ok(o) => out
                .warnings
                .push(format!("cargo update failed: {}", o.stderr.trim())),
            Err(e) => out.warnings.push(e.to_string()),
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_uses_incompatible_only_for_major() {
        assert_eq!(upgrade_args(false), vec!["upgrade"]);
        assert_eq!(upgrade_args(true), vec!["upgrade", "--incompatible"]);
    }

    struct FakeRunner;
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
            let status = if args.iter().any(|a| *a == "update") {
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
    fn cargo_update_failure_surfaces_as_warning_not_swallowed() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]\n").unwrap();
        let ctx = RunCtx {
            repo_root: tmp.path().to_path_buf(),
            dry_run: false,
            freeze: std::collections::HashSet::new(),
            major: false,
            only: Vec::new(),
            skip: Vec::new(),
            runner: Box::new(FakeRunner),
        };
        let det = Detection::Present {
            targets: vec![tmp.path().join("Cargo.toml")],
        };
        let out = Cargo.run(&ctx, &det);
        // `cargo upgrade` succeeds (status 0), `cargo update` fails (status
        // 1, since args contain "update"). The failure must surface as a
        // warning, not be silently dropped via `let _ =`.
        assert_eq!(out.status, Status::UpToDate);
        assert!(
            !out.warnings.is_empty(),
            "failed cargo update should surface as a warning"
        );
        assert!(out.errors.is_empty());
    }
}
