# Contributing to clockpin

> **Continuous Lockfile Pin updates** — keep every pinned dependency in a repo in step with upstream.

Thanks for your interest in improving clockpin! This document explains how to
propose changes, set up a development environment, and get your work merged.

By participating in this project you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- **Report a bug** — open an [issue](https://github.com/SimplicityGuy/clockpin/issues)
  with steps to reproduce, the command you ran, and the output you expected vs. saw.
- **Request a feature** — open an issue describing the problem you're trying to
  solve. Explaining the *why* helps us design the right *what*.
- **Improve the docs** — typos, clarifications, and missing examples are all welcome.
- **Submit code** — bug fixes and new ecosystem support (see below). For anything
  non-trivial, please open an issue first so we can agree on the approach before
  you invest time.

Please **do not** open a public issue for security vulnerabilities — see
[SECURITY.md](SECURITY.md) for private reporting.

## Development setup

clockpin is a single Rust binary. You'll need a recent stable Rust toolchain
(managed via `rust-toolchain.toml`) and [`just`](https://github.com/casey/just)
as the command runner.

```console
git clone https://github.com/SimplicityGuy/clockpin
cd clockpin

just install    # rustup components (rustfmt, clippy) + cargo fetch
just init       # install pre-commit hooks
```

Run `just --list` to see every available recipe.

## Development workflow

```console
just build      # cargo build
just test       # cargo test
just fmt        # cargo fmt
just lint       # cargo clippy --all-targets -- -D warnings
just check      # cargo check --all-targets
```

Before you push, make sure the full quality gate passes locally:

```console
just fmt-check
just lint
just test
```

clockpin dogfoods itself for its own dependency updates:

```console
just deps-check   # preview what clockpin would update in this repo
just deps         # apply updates (freeze all, matching the repo's pinning policy)
```

### Pre-commit hooks

We use [pre-commit](https://pre-commit.com/) to catch formatting and lint issues
before they reach CI. `just init` installs the hooks; they then run automatically
on `git commit`. To run them manually across the repo:

```console
pre-commit run --all-files
```

## Coding guidelines

- **Formatting & lints are non-negotiable.** CI runs `cargo fmt --check` and
  `cargo clippy -D warnings`. The crate also sets `warnings = "deny"`, so any
  compiler warning is a hard error. Run `just fmt` and `just lint` before pushing.
- **Match the surrounding style.** Keep comment density, naming, and idioms
  consistent with the code you're editing.
- **Every ecosystem lives behind the common `Updater` trait** in `src/updaters/`.
  New ecosystem support should follow the existing modules (e.g. `python_uv`,
  `cargo`, `github_actions`) and register itself in `updaters::all()`.
- **Prefer delegating to an ecosystem's own tooling** where one exists; only
  hand-roll updates (as with Docker and GitHub Actions) when there's no tool to
  delegate to.
- **Keep registry reads safe.** HTTP clients in `src/registry.rs` are size-capped
  and take injectable base URLs so tests can point at a mock server.

## Testing

- Add or update tests for any behavior change. Unit tests live alongside the code
  in `#[cfg(test)]` modules; end-to-end CLI tests live in `tests/cli.rs`.
- Network-dependent code is tested against `mockito` mock servers rather than live
  registries — please keep it that way so the suite stays hermetic and fast.
- Run `just test` and confirm everything passes before opening a PR.

## Commit and pull request process

1. **Fork** the repository and create a topic branch from `main`
   (e.g. `fix/docker-digest-resolution`).
1. **Make focused commits** with clear messages. Explain *why* in the body when
   the change isn't self-evident.
1. **Keep PRs scoped.** One logical change per PR is much easier to review.
1. **Fill out the PR description**: what changed, why, and how you verified it.
   Link any related issue (`Fixes #123`).
1. **Ensure CI is green.** PRs must pass formatting, clippy, and the test suite.
1. A maintainer will review your PR. Please be responsive to feedback — small,
   iterative changes get merged faster.

## Releases

Releases use CalVer (`YYYY.MM.MICRO`, e.g. `2026.7.0`) and are cut from `main` by
pushing a `v<version>` tag. `just release-tag` computes and pushes the next tag,
which triggers the release workflow to build and publish binaries. Releases are
handled by maintainers — you don't need to bump versions in your PRs.

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE) that covers the project.
