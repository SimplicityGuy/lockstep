//! HTTPS clients for the package registries clockpin queries. Base URLs are
//! fields so tests can point them at a mock server. All reads are size-capped
//! and time-bounded; any failure yields `None` rather than propagating.

use std::io::Read;
use std::time::Duration;

use serde_json::Value;

const TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MAX_BYTES: usize = 2_000_000;

pub struct Registry {
    pub pypi_base: String,
    pub npm_base: String,
    pub crates_base: String,
    pub docker_base: String,
    pub max_bytes: usize,
}

impl Default for Registry {
    fn default() -> Self {
        Registry {
            pypi_base: "https://pypi.org".into(),
            npm_base: "https://registry.npmjs.org".into(),
            crates_base: "https://crates.io".into(),
            docker_base: "https://hub.docker.com".into(),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

impl Registry {
    /// GET a URL and parse JSON, capping the body and swallowing all errors.
    pub fn get_json(&self, url: &str) -> Option<Value> {
        let resp = ureq::get(url).timeout(TIMEOUT).call().ok()?;
        let mut buf = String::new();
        resp.into_reader()
            .take((self.max_bytes as u64) + 1)
            .read_to_string(&mut buf)
            .ok()?;
        if buf.len() > self.max_bytes {
            return None;
        }
        serde_json::from_str(&buf).ok()
    }

    // reserved for held-back-major reporting; not yet wired (see docs/design.md)
    #[allow(dead_code)]
    pub fn latest_pypi(&self, name: &str) -> Option<String> {
        let v = self.get_json(&format!("{}/pypi/{name}/json", self.pypi_base))?;
        v.get("info")?.get("version")?.as_str().map(String::from)
    }

    // reserved for held-back-major reporting; not yet wired (see docs/design.md)
    #[allow(dead_code)]
    pub fn latest_npm(&self, name: &str) -> Option<String> {
        let v = self.get_json(&format!("{}/{name}/latest", self.npm_base))?;
        v.get("version")?.as_str().map(String::from)
    }

    // reserved for held-back-major reporting; not yet wired (see docs/design.md)
    #[allow(dead_code)]
    pub fn latest_crate(&self, name: &str) -> Option<String> {
        let v = self.get_json(&format!("{}/api/v1/crates/{name}", self.crates_base))?;
        v.get("crate")?
            .get("max_stable_version")
            .and_then(Value::as_str)
            .map(String::from)
    }

    /// Newest tag for `image` whose shape matches `current_tag`'s granularity
    /// (e.g. `3.13-slim` stays on the `3.13-slim` line). Docker Hub library
    /// images are namespaced `library/<name>`.
    pub fn latest_docker_tag(&self, image: &str, current_tag: &str) -> Option<String> {
        let repo = if image.contains('/') {
            image.to_string()
        } else {
            format!("library/{image}")
        };
        let url = format!(
            "{}/v2/repositories/{repo}/tags?page_size=100&ordering=last_updated",
            self.docker_base
        );
        let v = self.get_json(&url)?;
        let tags = v.get("results")?.as_array()?;
        let mut best: Option<(Vec<u64>, String)> = None;
        for t in tags {
            let name = t.get("name").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() || !shape_matches(name, current_tag) {
                continue;
            }
            let key = crate::version::version_key(name);
            if best.as_ref().is_none_or(|(bk, _)| &key > bk) {
                best = Some((key, name.to_string()));
            }
        }
        best.map(|(_, n)| n)
    }

    /// Resolve the manifest digest (`sha256:…`) for `image:tag` on Docker Hub.
    pub fn docker_digest(&self, image: &str, tag: &str) -> Option<String> {
        let repo = if image.contains('/') {
            image.to_string()
        } else {
            format!("library/{image}")
        };
        let url = format!("{}/v2/repositories/{repo}/tags/{tag}", self.docker_base);
        let v = self.get_json(&url)?;
        v.get("digest")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| {
                v.get("images")?
                    .as_array()?
                    .first()?
                    .get("digest")?
                    .as_str()
                    .map(String::from)
            })
    }
}

/// Leading numeric components of a docker tag, before any non-numeric suffix.
/// "3.13-slim" -> [3,13]; "22.04" -> [22,4]; "latest" -> [].
fn numeric_components(tag: &str) -> Vec<u64> {
    let numeric: String = tag
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    numeric
        .split('.')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// Two tags share a "shape" when their non-numeric suffix (e.g. `-slim`, `-alpine`)
/// is identical AND the current tag's leading numeric components are a prefix of
/// the candidate's — i.e. the candidate stays on the same numeric line.
/// `3.13-slim` matches `3.13-slim` and `3.13.5-slim`, but not `3.14-slim` or
/// `3.14-alpine`; `22.04` matches `22.04` and `22.04.3` but not `24.04`.
fn shape_matches(candidate: &str, current: &str) -> bool {
    fn suffix(tag: &str) -> String {
        tag.chars()
            .skip_while(|c| c.is_ascii_digit() || *c == '.')
            .collect()
    }
    fn is_numeric_lead(tag: &str) -> bool {
        tag.chars().next().is_some_and(|c| c.is_ascii_digit())
    }
    if !is_numeric_lead(candidate) || suffix(candidate) != suffix(current) {
        return false;
    }
    let cur = numeric_components(current);
    let cand = numeric_components(candidate);
    cur.len() <= cand.len() && cand[..cur.len()] == cur[..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(server: &mockito::Server) -> Registry {
        let base = server.url();
        Registry {
            pypi_base: base.clone(),
            npm_base: base.clone(),
            crates_base: base.clone(),
            docker_base: base,
            max_bytes: 1_000_000,
        }
    }

    #[test]
    fn parses_latest_pypi() {
        let mut s = mockito::Server::new();
        let _m = s
            .mock("GET", "/pypi/orjson/json")
            .with_body(r#"{"info":{"version":"3.11.9"}}"#)
            .create();
        assert_eq!(reg(&s).latest_pypi("orjson").as_deref(), Some("3.11.9"));
    }

    #[test]
    fn parses_latest_npm() {
        let mut s = mockito::Server::new();
        let _m = s
            .mock("GET", "/left-pad/latest")
            .with_body(r#"{"version":"1.3.0"}"#)
            .create();
        assert_eq!(reg(&s).latest_npm("left-pad").as_deref(), Some("1.3.0"));
    }

    #[test]
    fn parses_latest_crate() {
        let mut s = mockito::Server::new();
        let _m = s
            .mock("GET", "/api/v1/crates/serde")
            .with_body(r#"{"crate":{"max_stable_version":"1.0.215"}}"#)
            .create();
        assert_eq!(reg(&s).latest_crate("serde").as_deref(), Some("1.0.215"));
    }

    #[test]
    fn none_on_http_error() {
        let mut s = mockito::Server::new();
        let _m = s.mock("GET", "/pypi/ghost/json").with_status(404).create();
        assert_eq!(reg(&s).latest_pypi("ghost"), None);
    }

    #[test]
    fn latest_docker_tag_stays_on_numeric_line() {
        let mut s = mockito::Server::new();
        let _m = s
            .mock(
                "GET",
                "/v2/repositories/library/python/tags?page_size=100&ordering=last_updated",
            )
            .with_body(
                r#"{"results":[
                    {"name":"3.13-slim"},
                    {"name":"3.13.5-slim"},
                    {"name":"3.14-slim"},
                    {"name":"3.14.2-slim"},
                    {"name":"3-slim"}
                ]}"#,
            )
            .create();
        // Must stay on the 3.13 line (3.13.5-slim), never cross into 3.14.x.
        assert_eq!(
            reg(&s).latest_docker_tag("python", "3.13-slim").as_deref(),
            Some("3.13.5-slim")
        );
    }
}
