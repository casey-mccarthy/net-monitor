#!/usr/bin/env bash
#
# Render the GitHub release notes for a version from the conventional commits
# since the previous tag. The release workflow runs this; you can run it
# locally to preview what a release will say:
#
#   scripts/release-notes.sh 1.4.10            # notes for tag v1.4.10
#   scripts/release-notes.sh 1.5.0 v1.4.10     # HEAD as 1.5.0, changes since v1.4.10
#
# Usage: release-notes.sh <version> [<previous-tag>]
#
#   version        the version being released, without the leading "v"
#   previous-tag   where the changelog starts; defaults to the newest tag
#                  reachable from the release commit, other than v<version>
#
# The release commit is the tag v<version> when it exists, otherwise HEAD.
# Writes markdown to stdout.

set -euo pipefail

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
  echo "usage: $0 <version> [<previous-tag>]" >&2
  exit 64
fi

VERSION="$1"
TAG="v${VERSION}"
REPO_URL="https://github.com/casey-mccarthy/net-monitor"

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  TO_REF="$TAG"
else
  TO_REF="HEAD"
fi

if [ $# -eq 2 ]; then
  PREVIOUS_TAG="$2"
else
  PREVIOUS_TAG=$(git describe --tags --abbrev=0 --exclude "$TAG" "$TO_REF" 2>/dev/null || true)
fi

if [ -n "$PREVIOUS_TAG" ]; then
  RANGE="${PREVIOUS_TAG}..${TO_REF}"
else
  RANGE="$TO_REF"
fi

BREAKING=()
FEATURES=()
FIXES=()
PERFORMANCE=()
DOCS=()
OTHER=()

# One commit per iteration, newest first. Reading the subject and body with
# separate `git log -1` calls sidesteps every multi-line quoting problem.
while IFS= read -r hash; do
  subject=$(git log -1 --format=%s "$hash")
  body=$(git log -1 --format=%b "$hash")

  # The workflow's own version-bump commit is release plumbing, not a change.
  case "$subject" in
    "chore: bump version to "*) continue ;;
  esac

  # type(scope)!: description
  conventional='^([a-z]+)(\(([^)]+)\))?(!)?: (.+)$'
  if [[ "$subject" =~ $conventional ]]; then
    type="${BASH_REMATCH[1]}"
    scope="${BASH_REMATCH[3]}"
    bang="${BASH_REMATCH[4]}"
    description="${BASH_REMATCH[5]}"
  else
    type=""
    scope=""
    bang=""
    description="$subject"
  fi

  if [ -n "$scope" ]; then
    entry="- **${scope}:** ${description}"
  else
    entry="- ${description}"
  fi

  if [ -n "$bang" ] || [[ "$body" == *"BREAKING CHANGE"* ]] || [[ "$body" == *"BREAKING-CHANGE"* ]]; then
    BREAKING+=("$entry")
    continue
  fi

  case "$type" in
    feat) FEATURES+=("$entry") ;;
    fix) FIXES+=("$entry") ;;
    perf) PERFORMANCE+=("$entry") ;;
    docs) DOCS+=("$entry") ;;
    *) OTHER+=("$entry") ;;
  esac
done < <(git rev-list --no-merges "$RANGE")

section() {
  local title="$1"
  shift
  [ $# -eq 0 ] && return
  printf '## %s\n\n' "$title"
  printf '%s\n' "$@"
  printf '\n'
}

printf '# Release %s\n\n' "$TAG"
printf '**Release Date:** %s\n\n' "$(date -u +%Y-%m-%d)"

TOTAL=$(( ${#BREAKING[@]} + ${#FEATURES[@]} + ${#FIXES[@]} + ${#PERFORMANCE[@]} + ${#DOCS[@]} + ${#OTHER[@]} ))
if [ "$TOTAL" -eq 0 ]; then
  printf 'No changes since %s.\n\n' "${PREVIOUS_TAG:-the first commit}"
fi

section "⚠️ Breaking Changes" "${BREAKING[@]}"
section "✨ Features" "${FEATURES[@]}"
section "🐛 Bug Fixes" "${FIXES[@]}"
section "⚡ Performance" "${PERFORMANCE[@]}"
section "📚 Documentation" "${DOCS[@]}"
section "🔧 Other Changes" "${OTHER[@]}"

cat <<INSTALL
## 📦 Installation

Each archive contains the \`net-monitor\` binary, a terminal user interface (TUI).

| Platform | Archive |
|---|---|
| Windows x64 | \`net-monitor-${TAG}-windows-x64.zip\` |
| macOS Intel | \`net-monitor-${TAG}-macos-x64.tar.gz\` |
| macOS Apple Silicon | \`net-monitor-${TAG}-macos-arm64.tar.gz\` |
| Linux x64 | \`net-monitor-${TAG}-linux-x64.tar.gz\` |

Extract the archive and run \`./net-monitor\`. SHA256 checksums are in \`checksums.txt\`.
INSTALL

if [ -n "$PREVIOUS_TAG" ]; then
  printf '\n---\n\n**Full Changelog**: %s/compare/%s...%s\n' "$REPO_URL" "$PREVIOUS_TAG" "$TAG"
fi
