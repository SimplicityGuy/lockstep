//! Version, name-normalization, and tag-granularity primitives.
//!
//! Only exercised by this module's own unit tests until later tasks wire
//! `normalize`/`version_key`/`desired_tag`/`relax_cap` into `registry.rs` and
//! the updaters (see plan Tasks 4, 6, 8). `warnings = "deny"` turns the
//! interim "never used outside tests" state into a hard build failure for
//! `cargo build`/`cargo clippy` (though not `cargo test`, since the test cfg
//! counts as a use), so silence dead-code here until those call sites land.
#![allow(dead_code)]

use std::sync::OnceLock;

use regex::Regex;

/// PyPI canonical form: lowercase, runs of `-_.` collapsed to a single `-`.
pub fn normalize(name: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[-_.]+").unwrap());
    re.replace_all(name, "-").to_lowercase()
}

/// Best-effort numeric comparison key. Non-numeric segments contribute 0.
pub fn version_key(version: &str) -> Vec<u64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let split = RE.get_or_init(|| Regex::new(r"[.+!\-]").unwrap());
    let lead = |s: &str| -> u64 {
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().unwrap_or(0)
    };
    split.split(version).map(lead).collect()
}

/// Choose the replacement tag while preserving the existing pin granularity:
/// a major-only pin (`v6`) stays major-only (`v7`); a full pin keeps full precision.
pub fn desired_tag(current_ref: &str, latest: &str) -> String {
    static MAJOR_ONLY: OnceLock<Regex> = OnceLock::new();
    static LEAD_MAJOR: OnceLock<Regex> = OnceLock::new();
    let major_only = MAJOR_ONLY.get_or_init(|| Regex::new(r"^v?\d+$").unwrap());
    if major_only.is_match(current_ref) {
        let lead = LEAD_MAJOR.get_or_init(|| Regex::new(r"^v?(\d+)").unwrap());
        if let Some(c) = lead.captures(latest) {
            let prefix = if latest.starts_with('v') || current_ref.starts_with('v') {
                "v"
            } else {
                ""
            };
            return format!("{prefix}{}", &c[1]);
        }
    }
    latest.to_string()
}

/// Raise a `<N…` cap inside a requirement remainder to `<{ceiling}.0.0`.
/// Returns the rewritten remainder, or `None` when there is no `<` cap.
pub fn relax_cap(rest: &str, ceiling: u64) -> Option<String> {
    if !rest.contains('<') {
        return None;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(<\s*)\d+(?:\.\d+)*").unwrap());
    let replaced = re.replace(rest, |c: &regex::Captures| {
        format!("{}{ceiling}.0.0", &c[1])
    });
    if replaced == rest {
        None
    } else {
        Some(replaced.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize("Boto3"), "boto3");
        assert_eq!(normalize("boto3.stubs"), "boto3-stubs");
        assert_eq!(normalize("types__requests"), "types-requests");
        assert_eq!(normalize("A.B-c_D"), "a-b-c-d");
    }

    #[test]
    fn orders_versions_numerically() {
        assert!(version_key("1.34.0") < version_key("1.43.36"));
        assert!(version_key("2.7.0") > version_key("1.26.0"));
        // non-numeric suffix segments do not panic and sort low
        assert!(version_key("2.33.0.20260518") > version_key("2.31.0"));
    }

    #[test]
    fn desired_tag_preserves_granularity() {
        assert_eq!(desired_tag("v6", "v7.0.0"), "v7"); // major-only stays major-only
        assert_eq!(desired_tag("v3", "v3.1.2"), "v3");
        assert_eq!(desired_tag("v1.2.3", "v1.4.0"), "v1.4.0"); // full pin takes full
        assert_eq!(desired_tag("6", "7.0.0"), "7"); // no-v major pin
    }

    #[test]
    fn relax_cap_raises_upper_bound() {
        assert_eq!(relax_cap(",<5.0.0", 6).as_deref(), Some(",<6.0.0"));
        assert_eq!(relax_cap("", 6), None); // no cap present
    }
}
