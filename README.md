# clockpin

> **Continuous Lockfile Pin updates** — keep every pinned dependency in a repo in step with upstream.

`clockpin` is a single Rust binary you run in any repository. It auto-detects the
dependency ecosystems present and updates each one — delegating to that
ecosystem's own tooling where one exists, and hand-rolling GitHub Actions and
Dockerfile updates where none does.

## Ecosystems

| Ecosystem             | Detected by                  | Engine                              |
| --------------------- | ---------------------------- | ----------------------------------- |
| uv (Python)           | `pyproject.toml` + `uv.lock` | `uv lock --upgrade` + bound bumping |
| requirements (Python) | `requirements*.in`           | `uv pip compile --upgrade`          |
| npm                   | `package.json`               | `npm-check-updates` + `npm install` |
| cargo                 | `Cargo.toml`                 | `cargo upgrade` + `cargo update`    |
| docker                | `Dockerfile*`                | registry tag bump (hand-rolled)     |
| actions               | `.github/workflows/*.yml`    | `uses:` pin rewrite (hand-rolled)   |
| pre-commit            | `.pre-commit-config.yaml`    | `pre-commit autoupdate`             |

A detected ecosystem whose toolchain isn't installed is **skipped with a warning**,
not an error.

## Install

Prebuilt binaries for Linux (x86_64, aarch64), macOS (Intel, Apple Silicon), and
Windows (x86_64) are attached to each [GitHub Release](https://github.com/SimplicityGuy/clockpin/releases),
along with a `SHA256SUMS` file. Download the archive for your platform, verify the
checksum, and put `clockpin` on your `PATH`. Or build from source with `cargo build --release`.

Releases use CalVer (`YYYY.MM.MICRO`, e.g. `2026.7.0`) and are cut by pushing a
`v<version>` tag; `just release-tag` computes and pushes the next tag for you.

## Usage

```console
clockpin run                 # detect + apply updates (default)
clockpin check               # report only — never writes
clockpin list                # show detected ecosystems + toolchain availability
clockpin completions zsh     # shell completions

# common flags
clockpin run --dry-run                 # preview
clockpin run --freeze actions docker   # pin those tools to SHA/digest
clockpin run --only cargo uv           # restrict
clockpin run --skip docker             # exclude
clockpin run --major                   # pull held-back majors
clockpin run --log-json run.json       # also write a JSON run log
```

## Freezing

Freezing is **opt-in, per tool**. By default clockpin writes plain tags/versions.
`--freeze` pins the selected freezable tools to an immutable ref:

- `--freeze actions` → commit SHA + `# frozen: vX.Y.Z`
- `--freeze pre-commit` → `pre-commit autoupdate --freeze`
- `--freeze docker` → `@sha256:…` digest on the bumped tag
- `--freeze all` → all three

## GitHub Action

Run clockpin in CI and let it open a pull request with the updates. The action
downloads the prebuilt clockpin binary for the runner, runs the subcommand you
choose, and — for `run` — commits any changes to a branch and opens (or updates)
a PR.

```yaml
name: Update dependencies
on:
  schedule:
    - cron: "0 6 * * 1" # Mondays at 06:00 UTC
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write

jobs:
  clockpin:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: SimplicityGuy/clockpin@v2026.7.0
        with:
          freeze: actions docker # optional: pin these to SHA/digest
          skip: npm # optional: exclude ecosystems
```

The action ships in this repo, so it shares clockpin's CalVer tags: pin it to a
release tag like `@v2026.7.0` (or a commit SHA). By default the action installs
the clockpin binary **matching the tag you pinned**, so the action and the CLI
stay in lockstep — no separate versioning to track. Override with the `version`
input (`latest`, or a specific `vYYYY.MM.MICRO`) if you need to.

The action needs `contents: write` and `pull-requests: write`. To let it update
dependencies **inside `.github/workflows/`**, the default `GITHUB_TOKEN` isn't
enough — pass a PAT (or GitHub App token) via the `token` input.

### Inputs

Every CLI option is exposed. List inputs accept space- or comma-separated
values; booleans are `true`/`false`.

| Input      | CLI flag                    | Default     |
| ---------- | --------------------------- | ----------- |
| `command`  | `run` / `check` / `list`    | `run`       |
| `version`  | clockpin release to install | matches tag |
| `path`     | `--path`                    |             |
| `dry-run`  | `--dry-run`                 | `false`     |
| `freeze`   | `--freeze`                  |             |
| `only`     | `--only`                    |             |
| `skip`     | `--skip`                    |             |
| `major`    | `--major`                   | `false`     |
| `log-json` | `--log-json`                |             |
| `verbose`  | `--verbose`                 | `false`     |
| `quiet`    | `--quiet`                   | `false`     |
| `no-color` | `--no-color`                | `false`     |

Pull-request inputs (used only when `command: run`): `create-pr` (default
`true`), `token`, `branch` (`clockpin/updates`), `base`, `commit-message`,
`pr-title`, `pr-body`, `pr-labels` (`dependencies`), `pr-draft`,
`commit-user-name`, `commit-user-email`.

### Outputs

| Output                | Meaning                                      |
| --------------------- | -------------------------------------------- |
| `changed`             | `true` if clockpin modified any tracked file |
| `clockpin-version`    | resolved installed version                   |
| `pull-request-number` | PR number, when created/updated              |
| `pull-request-url`    | PR URL, when created/updated                 |

Set `create-pr: false` (or use `command: check`) to run without opening a PR —
handy for validating dependencies on every push. A full example lives in
[`docs/example-workflow.yml`](docs/example-workflow.yml).

## Development

```console
just install    # toolchain + fetch
just test       # cargo test
just lint       # clippy -D warnings
just deps       # dogfood: run clockpin on this repo
```

## Notes & limitations (v1)

- **Delegated ecosystems don't enumerate changes.** npm, cargo, pre-commit, and
  requirements delegate entirely to their underlying tool, which doesn't report
  individual package changes back to clockpin. A successful `clockpin run` shows
  these ecosystems as up to date, and `clockpin check` optimistically shows them
  as would-change, since clockpin can't preview what the tool would do without
  actually running it. Run `git diff` afterward to see exactly what changed.
- **`--major` is asymmetric.** It's honored by npm and cargo (pull the latest
  major); uv and requirements ignore it in v1 — cap relaxation isn't wired up
  for those ecosystems yet.
- **JVM ecosystems are out of scope.** Maven and Gradle are not supported in v1.
- **Docker resolves public registries only.** Image tag/digest resolution covers
  Docker Hub and GHCR; private or self-hosted registries are not supported.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for how to set
up a development environment and submit changes. Participation is governed by our
[Code of Conduct](CODE_OF_CONDUCT.md). To report a security issue, please follow
the [Security Policy](SECURITY.md) rather than opening a public issue.

## License

MIT — see [LICENSE](LICENSE).
