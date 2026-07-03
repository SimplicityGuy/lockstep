#!/usr/bin/env bash
# Commit clockpin's changes to a branch and open (or update) a pull request.
#
# No-ops unless: command is `run`, create-pr is truthy, and the working tree
# changed. Safe to call unconditionally from the action.
#
# Environment:
#   INPUT_COMMAND            run | check | list
#   INPUT_CREATE_PR          truthy to enable PR creation
#   CHANGED                  "true" if run.sh detected changes
#   INPUT_TOKEN              GitHub token (git push + gh)
#   INPUT_BRANCH             head branch                 (default: clockpin/updates)
#   INPUT_BASE               base branch                 (default: GITHUB_REF_NAME)
#   INPUT_COMMIT_MESSAGE     commit message
#   INPUT_PR_TITLE           PR title                    (default: commit message)
#   INPUT_PR_BODY            PR body                     (default: generated)
#   INPUT_PR_LABELS          comma/space label list
#   INPUT_PR_DRAFT           truthy to open as draft
#   INPUT_COMMIT_USER_NAME   git author/committer name
#   INPUT_COMMIT_USER_EMAIL  git author/committer email
#   INPUT_LOG_JSON           if set and present, embedded in the PR body
#   GITHUB_REPOSITORY, GITHUB_SERVER_URL, GITHUB_REF_NAME, GITHUB_OUTPUT
#
# Sets `pull-request-number` and `pull-request-url` step outputs.
set -euo pipefail

is_true() {
  case "$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]')" in
    true|1|yes|on) return 0 ;;
    *) return 1 ;;
  esac
}

command="${INPUT_COMMAND:-run}"
if [ "$command" != "run" ]; then
  echo "command is '${command}' (read-only); skipping PR."
  exit 0
fi
if ! is_true "${INPUT_CREATE_PR:-}"; then
  echo "create-pr is disabled; skipping PR."
  exit 0
fi
if ! is_true "${CHANGED:-}"; then
  echo "No changes detected; skipping PR."
  exit 0
fi

TOKEN="${INPUT_TOKEN:-}"
[ -n "$TOKEN" ] || { echo "::error::token is required to create a pull request" >&2; exit 1; }

branch="${INPUT_BRANCH:-clockpin/updates}"
base="${INPUT_BASE:-${GITHUB_REF_NAME:-}}"
if [ -z "$base" ]; then
  base="$(git rev-parse --abbrev-ref HEAD)"
fi
commit_message="${INPUT_COMMIT_MESSAGE:-chore(deps): update pinned dependencies via clockpin}"
pr_title="${INPUT_PR_TITLE:-$commit_message}"
user_name="${INPUT_COMMIT_USER_NAME:-github-actions[bot]}"
user_email="${INPUT_COMMIT_USER_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"

# --- Build the PR body (explicit body wins; otherwise generate one) ---
if [ -n "${INPUT_PR_BODY:-}" ]; then
  body="${INPUT_PR_BODY}"
else
  body="Automated dependency updates produced by [clockpin](https://github.com/SimplicityGuy/clockpin).

Run \`git diff\` on this branch to see exactly what changed. Some ecosystems delegate to their own tooling and don't enumerate individual package changes."
fi
if [ -n "${INPUT_LOG_JSON:-}" ] && [ -f "${INPUT_LOG_JSON}" ]; then
  body="${body}

<details><summary>clockpin run log</summary>

\`\`\`json
$(cat "${INPUT_LOG_JSON}")
\`\`\`

</details>"
fi

# --- Commit on the head branch (keeping the working-tree changes) ---
git config --global --add safe.directory "$PWD" 2>/dev/null || true
git switch -C "$branch"
git add -A
git -c user.name="$user_name" -c user.email="$user_email" \
    commit -m "$commit_message"

# --- Push with an explicitly authenticated URL (independent of checkout creds) ---
server="${GITHUB_SERVER_URL:-https://github.com}"
host="${server#https://}"; host="${host#http://}"
push_url="https://x-access-token:${TOKEN}@${host}/${GITHUB_REPOSITORY}.git"
echo "Pushing ${branch}…"
git push --force "$push_url" "HEAD:refs/heads/${branch}"

# --- Open or reuse the PR ---
export GH_TOKEN="$TOKEN"
number="$(gh pr list --head "$branch" --state open --json number --jq '.[0].number // empty' 2>/dev/null || true)"

if [ -n "$number" ]; then
  echo "Reusing existing PR #${number} (branch force-updated)."
  gh pr edit "$number" --title "$pr_title" --body "$body" >/dev/null || true
else
  create_args=(--base "$base" --head "$branch" --title "$pr_title" --body "$body")
  is_true "${INPUT_PR_DRAFT:-}" && create_args+=(--draft)
  gh pr create "${create_args[@]}"
  number="$(gh pr list --head "$branch" --state open --json number --jq '.[0].number // empty')"
fi

# --- Labels (tolerant: a missing label warns rather than fails the run) ---
if [ -n "${INPUT_PR_LABELS:-}" ] && [ -n "$number" ]; then
  for label in ${INPUT_PR_LABELS//,/ }; do
    gh pr edit "$number" --add-label "$label" >/dev/null 2>&1 \
      || echo "::warning::could not add label '${label}' (does it exist in the repo?)"
  done
fi

url="$(gh pr view "$number" --json url --jq '.url' 2>/dev/null || echo '')"
echo "Pull request: #${number} ${url}"
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  {
    echo "pull-request-number=${number}"
    echo "pull-request-url=${url}"
  } >> "$GITHUB_OUTPUT"
fi
