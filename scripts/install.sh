#!/usr/bin/env bash
# Install the clockpin release binary for the current runner and put it on PATH.
#
# Driven entirely by environment variables so it can be exercised outside the
# GitHub Actions runtime:
#   INPUT_VERSION   clockpin version to install. Empty/"matching" (default) tracks
#                   the action's own CalVer tag; "latest" or "YYYY.MM.MICRO" pin.
#   INPUT_TOKEN     GitHub token, used for API calls and downloads
#   GITHUB_ACTION_REF  the ref the action was resolved at (provided by Actions)
#   RUNNER_OS       Linux | macOS | Windows   (provided by Actions)
#   RUNNER_ARCH     X64 | ARM64               (provided by Actions)
#   CLOCKPIN_REPO   override the source repo (default SimplicityGuy/clockpin)
#   CLOCKPIN_BIN_DIR override the install directory (default a temp dir)
#
# Emits the resolved version as the `clockpin-version` step output and appends
# the install directory to $GITHUB_PATH.
set -euo pipefail

REPO="${CLOCKPIN_REPO:-SimplicityGuy/clockpin}"
VERSION="${INPUT_VERSION:-}"
TOKEN="${INPUT_TOKEN:-}"

# clockpin releases use CalVer: YYYY.MM.MICRO, tagged vYYYY.MM.MICRO.
CALVER_RE='^v?[0-9]{4}\.[0-9]{1,2}\.[0-9]+$'

die() {
  echo "::error::$*" >&2
  exit 1
}

# --- Map RUNNER_OS/RUNNER_ARCH to a Rust target triple + archive extension ---
os="${RUNNER_OS:-}"
arch="${RUNNER_ARCH:-}"
ext="tar.gz"
case "${os}:${arch}" in
  Linux:X64)     triple="x86_64-unknown-linux-gnu" ;;
  Linux:ARM64)   triple="aarch64-unknown-linux-gnu" ;;
  macOS:X64)     triple="x86_64-apple-darwin" ;;
  macOS:ARM64)   triple="aarch64-apple-darwin" ;;
  Windows:X64)   triple="x86_64-pc-windows-msvc"; ext="zip" ;;
  *) die "unsupported runner platform: OS='${os}' ARCH='${arch}'" ;;
esac

# --- Resolve `latest` to a concrete tag via the releases API ---
api_get() {
  # Two explicit forms rather than an array — expanding an empty array under
  # `set -u` errors on bash 3.2 (the default on macOS runners).
  local url="$1"
  if [ -n "$TOKEN" ]; then
    curl -sSfL -H "Authorization: Bearer ${TOKEN}" -H "Accept: application/vnd.github+json" "$url"
  else
    curl -sSfL -H "Accept: application/vnd.github+json" "$url"
  fi
}

resolve_latest() {
  echo "Resolving latest clockpin release for ${REPO}…" >&2
  local t
  t="$(api_get "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n1)"
  [ -n "$t" ] || die "could not resolve the latest release for ${REPO} (no releases published yet?)"
  printf '%s' "$t"
}

if [ -z "$VERSION" ] || [ "$VERSION" = "matching" ]; then
  # Default: keep the binary in lockstep with the action's own CalVer tag.
  # github.action_ref is the tag/branch/SHA the caller pinned the action to.
  ref="${GITHUB_ACTION_REF:-}"
  if printf '%s' "$ref" | grep -Eq "$CALVER_RE"; then
    num="${ref#v}"
    echo "Action pinned to ${ref}; installing the matching clockpin ${num}."
  else
    echo "Action ref '${ref:-<none>}' is not a CalVer tag; falling back to the latest release."
    num="$(resolve_latest)"
  fi
elif [ "$VERSION" = "latest" ]; then
  num="$(resolve_latest)"
else
  num="$VERSION"
fi

# clockpin tags are bare CalVer (no leading "v"); tolerate a "v" a caller passes.
num="${num#v}"
archive="clockpin-${num}-${triple}.${ext}"
base_url="https://github.com/${REPO}/releases/download/${num}"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

echo "Downloading ${archive} (${num})…"
curl -sSfL -o "${workdir}/${archive}" "${base_url}/${archive}" \
  || die "failed to download ${base_url}/${archive}"
curl -sSfL -o "${workdir}/SHA256SUMS" "${base_url}/SHA256SUMS" \
  || die "failed to download ${base_url}/SHA256SUMS"

# --- Verify the checksum against the release's SHA256SUMS ---
expected="$(awk -v f="$archive" '$2==f {print $1}' "${workdir}/SHA256SUMS")"
[ -n "$expected" ] || die "no checksum for ${archive} in SHA256SUMS"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "${workdir}/${archive}" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "${workdir}/${archive}" | awk '{print $1}')"
fi
[ "$expected" = "$actual" ] || die "checksum mismatch for ${archive}: expected ${expected}, got ${actual}"
echo "Checksum OK."

# --- Extract ---
stage="clockpin-${num}-${triple}"
if [ "$ext" = "zip" ]; then
  if command -v unzip >/dev/null 2>&1; then
    unzip -q "${workdir}/${archive}" -d "${workdir}"
  elif command -v 7z >/dev/null 2>&1; then
    7z x -o"${workdir}" "${workdir}/${archive}" >/dev/null
  else
    powershell -NoProfile -Command "Expand-Archive -Force '${workdir}/${archive}' '${workdir}'"
  fi
  bin_name="clockpin.exe"
else
  tar -xzf "${workdir}/${archive}" -C "${workdir}"
  bin_name="clockpin"
fi

# --- Place on PATH ---
bin_dir="${CLOCKPIN_BIN_DIR:-$(mktemp -d)}"
mkdir -p "$bin_dir"
cp "${workdir}/${stage}/${bin_name}" "${bin_dir}/${bin_name}"
chmod +x "${bin_dir}/${bin_name}" 2>/dev/null || true

if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$bin_dir" >> "$GITHUB_PATH"
fi
export PATH="${bin_dir}:${PATH}"

resolved="$("${bin_dir}/${bin_name}" --version 2>/dev/null || echo "clockpin ${num}")"
echo "Installed: ${resolved} → ${bin_dir}/${bin_name}"

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "clockpin-version=${num}" >> "$GITHUB_OUTPUT"
fi
