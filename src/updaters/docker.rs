//! Dockerfile `FROM` image tags. Hand-rolled: query the registry for the newest
//! tag matching the current tag's granularity, rewrite; under freeze append the
//! `@sha256:…` digest. Only Docker Hub public images are resolved in v1.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::context::{Ecosystem, RunCtx};
use crate::registry::Registry;
use crate::report::{Change, ChangeKind, Status};
use crate::updaters::{Detection, UpdateOutcome, Updater};

pub struct Docker;

fn from_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // FROM <image>:<tag>[@sha256:...] [AS stage]
        Regex::new(
            r"(?m)^(?P<prefix>FROM\s+)(?P<image>[A-Za-z0-9][A-Za-z0-9._/-]*):(?P<tag>[A-Za-z0-9._-]+)(?P<digest>@sha256:[0-9a-f]+)?(?P<suffix>\s+[Aa][Ss]\s+\S+)?[ \t]*$",
        )
        .unwrap()
    })
}

/// Rewrite every resolvable `FROM` line. `resolve(image, tag)` returns the new
/// tag and (when needed) a digest. Returns new text + changes.
pub fn rewrite_from(
    text: &str,
    frozen: bool,
    resolve: impl Fn(&str, &str) -> Option<(String, Option<String>)>,
) -> (String, Vec<Change>) {
    let re = from_re();
    let mut changes = Vec::new();
    let new = re.replace_all(text, |c: &regex::Captures| {
        let image = &c["image"];
        let tag = &c["tag"];
        let suffix = c.name("suffix").map(|m| m.as_str()).unwrap_or("");
        let Some((new_tag, digest)) = resolve(image, tag) else {
            return c[0].to_string();
        };
        let tag_changed = new_tag != tag;
        let digest_part = if frozen {
            digest.map(|d| format!("@{d}")).unwrap_or_default()
        } else {
            String::new()
        };
        // No-op if nothing would change.
        let old_digest = c.name("digest").map(|m| m.as_str()).unwrap_or("");
        if !tag_changed && digest_part == old_digest {
            return c[0].to_string();
        }
        changes.push(Change {
            name: image.to_string(),
            from: format!("{tag}{old_digest}"),
            to: format!("{new_tag}{digest_part}"),
            kind: ChangeKind::Image,
        });
        format!("{}{image}:{new_tag}{digest_part}{suffix}", &c["prefix"])
    });
    (new.into_owned(), changes)
}

impl Docker {
    fn resolver(
        reg: &Registry,
        frozen: bool,
    ) -> impl Fn(&str, &str) -> Option<(String, Option<String>)> + '_ {
        move |image: &str, tag: &str| {
            // Only Docker Hub images (no registry host in the name) are resolved.
            let host_prefixed = image
                .split('/')
                .next()
                .is_some_and(|h| h.contains('.') || h.contains(':'));
            if host_prefixed {
                return None;
            }
            let new_tag = reg.latest_docker_tag(image, tag)?;
            let digest = if frozen {
                reg.docker_digest(image, &new_tag)
            } else {
                None
            };
            Some((new_tag, digest))
        }
    }
}

impl Updater for Docker {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Docker
    }

    fn detect(&self, root: &Path) -> Detection {
        let mut targets: Vec<PathBuf> = Vec::new();
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if p.is_dir() && !matches!(name, ".git" | "target" | "node_modules" | ".venv") {
                        walk(&p, out);
                    } else if name == "Dockerfile"
                        || name.starts_with("Dockerfile.")
                        || name.ends_with(".dockerfile")
                    {
                        out.push(p);
                    }
                }
            }
        }
        walk(root, &mut targets);
        targets.sort();
        if targets.is_empty() {
            Detection::Absent {
                reason: "no Dockerfile".into(),
            }
        } else {
            Detection::Present { targets }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        let Detection::Present { targets } = det else {
            return UpdateOutcome::not_present("no Dockerfile");
        };
        let frozen = ctx.is_frozen(Ecosystem::Docker);
        let reg = Registry::default();
        let resolver = Docker::resolver(&reg, frozen);
        let mut all_changes = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for df in targets {
            let Ok(text) = std::fs::read_to_string(df) else {
                continue;
            };
            let (new, changes) = rewrite_from(&text, frozen, &resolver);
            if !changes.is_empty() && !ctx.dry_run {
                if let Err(e) = std::fs::write(df, new) {
                    errors.push(format!("write {}: {e}", df.display()));
                    continue;
                }
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
            warnings: Vec::new(),
            errors,
            held_back: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // bump 3.13-slim → 3.14-slim; digest only requested when frozen
    fn resolve(_image: &str, _tag: &str) -> Option<(String, Option<String>)> {
        Some(("3.14-slim".to_string(), Some("sha256:dead".to_string())))
    }

    #[test]
    fn bumps_tag_without_digest_when_not_frozen() {
        let text = "FROM python:3.13-slim\n";
        let (new, changes) = rewrite_from(text, false, resolve);
        assert_eq!(new, "FROM python:3.14-slim\n");
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn appends_digest_when_frozen() {
        let text = "FROM python:3.13-slim\n";
        let (new, _c) = rewrite_from(text, true, resolve);
        assert_eq!(new, "FROM python:3.14-slim@sha256:dead\n");
    }

    #[test]
    fn leaves_unresolvable_lines_untouched() {
        fn none(_i: &str, _t: &str) -> Option<(String, Option<String>)> {
            None
        }
        let text = "FROM scratch\nFROM private.reg/x:1\n";
        let (new, changes) = rewrite_from(text, false, none);
        assert_eq!(new, text);
        assert!(changes.is_empty());
    }
}
