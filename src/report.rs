//! Screen reporting (emoji-forward, color-aware) and the JSON run log.

use std::path::Path;

use console::style;
use serde::Serialize;

use crate::context::Ecosystem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Applied,
    WouldChange,
    UpToDate,
    NotPresent,
    Warned,
    Errored,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Applied => "applied",
            Status::WouldChange => "would-change",
            Status::UpToDate => "up-to-date",
            Status::NotPresent => "not-present",
            Status::Warned => "skipped",
            Status::Errored => "error",
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Status::Applied => "✅",
            Status::WouldChange => "📝",
            Status::UpToDate => "✅",
            Status::NotPresent => "⏭️",
            Status::Warned => "⚠️",
            Status::Errored => "❌",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Bump,
    Pin,
    Image,
}

#[derive(Clone, Debug, Serialize)]
pub struct Change {
    pub name: String,
    pub from: String,
    pub to: String,
    pub kind: ChangeKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct HeldBackMajor {
    pub ecosystem: String,
    pub name: String,
    pub current: String,
    pub latest: String,
}

pub struct Reporter {
    pub quiet: bool,
    pub verbose: bool,
    pub color: bool,
}

impl Reporter {
    pub fn header(&self) {
        if !self.quiet {
            println!("\n  🔒 {} · keeping 'em in step\n", self.emph("lockstep"));
        }
    }

    pub fn detected(&self, present: &[Ecosystem], absent: &[(Ecosystem, String)]) {
        if self.quiet {
            return;
        }
        let p = present
            .iter()
            .map(|e| e.key())
            .collect::<Vec<_>>()
            .join(" · ");
        let a = absent
            .iter()
            .map(|(e, _)| e.key())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  🔎 detected: {p}   (absent: {a})\n");
        if self.verbose {
            for (e, reason) in absent {
                println!("      {} {}: {reason}", e.emoji(), e.key());
            }
        }
    }

    pub fn line(&self, eco: Ecosystem, out: &crate::updaters::UpdateOutcome, frozen: bool) {
        if self.quiet && matches!(out.status, Status::UpToDate | Status::NotPresent) {
            return;
        }
        let detail = match out.status {
            Status::NotPresent => out
                .warnings
                .first()
                .cloned()
                .unwrap_or_else(|| "not present".into()),
            Status::Warned | Status::Errored => out
                .warnings
                .iter()
                .chain(&out.errors)
                .cloned()
                .collect::<Vec<_>>()
                .join("; "),
            _ if out.changes.is_empty() => "up to date".into(),
            _ => out
                .changes
                .iter()
                .map(|c| format!("{} {}→{}", c.name, c.from, c.to))
                .collect::<Vec<_>>()
                .join(" · "),
        };
        let frozen_tag = if frozen && !out.changes.is_empty() {
            "  🔒 frozen"
        } else {
            ""
        };
        println!(
            "  {} {:<11} {} {detail}{frozen_tag}",
            eco.emoji(),
            eco.key(),
            out.status.glyph()
        );
    }

    pub fn held_back(&self, held: &[HeldBackMajor]) {
        if self.quiet || held.is_empty() {
            return;
        }
        let n = held.len();
        println!("\n  ⚠️  {n} major upgrade(s) held back by a cap:");
        for h in held {
            println!(
                "      {} {} → {}     run with --major to apply",
                h.name, h.current, h.latest
            );
        }
    }

    pub fn summary(&self, updated: usize, ecosystems: usize, errors: usize) {
        println!(
            "\n  ✨ updated {updated} dependencies across {ecosystems} ecosystems · {errors} errors"
        );
    }

    fn emph(&self, s: &str) -> String {
        if self.color {
            style(s).bold().to_string()
        } else {
            s.to_string()
        }
    }
}

// ---- JSON run log -------------------------------------------------------

#[derive(Serialize)]
pub struct RunLog {
    pub tool: String,
    pub version: String,
    pub repo_root: String,
    pub options: OptionsLog,
    pub ecosystems: Vec<EcoLog>,
    pub held_back_majors: Vec<HeldBackMajor>,
    pub summary: SummaryLog,
}

#[derive(Serialize)]
pub struct OptionsLog {
    pub dry_run: bool,
    pub freeze: Vec<String>,
    pub major: bool,
    pub only: Vec<String>,
    pub skip: Vec<String>,
}

#[derive(Serialize)]
pub struct EcoLog {
    pub key: String,
    pub detected: bool,
    pub tool_available: bool,
    pub status: String,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Serialize)]
pub struct SummaryLog {
    pub updated: usize,
    pub ecosystems_touched: usize,
    pub errors: usize,
}

pub fn write_json_log(path: &Path, log: &RunLog) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(log)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_kind_serializes_lowercase() {
        let c = Change {
            name: "orjson".into(),
            from: "3.11.7".into(),
            to: "3.11.9".into(),
            kind: ChangeKind::Bump,
        };
        let j = serde_json::to_value(&c).unwrap();
        assert_eq!(j["kind"], "bump");
    }

    #[test]
    fn run_log_shape_is_stable() {
        let log = RunLog {
            tool: "lockstep".into(),
            version: "0.1.0".into(),
            repo_root: "/x".into(),
            options: OptionsLog {
                dry_run: false,
                freeze: vec!["actions".into()],
                major: false,
                only: vec![],
                skip: vec![],
            },
            ecosystems: vec![],
            held_back_majors: vec![],
            summary: SummaryLog {
                updated: 0,
                ecosystems_touched: 0,
                errors: 0,
            },
        };
        let j = serde_json::to_value(&log).unwrap();
        assert_eq!(j["tool"], "lockstep");
        assert_eq!(j["options"]["freeze"][0], "actions");
    }
}
