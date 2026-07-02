//! uv-managed Python packages: `uv lock --upgrade`, then raise each `>=` lower
//! bound in pyproject.toml to the locked version, then `uv sync`. A repo may
//! hold several uv packages; each dir with pyproject.toml + uv.lock is updated.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::context::{Ecosystem, RunCtx};
use crate::report::{Change, ChangeKind, Status};
use crate::updaters::{Detection, UpdateOutcome, Updater};
use crate::version::{normalize, version_key};

pub struct PythonUv;

/// A `name[extras]>=min<rest>` requirement inside a dependency array.
fn req_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?P<name>[A-Za-z0-9][A-Za-z0-9._-]*)(?P<extras>\[[^\]]*\])?\s*>=\s*(?P<min>[0-9][0-9A-Za-z.+!-]*)(?P<rest>[^"'\n]*)"#,
        )
        .unwrap()
    })
}

pub fn discover(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if dir.join("pyproject.toml").is_file() && dir.join("uv.lock").is_file() {
            out.push(dir.to_path_buf());
        }
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if p.is_dir() && !matches!(name, ".git" | "target" | "node_modules" | ".venv") {
                    walk(&p, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

pub fn locked_versions(uv_lock_text: &str) -> HashMap<String, String> {
    let doc: toml::Value =
        toml::from_str(uv_lock_text).unwrap_or(toml::Value::Table(Default::default()));
    let mut map = HashMap::new();
    if let Some(pkgs) = doc.get("package").and_then(|v| v.as_array()) {
        for p in pkgs {
            if let (Some(n), Some(v)) = (
                p.get("name").and_then(|v| v.as_str()),
                p.get("version").and_then(|v| v.as_str()),
            ) {
                map.insert(normalize(n), v.to_string());
            }
        }
    }
    map
}

pub fn bump_minimums(
    pyproject_text: &str,
    locked: &HashMap<String, String>,
) -> (String, Vec<Change>) {
    let re = req_re();
    let mut changes = Vec::new();
    let out = re.replace_all(pyproject_text, |c: &regex::Captures| {
        let name = &c["name"];
        let cur = &c["min"];
        let Some(lv) = locked.get(&normalize(name)) else {
            return c[0].to_string();
        };
        if version_key(lv) <= version_key(cur) {
            return c[0].to_string();
        }
        changes.push(Change {
            name: name.to_string(),
            from: cur.to_string(),
            to: lv.clone(),
            kind: ChangeKind::Bump,
        });
        let extras = c.name("extras").map(|m| m.as_str()).unwrap_or("");
        format!("{name}{extras}>={lv}{}", &c["rest"])
    });
    (out.into_owned(), changes)
}

impl Updater for PythonUv {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Uv
    }

    fn detect(&self, root: &Path) -> Detection {
        let targets = discover(root);
        if targets.is_empty() {
            Detection::Absent {
                reason: "no pyproject.toml + uv.lock".into(),
            }
        } else {
            Detection::Present { targets }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        let Detection::Present { targets } = det else {
            return UpdateOutcome::not_present("no pyproject.toml + uv.lock");
        };
        if !ctx.runner.which("uv") {
            return UpdateOutcome::warned("`uv` not on PATH — skipped");
        }
        let mut all_changes = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for pkg in targets {
            let pkg_str = pkg.to_string_lossy().to_string();
            if ctx.dry_run {
                let lock_text = std::fs::read_to_string(pkg.join("uv.lock")).unwrap_or_default();
                let py_text =
                    std::fs::read_to_string(pkg.join("pyproject.toml")).unwrap_or_default();
                let (_new, changes) = bump_minimums(&py_text, &locked_versions(&lock_text));
                all_changes.extend(changes);
                continue;
            }
            if let Err(e) = run_uv(ctx, &["lock", "--project", &pkg_str, "--upgrade"]) {
                errors.push(e);
                continue;
            }
            let lock_text = std::fs::read_to_string(pkg.join("uv.lock")).unwrap_or_default();
            let py_path = pkg.join("pyproject.toml");
            let py_text = std::fs::read_to_string(&py_path).unwrap_or_default();
            let (new_py, changes) = bump_minimums(&py_text, &locked_versions(&lock_text));
            if !changes.is_empty() {
                if let Err(e) = std::fs::write(&py_path, new_py) {
                    errors.push(format!("write {}: {e}", py_path.display()));
                    continue; // do NOT report these changes as applied
                }
                if let Err(e) = run_uv(ctx, &["lock", "--project", &pkg_str]) {
                    warnings.push(e);
                }
            }
            if let Err(e) = run_uv(ctx, &["sync", "--project", &pkg_str, "--all-groups"]) {
                warnings.push(e);
            }
            all_changes.extend(changes);
        }
        let status = if !errors.is_empty() {
            Status::Errored
        } else if all_changes.is_empty() {
            Status::UpToDate
        } else if ctx.dry_run {
            Status::WouldChange
        } else {
            Status::Applied
        };
        UpdateOutcome {
            status,
            changes: all_changes,
            warnings,
            errors,
            held_back: Vec::new(),
        }
    }
}

fn run_uv(ctx: &RunCtx, args: &[&str]) -> Result<(), String> {
    match ctx.runner.run("uv", args, &ctx.repo_root) {
        Ok(o) if o.ok() => Ok(()),
        Ok(o) => Err(format!("uv {}: {}", args.join(" "), o.stderr.trim())),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_locked_versions_normalized() {
        let lock = r#"
version = 1
[[package]]
name = "boto3"
version = "1.43.36"
[[package]]
name = "Types_Requests"
version = "2.33.0.20260518"
"#;
        let m = locked_versions(lock);
        assert_eq!(m.get("boto3").map(String::as_str), Some("1.43.36"));
        assert_eq!(
            m.get("types-requests").map(String::as_str),
            Some("2.33.0.20260518")
        );
    }

    #[test]
    fn bumps_minimum_preserving_cap_and_extras() {
        let src = r#"
dependencies = [
    "boto3>=1.34.0,<2.0.0",
    "ruff>=0.6.0",
    "boto3-stubs[lambda,logs]>=1.34.0",
]
"#;
        let locked = [
            ("boto3", "1.43.36"),
            ("ruff", "0.15.19"),
            ("boto3-stubs", "1.43.36"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let (out, changes) = bump_minimums(src, &locked);
        assert!(out.contains("boto3>=1.43.36,<2.0.0"));
        assert!(out.contains("ruff>=0.15.19"));
        assert!(out.contains("boto3-stubs[lambda,logs]>=1.43.36"));
        assert_eq!(changes.len(), 3);
    }

    #[test]
    fn never_downgrades() {
        let src = "dependencies = [\"boto3>=1.50.0,<2.0.0\"]\n";
        let locked = [("boto3".to_string(), "1.43.36".to_string())]
            .into_iter()
            .collect();
        let (out, changes) = bump_minimums(src, &locked);
        assert!(out.contains("boto3>=1.50.0,<2.0.0"));
        assert!(changes.is_empty());
    }

    #[test]
    fn rest_capture_stops_at_single_quote_and_newline() {
        // A single-quoted TOML literal dep on its own line, followed by a
        // normal double-quoted dep. The `rest` capture must not overrun past
        // the closing single quote / newline and swallow the following line.
        let src = "dependencies = [\n    'boto3>=1.34.0,<2.0.0',\n    \"ruff>=0.6.0\",\n]\n";
        let locked = [
            ("boto3".to_string(), "1.43.36".to_string()),
            ("ruff".to_string(), "0.15.19".to_string()),
        ]
        .into_iter()
        .collect();
        let (out, changes) = bump_minimums(src, &locked);
        // Intended bump applied to the single-quoted dep; its cap preserved.
        assert!(out.contains("boto3>=1.43.36,<2.0.0"));
        // The following dep line is intact, not merged into the previous match.
        assert!(out.contains("ruff>=0.15.19"));
        assert!(out.contains("'boto3>=1.43.36,<2.0.0',"));
        // Line count preserved: no line-swallowing / merging occurred.
        assert_eq!(out.lines().count(), src.lines().count());
        assert_eq!(changes.len(), 2);
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

    fn ctx_for(tmp: &std::path::Path, fail_on: &'static str) -> RunCtx {
        RunCtx {
            repo_root: tmp.to_path_buf(),
            dry_run: false,
            freeze: std::collections::HashSet::new(),
            major: false,
            only: Vec::new(),
            skip: Vec::new(),
            runner: Box::new(FakeRunner { fail_on }),
        }
    }

    fn write_pkg(dir: &std::path::Path) {
        std::fs::write(
            dir.join("pyproject.toml"),
            "dependencies = [\"boto3>=1.0.0\"]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("uv.lock"),
            "version = 1\n[[package]]\nname = \"boto3\"\nversion = \"1.5.0\"\n",
        )
        .unwrap();
    }

    #[test]
    fn sync_failure_surfaces_as_warning_not_swallowed() {
        let tmp = tempfile::tempdir().unwrap();
        write_pkg(tmp.path());
        let ctx = ctx_for(tmp.path(), "sync");
        let det = Detection::Present {
            targets: vec![tmp.path().to_path_buf()],
        };
        let out = PythonUv.run(&ctx, &det);
        assert_eq!(out.status, Status::Applied);
        assert!(!out.changes.is_empty());
        let c = &out.changes[0];
        assert_eq!((c.from.as_str(), c.to.as_str()), ("1.0.0", "1.5.0"));
        assert!(!out.warnings.is_empty(), "failed uv sync should warn");
        assert!(out.errors.is_empty());
    }

    #[test]
    fn lock_upgrade_failure_surfaces_as_error() {
        let tmp = tempfile::tempdir().unwrap();
        write_pkg(tmp.path());
        let ctx = ctx_for(tmp.path(), "--upgrade");
        let det = Detection::Present {
            targets: vec![tmp.path().to_path_buf()],
        };
        let out = PythonUv.run(&ctx, &det);
        assert_eq!(out.status, Status::Errored);
        assert!(!out.errors.is_empty());
        assert!(out.changes.is_empty());
    }
}
