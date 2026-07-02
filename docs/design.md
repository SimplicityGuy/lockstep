# clockpin design

A concise, committed record of how clockpin is built. (Working specs and plans
live under `docs/superpowers/`, which is gitignored.)

## Shape

- Shared core: `version` (parsing/granularity), `context` (Ecosystem, RunCtx,
  injectable Runner), `registry` (HTTP clients), `report` (screen + JSON log).
- One `Updater` per ecosystem behind a common trait; `updaters::all()` lists them
  in run order (`uv → requirements → npm → cargo → docker → actions → pre-commit`).
- CLI (`clap` derive) → `RunCtx` → orchestrator that detects, filters, runs,
  reports, and optionally writes a JSON log.

## Key decisions

- **Delegate to native tooling.** Only `actions` and `docker` are hand-rolled.
  Missing toolchain ⇒ warn + skip.
- **Freeze is opt-in per tool** (`--freeze actions|pre-commit|docker|all`); default
  writes plain tags/versions. Granularity is preserved when bumping.
- **Testability.** Process execution goes through `Runner` (fake in tests);
  registry access goes through `Registry` (mock server in tests). Pure helpers
  (version keys, tag granularity, `uses:`/`FROM` rewriters, bound bumping) are
  unit-tested directly.
- **Deferred:** held-back-major reporting and `--major` cap relaxation are not
  yet wired — `registry::Registry::latest_pypi`/`latest_npm`/`latest_crate` and
  `version::relax_cap` exist and are tested, but no updater calls them yet.
