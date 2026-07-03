#!/usr/bin/env bash
# Verifies that scripts/run.sh maps action inputs to the correct clockpin argv.
# Uses a stub "clockpin" that records the argv it was called with, so no real
# binary, network, or release is needed.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${here}/../.." && pwd)"
run_sh="${repo_root}/scripts/run.sh"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Stub binary: write each argument on its own line to $ARGV_OUT.
stub="${tmp}/clockpin"
cat > "$stub" <<'STUB'
#!/usr/bin/env bash
: > "$ARGV_OUT"
for a in "$@"; do printf '%s\n' "$a" >> "$ARGV_OUT"; done
STUB
chmod +x "$stub"

fail=0
# Run run.sh with the given INPUT_* env and assert argv equals the expected
# newline-joined string. git status is stubbed out via a throwaway repo so the
# `changed` detection never touches the real working tree.
assert_argv() {
  local name="$1" expected="$2"; shift 2
  local argv_out="${tmp}/argv"
  local sandbox="${tmp}/sandbox"
  rm -rf "$sandbox"; mkdir -p "$sandbox"
  ( cd "$sandbox" && git init -q .
    env ARGV_OUT="$argv_out" CLOCKPIN_BIN="$stub" GITHUB_OUTPUT="" "$@" bash "$run_sh" >/dev/null )
  local actual; actual="$(paste -sd'|' "$argv_out")"
  local want; want="$(printf '%s' "$expected" | paste -sd'|' -)"
  if [ "$actual" = "$want" ]; then
    echo "ok   - ${name}"
  else
    echo "FAIL - ${name}"
    echo "       expected: ${want}"
    echo "       actual:   ${actual}"
    fail=1
  fi
}

# Default command is run, no flags.
assert_argv "default run" "run" env INPUT_COMMAND=

# Every run flag set.
assert_argv "all run flags" "$(printf 'run\n--path\nsub/dir\n--dry-run\n--freeze\nactions\ndocker\n--only\ncargo\nuv\n--skip\nnpm\n--major\n--log-json\nout.json\n--verbose\n--quiet\n--no-color')" \
  env INPUT_COMMAND=run INPUT_PATH=sub/dir INPUT_DRY_RUN=true \
      INPUT_FREEZE="actions docker" INPUT_ONLY="cargo,uv" INPUT_SKIP=npm \
      INPUT_MAJOR=true INPUT_LOG_JSON=out.json INPUT_VERBOSE=true \
      INPUT_QUIET=1 INPUT_NO_COLOR=yes

# Falsey booleans are omitted.
assert_argv "falsey booleans omitted" "$(printf 'run\n--freeze\nall')" \
  env INPUT_COMMAND=run INPUT_DRY_RUN=false INPUT_MAJOR=no INPUT_VERBOSE=off \
      INPUT_FREEZE=all

# check passes flags through like run (flag order follows run.sh, not input order).
assert_argv "check with flags" "$(printf 'check\n--dry-run\n--only\ncargo')" \
  env INPUT_COMMAND=check INPUT_ONLY=cargo INPUT_DRY_RUN=true

# list takes only --path; run/check flags are dropped.
assert_argv "list ignores run flags" "$(printf 'list\n--path\n.')" \
  env INPUT_COMMAND=list INPUT_PATH=. INPUT_MAJOR=true INPUT_DRY_RUN=true INPUT_FREEZE=actions

exit $fail
