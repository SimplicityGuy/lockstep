#!/usr/bin/env bash
# Assemble clockpin's argv from the action inputs, run it, and report whether
# the working tree changed.
#
# Environment (all optional except INPUT_COMMAND):
#   INPUT_COMMAND   run | check | list                (default: run)
#   INPUT_PATH      --path value
#   INPUT_DRY_RUN   --dry-run       (truthy)
#   INPUT_FREEZE    --freeze  values (space/comma list)
#   INPUT_ONLY      --only    values (space/comma list)
#   INPUT_SKIP      --skip    values (space/comma list)
#   INPUT_MAJOR     --major         (truthy)
#   INPUT_LOG_JSON  --log-json value
#   INPUT_VERBOSE   --verbose       (truthy)
#   INPUT_QUIET     --quiet         (truthy)
#   INPUT_NO_COLOR  --no-color      (truthy)
#   CLOCKPIN_BIN    binary to invoke (default: clockpin on PATH)
#
# Sets the `changed` step output.
set -euo pipefail

BIN="${CLOCKPIN_BIN:-clockpin}"
command="${INPUT_COMMAND:-run}"

case "$command" in
  run|check|list) ;;
  *) echo "::error::invalid command '${command}' (expected run, check, or list)" >&2; exit 1 ;;
esac

is_true() {
  case "$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]')" in
    true|1|yes|on) return 0 ;;
    *) return 1 ;;
  esac
}

# Append a repeated value-list flag, splitting on commas and whitespace.
append_list() {
  local flag="$1" raw="$2" tok
  [ -n "$raw" ] || return 0
  args+=("$flag")
  for tok in ${raw//,/ }; do
    args+=("$tok")
  done
}

args=("$command")

# --path applies to run, check, and list.
[ -n "${INPUT_PATH:-}" ] && args+=("--path" "${INPUT_PATH}")

# The remaining flags belong to run/check only (list takes just --path).
if [ "$command" = "run" ] || [ "$command" = "check" ]; then
  is_true "${INPUT_DRY_RUN:-}"  && args+=("--dry-run")
  append_list "--freeze" "${INPUT_FREEZE:-}"
  append_list "--only"   "${INPUT_ONLY:-}"
  append_list "--skip"   "${INPUT_SKIP:-}"
  is_true "${INPUT_MAJOR:-}"    && args+=("--major")
  [ -n "${INPUT_LOG_JSON:-}" ]  && args+=("--log-json" "${INPUT_LOG_JSON}")
  is_true "${INPUT_VERBOSE:-}"  && args+=("--verbose")
  is_true "${INPUT_QUIET:-}"    && args+=("--quiet")
  is_true "${INPUT_NO_COLOR:-}" && args+=("--no-color")
fi

echo "+ ${BIN} ${args[*]}"
"$BIN" "${args[@]}"

# Report whether the working tree changed (empty for check/list — they never write).
changed=false
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  changed=true
fi
echo "changed=${changed}"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "changed=${changed}" >> "$GITHUB_OUTPUT"
fi
