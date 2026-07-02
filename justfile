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

# ── Release (CalVer YYYY.MM.MICRO) ─────────────────────────────────────────
# Compute the next release tag: today's year.month, MICRO = next unused count
# for that year+month among existing v* tags (0 if none). Private helper.
_next-tag:
    #!/usr/bin/env bash
    set -euo pipefail
    ym="$(date +%Y).$((10#$(date +%m)))"
    micro=0
    for t in $(git tag --list "v${ym}.*"); do
        m="${t##*.}"
        if [[ "$m" =~ ^[0-9]+$ ]] && (( m >= micro )); then micro=$((m + 1)); fi
    done
    echo "v${ym}.${micro}"

# Print the next release tag without creating it
[group('release')]
release-dry:
    @echo "next release tag: $(just _next-tag)"

# Tag the next CalVer release and push it (triggers the release workflow)
[group('release')]
release-tag:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "$(git status --porcelain)" ]; then
        echo "error: working tree is dirty — commit or stash first" >&2
        exit 1
    fi
    branch="$(git branch --show-current)"
    if [ "$branch" != "main" ]; then
        echo "error: not on main (on '$branch') — releases are cut from main" >&2
        exit 1
    fi
    tag="$(just _next-tag)"
    echo "tagging and pushing $tag ..."
    git tag "$tag"
    git push origin "$tag"
    echo "✅ pushed $tag — the Release workflow will build and publish it"

# ── Clean ──────────────────────────────────────────────────────────────────
[group('clean')]
clean:
    cargo clean
