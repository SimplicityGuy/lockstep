#!/usr/bin/env bash
# Install the clockpin release binary for the current runner and put it on PATH.
#
# Driven entirely by environment variables so it can be exercised outside the
# GitHub Actions runtime:
#   INPUT_VERSION   clockpin version to install ("latest" or "vYYYY.MM.MICRO")
#   INPUT_TOKEN     GitHub token, used for API calls and downloads
#   RUNNER_OS       Linux | macOS | Windows   (provided by Actions)
#   RUNNER_ARCH     X64 | ARM64               (provided by Actions)
#   CLOCKPIN_REPO   override the source repo (default SimplicityGuy/clockpin)
#   CLOCKPIN_BIN_DIR override the install directory (default a temp dir)
#
# Emits the resolved version as the `clockpin-version` step output and appends
# the install directory to $GITHUB_PATH.
set -euo pipefail

REPO="${CLOCKPIN_REPO:-SimplicityGuy/clockpin}"
VERSION="${INPUT_VERSION:-latest}"
TOKEN="${INPUT_TOKEN:-}"

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
  local url="$1"
  local auth=()
  [ -n "$TOKEN" ] && auth=(-H "Authorization: Bearer ${TOKEN}")
  curl -sSfL "${auth[@]}" -H "Accept: application/vnd.github+json" "$url"
}

if [ "$VERSION" = "latest" ] || [ -z "$VERSION" ]; then
  echo "Resolving latest clockpin release for ${REPO}…"
  tag="$(api_get "https://api.github.com/repos/${REPO}/releases/latest" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n1)"
  [ -n "$tag" ] || die "could not resolve the latest release for ${REPO} (no releases published yet?)"
else
  # Accept either "vX.Y.Z" or "X.Y.Z".
  case "$VERSION" in
    v*) tag="$VERSION" ;;
    *)  tag="v${VERSION}" ;;
  esac
fi

num="${tag#v}"
archive="clockpin-${num}-${triple}.${ext}"
base_url="https://github.com/${REPO}/releases/download/${tag}"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

echo "Downloading ${archive} (${tag})…"
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
