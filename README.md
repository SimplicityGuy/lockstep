# lockstep

> Keep every pinned dependency in a repo in step with upstream.

`lockstep` is a single Rust binary you run in any repository. It auto-detects the
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

## Usage

```console
lockstep run                 # detect + apply updates (default)
lockstep check               # report only — never writes
lockstep list                # show detected ecosystems + toolchain availability
lockstep completions zsh     # shell completions

# common flags
lockstep run --dry-run                 # preview
lockstep run --freeze actions docker   # pin those tools to SHA/digest
lockstep run --only cargo uv           # restrict
lockstep run --skip docker             # exclude
lockstep run --major                   # pull held-back majors
lockstep run --log-json run.json       # also write a JSON run log
```

## Freezing

Freezing is **opt-in, per tool**. By default lockstep writes plain tags/versions.
`--freeze` pins the selected freezable tools to an immutable ref:

- `--freeze actions` → commit SHA + `# frozen: vX.Y.Z`
- `--freeze pre-commit` → `pre-commit autoupdate --freeze`
- `--freeze docker` → `@sha256:…` digest on the bumped tag
- `--freeze all` → all three

## Development

```console
just install    # toolchain + fetch
just test       # cargo test
just lint       # clippy -D warnings
just deps       # dogfood: run lockstep on this repo
```

## Notes & limitations (v1)

- **Delegated ecosystems don't enumerate changes.** npm, cargo, pre-commit, and
  requirements delegate entirely to their underlying tool, which doesn't report
  individual package changes back to lockstep. A successful `lockstep run` shows
  these ecosystems as up to date, and `lockstep check` optimistically shows them
  as would-change, since lockstep can't preview what the tool would do without
  actually running it. Run `git diff` afterward to see exactly what changed.
- **`--major` is asymmetric.** It's honored by npm and cargo (pull the latest
  major); uv and requirements ignore it in v1 — cap relaxation isn't wired up
  for those ecosystems yet.
- **JVM ecosystems are out of scope.** Maven and Gradle are not supported in v1.
- **Docker resolves public registries only.** Image tag/digest resolution covers
  Docker Hub and GHCR; private or self-hosted registries are not supported.

## License

MIT — see [LICENSE](LICENSE).
