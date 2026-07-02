#!/usr/bin/env just --justfile

# 🔒 lockstep — keep 'em in step
# Run 'just --list' to see all commands.

default:
    @just --list

# ── Setup ──────────────────────────────────────────────────────────────────
[group('setup')]
install:
    rustup component add rustfmt clippy
    cargo fetch

[group('setup')]
init:
    uv run pre-commit install || pre-commit install
    @echo '✅ pre-commit hooks installed'

[group('setup')]
update-hooks:
    pre-commit autoupdate --freeze

# ── Build ──────────────────────────────────────────────────────────────────
[group('build')]
build:
    cargo build

[group('build')]
build-release:
    cargo build --release

# ── Quality ────────────────────────────────────────────────────────────────
[group('quality')]
fmt:
    cargo fmt

[group('quality')]
fmt-check:
    cargo fmt --check

[group('quality')]
lint:
    cargo clippy --all-targets -- -D warnings

[group('quality')]
check:
    cargo check --all-targets

# ── Test ───────────────────────────────────────────────────────────────────
[group('test')]
test:
    cargo test

[group('test')]
test-verbose:
    cargo test -- --nocapture

# ── Deps (dogfood lockstep on itself) ───────────────────────────────────────
# `--freeze actions` keeps the CI workflow's action pins as immutable SHAs
# (CI is SHA-frozen; lockstep's own freeze is opt-in, so we opt in here).
[group('deps')]
deps:
    cargo run -- run --freeze actions

[group('deps')]
deps-check:
    cargo run -- check --freeze actions

[group('deps')]
deps-major:
    cargo run -- run --major --freeze actions

# ── Clean ──────────────────────────────────────────────────────────────────
[group('clean')]
clean:
    cargo clean
