#!/usr/bin/env bash
# Verifies scripts/install.sh version resolution — in particular that an empty
# version tracks the action's CalVer tag (github.action_ref) and only falls back
# to the releases API for non-CalVer refs.
#
# Fully offline: `curl` is stubbed to serve a locally-built release archive,
# SHA256SUMS, and a canned "latest" API response, and to log requested URLs.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "${here}/../.." && pwd)"
install_sh="${repo_root}/scripts/install.sh"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

triple="x86_64-unknown-linux-gnu"
ver="2026.7.0"
tag="v${ver}"

# --- Build a fake release archive + SHA256SUMS ---
rel="${tmp}/release"
mkdir -p "${rel}"
stage="clockpin-${ver}-${triple}"
mkdir -p "${tmp}/build/${stage}"
cat > "${tmp}/build/${stage}/clockpin" <<STUB
#!/usr/bin/env bash
echo "clockpin ${ver}"
STUB
chmod +x "${tmp}/build/${stage}/clockpin"
archive="clockpin-${ver}-${triple}.tar.gz"
tar -C "${tmp}/build" -czf "${rel}/${archive}" "${stage}"
if command -v sha256sum >/dev/null 2>&1; then
  ( cd "${rel}" && sha256sum "${archive}" > SHA256SUMS )
else
  ( cd "${rel}" && shasum -a 256 "${archive}" > SHA256SUMS )
fi

# --- Stub curl on PATH ---
bindir="${tmp}/bin"
mkdir -p "$bindir"
cat > "${bindir}/curl" <<STUB
#!/usr/bin/env bash
out=""; url=""; prev=""
for a in "\$@"; do
  [ "\$prev" = "-o" ] && out="\$a"
  case "\$a" in http*) url="\$a" ;; esac
  prev="\$a"
done
echo "\$url" >> "${tmp}/curl.log"
case "\$url" in
  */releases/latest) printf '{"tag_name": "%s"}\n' "${STUB_LATEST_TAG:-${tag}}" ;;
  *SHA256SUMS)       cp "${rel}/SHA256SUMS" "\$out" ;;
  *.tar.gz)          cp "${rel}/${archive}" "\$out" ;;
  *) echo "unexpected url: \$url" >&2; exit 22 ;;
esac
STUB
chmod +x "${bindir}/curl"

fail=0
run_case() {
  local name="$1" expect_api="$2"; shift 2
  : > "${tmp}/curl.log"
  local out; out="${tmp}/out.$$"; : > "$out"
  env PATH="${bindir}:${PATH}" \
      CLOCKPIN_REPO="acme/clockpin" RUNNER_OS=Linux RUNNER_ARCH=X64 \
      CLOCKPIN_BIN_DIR="${tmp}/install" GITHUB_OUTPUT="$out" GITHUB_PATH="" \
      STUB_LATEST_TAG="${tag}" \
      "$@" bash "$install_sh" >/dev/null 2>&1

  local got_ver; got_ver="$(sed -n 's/^clockpin-version=//p' "$out")"
  local api=no; grep -q 'releases/latest' "${tmp}/curl.log" && api=yes
  local ok=1
  [ "$got_ver" = "$ver" ] || ok=0
  [ "$api" = "$expect_api" ] || ok=0
  if [ "$ok" = 1 ]; then
    echo "ok   - ${name} (version=${got_ver}, api=${api})"
  else
    echo "FAIL - ${name}: version=${got_ver} (want ${ver}), api=${api} (want ${expect_api})"
    fail=1
  fi
}

# Empty version + CalVer action ref → install the matching tag, no API call.
run_case "matching action ref" no   env INPUT_VERSION= GITHUB_ACTION_REF="${tag}"
# Empty version + bare numeric CalVer ref (no leading v) → still matches.
run_case "matching bare calver"  no   env INPUT_VERSION= GITHUB_ACTION_REF="${ver}"
# Empty version + non-CalVer ref (branch) → fall back to latest via API.
run_case "branch ref falls back" yes  env INPUT_VERSION= GITHUB_ACTION_REF="main"
# Explicit pin → no API call.
run_case "explicit pin"          no   env INPUT_VERSION="${tag}" GITHUB_ACTION_REF="main"
# Explicit latest → API call.
run_case "explicit latest"       yes  env INPUT_VERSION=latest GITHUB_ACTION_REF="${tag}"

exit $fail
