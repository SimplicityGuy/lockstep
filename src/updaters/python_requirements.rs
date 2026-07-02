//! requirements files. When a `*.in` exists, delegate to pip-tools
//! (`uv pip compile --upgrade`); a plain `requirements.txt` with no `.in` is
//! reported as unmanaged (warn) since there is nothing to recompile from.

use std::path::{Path, PathBuf};

use crate::context::{Ecosystem, RunCtx};
use crate::report::Status;
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct PythonRequirements;

pub fn compile_args(in_file: &str) -> Vec<String> {
    let out_file = in_file
        .strip_suffix(".in")
        .map(|s| format!("{s}.txt"))
        .unwrap_or_else(|| "requirements.txt".into());
    vec![
        "compile".into(),
        "--upgrade".into(),
        "--output-file".into(),
        out_file,
        in_file.into(),
    ]
}

impl Updater for PythonRequirements {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Requirements
    }

    fn detect(&self, root: &Path) -> Detection {
        let mut ins: Vec<PathBuf> = Vec::new();
        let mut txts: Vec<PathBuf> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(root) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("requirements") && name.ends_with(".in") {
                    ins.push(e.path());
                } else if name.starts_with("requirements") && name.ends_with(".txt") {
                    txts.push(e.path());
                }
            }
        }
        ins.sort();
        let targets = if ins.is_empty() { txts } else { ins };
        if targets.is_empty() {
            Detection::Absent {
                reason: "no requirements files".into(),
            }
        } else {
            Detection::Present { targets }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        let Detection::Present { targets } = det else {
            return UpdateOutcome::not_present("no requirements files");
        };
        let ins: Vec<&PathBuf> = targets
            .iter()
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("in"))
            .collect();
        if ins.is_empty() {
            return UpdateOutcome::warned("requirements.txt without a .in — nothing to recompile");
        }
        if !ctx.runner.which("uv") {
            return UpdateOutcome::warned("`uv` (pip compile) not on PATH — skipped");
        }
        if ctx.dry_run {
            return UpdateOutcome::new(Status::WouldChange);
        }
        for in_file in ins {
            let name = in_file.file_name().unwrap().to_string_lossy().to_string();
            let mut args = vec!["pip".to_string()];
            args.extend(compile_args(&name));
            let argv: Vec<&str> = args.iter().map(String::as_str).collect();
            match ctx.runner.run("uv", &argv, &ctx.repo_root) {
                Ok(o) if o.ok() => {}
                Ok(o) => {
                    return UpdateOutcome::errored(format!(
                        "uv pip compile failed: {}",
                        o.stderr.trim()
                    ));
                }
                Err(e) => return UpdateOutcome::errored(e.to_string()),
            }
        }
        UpdateOutcome::applied(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_args_upgrade_and_output() {
        let a = compile_args("requirements.in");
        assert_eq!(
            a,
            vec![
                "compile".to_string(),
                "--upgrade".to_string(),
                "--output-file".to_string(),
                "requirements.txt".to_string(),
                "requirements.in".to_string(),
            ]
        );
    }
}
