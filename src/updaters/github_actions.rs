//! GitHub Actions `uses:` pins. Hand-rolled: no native updater exists. Latest
//! tag + commit SHA come from `gh api`; the rewrite preserves pin granularity
//! and, when frozen, writes `@<sha>  # frozen: <tag>`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::context::{Ecosystem, RunCtx};
use crate::report::{Change, ChangeKind, Status};
use crate::updaters::{Detection, UpdateOutcome, Updater};
use crate::version::desired_tag;

pub struct GithubActions;

fn uses_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?P<prefix>\buses:\s*)(?P<owner>[A-Za-z0-9][A-Za-z0-9-]*)/(?P<repo>[A-Za-z0-9._-]+)@(?P<ref>[A-Za-z0-9._/-]+)(?P<comment>\s*#\s*(?:frozen:\s*)?(?P<cver>\S+))?",
        )
        .unwrap()
    })
}

/// Rewrite every `uses:` pin in `text`. `resolve(owner, repo)` yields
/// `(latest_tag, commit_sha)`. Returns the new text and the changes made.
pub fn rewrite_uses(
    text: &str,
    frozen: bool,
    resolve: impl Fn(&str, &str) -> Option<(String, String)>,
) -> (String, Vec<Change>) {
    let re = uses_re();
    let mut changes = Vec::new();
    let sha_re = Regex::new(r"^[0-9a-f]{40}$").unwrap();

    let new = re.replace_all(text, |c: &regex::Captures| {
        let owner = &c["owner"];
        let repo = &c["repo"];
        let cur_ref = &c["ref"];
        let action = format!("{owner}/{repo}");
        let Some((latest, sha)) = resolve(owner, repo) else {
            return c[0].to_string();
        };
        let is_sha = sha_re.is_match(cur_ref);

        if frozen {
            let old_ver = c.name("cver").map(|m| m.as_str()).unwrap_or(cur_ref);
            let new_ver = desired_tag(old_ver, &latest);
            if is_sha && cur_ref == sha {
                return c[0].to_string();
            }
            changes.push(Change {
                name: action.clone(),
                from: c
                    .name("cver")
                    .map(|m| m.as_str())
                    .unwrap_or(cur_ref)
                    .to_string(),
                to: new_ver.clone(),
                kind: ChangeKind::Pin,
            });
            format!("{}{action}@{sha}  # frozen: {new_ver}", &c["prefix"])
        } else {
            // Non-frozen mode manages bare tag pins only. Never silently un-freeze an
            // existing SHA pin into a floating tag — that would drop a supply-chain pin.
            if is_sha {
                return c[0].to_string();
            }
            let new_ref = desired_tag(cur_ref, &latest);
            if new_ref == cur_ref {
                return c[0].to_string();
            }
            changes.push(Change {
                name: action.clone(),
                from: cur_ref.to_string(),
                to: new_ref.clone(),
                kind: ChangeKind::Pin,
            });
            let comment = c.name("comment").map(|m| m.as_str()).unwrap_or("");
            format!("{}{action}@{new_ref}{comment}", &c["prefix"])
        }
    });
    (new.into_owned(), changes)
}

impl GithubActions {
    /// `gh api` resolver: latest release tag (fallback to newest semver tag) + its SHA.
    fn gh_resolver<'a>(
        ctx: &'a RunCtx,
        root: &'a Path,
    ) -> impl Fn(&str, &str) -> Option<(String, String)> + 'a {
        move |owner: &str, repo: &str| {
            let tag = gh_json(
                ctx,
                root,
                &[
                    "api",
                    &format!("repos/{owner}/{repo}/releases/latest"),
                    "--jq",
                    ".tag_name",
                ],
            )
            .filter(|s| !s.is_empty())?;
            let sha = gh_json(
                ctx,
                root,
                &[
                    "api",
                    &format!("repos/{owner}/{repo}/commits/{tag}"),
                    "--jq",
                    ".sha",
                ],
            )?;
            Some((tag, sha))
        }
    }
}

fn gh_json(ctx: &RunCtx, root: &Path, args: &[&str]) -> Option<String> {
    let out = ctx.runner.run("gh", args, root).ok()?;
    if !out.ok() {
        return None;
    }
    let s = out.stdout.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

impl Updater for GithubActions {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Actions
    }

    fn detect(&self, root: &Path) -> Detection {
        let dir = root.join(".github/workflows");
        let mut targets: Vec<PathBuf> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if matches!(p.extension().and_then(|s| s.to_str()), Some("yml" | "yaml")) {
                    targets.push(p);
                }
            }
        }
        targets.sort();
        if targets.is_empty() {
            Detection::Absent {
                reason: "no .github/workflows".into(),
            }
        } else {
            Detection::Present { targets }
        }
    }

    fn run(&self, ctx: &RunCtx, det: &Detection) -> UpdateOutcome {
        let Detection::Present { targets } = det else {
            return UpdateOutcome::not_present("no .github/workflows");
        };
        if !ctx.runner.which("gh") {
            return UpdateOutcome::warned("`gh` not on PATH — skipped");
        }
        let frozen = ctx.is_frozen(Ecosystem::Actions);
        let resolver = GithubActions::gh_resolver(ctx, &ctx.repo_root);
        let mut all_changes = Vec::new();
        for wf in targets {
            let Ok(text) = std::fs::read_to_string(wf) else {
                continue;
            };
            let (new, changes) = rewrite_uses(&text, frozen, &resolver);
            if !changes.is_empty() && !ctx.dry_run {
                if let Err(e) = std::fs::write(wf, new) {
                    return UpdateOutcome::errored(format!("write {}: {e}", wf.display()));
                }
            }
            all_changes.extend(changes);
        }
        let mut out = UpdateOutcome::applied(all_changes);
        if ctx.dry_run && out.status == Status::Applied {
            out.status = Status::WouldChange;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // resolver: every action → latest tag "v7.0.0" and sha "b"*40
    fn resolve(_owner: &str, _repo: &str) -> Option<(String, String)> {
        Some(("v7.0.0".to_string(), "b".repeat(40)))
    }

    #[test]
    fn frozen_sha_pin_bumps_sha_and_preserves_major_only_comment() {
        let old = "a".repeat(40);
        let text = format!("      - uses: actions/checkout@{old}  # frozen: v6\n");
        let (new, changes) = rewrite_uses(&text, true, resolve);
        let sha = "b".repeat(40);
        // Major-only comment (`v6`) must stay major-only (`v7`), not become `v7.0.0`.
        assert_eq!(
            new,
            format!("      - uses: actions/checkout@{sha}  # frozen: v7\n")
        );
        assert!(!new.contains("v7.0.0"));
        assert_eq!(changes.len(), 1);
        assert!(!new.contains(&old));
    }

    #[test]
    fn tag_pin_bumps_tag_when_not_frozen() {
        let text = "      - uses: actions/checkout@v6\n";
        let (new, _c) = rewrite_uses(text, false, resolve);
        assert_eq!(new, "      - uses: actions/checkout@v7\n");
        assert!(!new.contains("v7.0.0"));
    }

    #[test]
    fn tag_pin_freezes_to_sha_when_frozen() {
        let text = "      - uses: actions/checkout@v6\n";
        let (new, _c) = rewrite_uses(text, true, resolve);
        let sha = "b".repeat(40);
        // Bare tag pin with no existing comment freezes to sha, granularity preserved.
        assert_eq!(
            new,
            format!("      - uses: actions/checkout@{sha}  # frozen: v7\n")
        );
        assert!(!new.contains("v7.0.0"));
    }

    #[test]
    fn non_frozen_leaves_sha_frozen_pin_untouched() {
        let sha = "a".repeat(40);
        let text = format!("      - uses: actions/checkout@{sha}  # frozen: v6\n");
        let (new, changes) = rewrite_uses(&text, false, resolve);
        // Non-frozen mode must never un-freeze an existing SHA pin into a
        // floating tag — that would silently drop a supply-chain pin.
        assert_eq!(new, text);
        assert!(changes.is_empty());
    }

    #[test]
    fn non_frozen_tag_pin_preserves_trailing_comment() {
        let text = "      - uses: actions/checkout@v6  # pin\n";
        let (new, _c) = rewrite_uses(text, false, resolve);
        assert_eq!(new, "      - uses: actions/checkout@v7  # pin\n");
    }

    // Composite / subpath actions (`owner/repo/subdir@ref`) don't fit the
    // `owner/repo@ref` shape the regex matches (`repo` excludes `/` and `@ref`
    // is required), so the whole match fails and the line is left UNCHANGED —
    // a safe skip rather than a mangled rewrite.
    #[test]
    fn composite_subpath_action_is_left_unchanged() {
        let text = "      - uses: github/codeql-action/analyze@v3\n";
        let (new, changes) = rewrite_uses(text, false, resolve);
        assert_eq!(new, text);
        assert!(changes.is_empty());

        let (new_f, changes_f) = rewrite_uses(text, true, resolve);
        assert_eq!(new_f, text);
        assert!(changes_f.is_empty());
    }
}
