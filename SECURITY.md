# Security Policy

## Supported versions

clockpin is distributed as a single binary released under CalVer
(`YYYY.MM.MICRO`). Security fixes are applied to the **latest release** and shipped
in a new release. There are no long-term support branches; please upgrade to the
most recent version to receive fixes.

| Version        | Supported          |
| -------------- | ------------------ |
| Latest release | :white_check_mark: |
| Older releases | :x:                |

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues,
discussions, or pull requests.**

Instead, report them privately using either of the following:

- **GitHub Security Advisories** (preferred): use the
  ["Report a vulnerability"](https://github.com/SimplicityGuy/clockpin/security/advisories/new)
  button on the repository's **Security** tab. This opens a private advisory
  visible only to you and the maintainers.
- **Email**: <robert@simplicityguy.com> with the subject line
  `clockpin security`.

To help us triage quickly, please include as much of the following as you can:

- The version of clockpin affected (`clockpin --version`).
- Your operating system and architecture.
- A description of the vulnerability and its potential impact.
- Step-by-step instructions to reproduce it, including the command(s) run.
- Any proof-of-concept, logs, or affected configuration.

## What to expect

- **Acknowledgement** within **3 business days** of your report.
- An initial **assessment and severity triage** within **7 days**.
- Regular updates on remediation progress, at least every **7 days** until resolved.
- Coordinated disclosure: we'll work with you on timing and will credit you in the
  release notes and advisory unless you prefer to remain anonymous.

We ask that you give us a reasonable opportunity to release a fix before any public
disclosure. We greatly appreciate responsible disclosure and your help keeping
clockpin's users safe.

## Scope and threat model

clockpin runs locally in a repository and:

- **Executes external toolchains** (e.g. `uv`, `npm`, `npm-check-updates`,
  `cargo`, `cargo upgrade`, `pre-commit`) that it detects. clockpin trusts these
  tools; running clockpin implies you trust the toolchains installed on your
  machine and the dependency manifests in the repository it operates on.
- **Makes outbound HTTPS requests** to public package registries and container
  registries (crates.io, npm, PyPI, Docker Hub, GHCR, the GitHub API) to resolve
  the latest versions, tags, and digests. Registry reads are size-capped.
- **Writes to dependency manifests and lockfiles** in the target repository.

Reports that are especially valuable include: command or argument injection into
the tools clockpin invokes, unsafe handling of registry responses, path traversal
when writing manifests, or any way clockpin could be induced to run untrusted code.

Thank you for helping keep clockpin and its community secure.
