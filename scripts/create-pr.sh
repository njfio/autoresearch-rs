#!/usr/bin/env bash
set -euo pipefail

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required" >&2
  exit 1
fi

if [ $# -lt 2 ]; then
  cat >&2 <<USAGE
Usage:
  scripts/create-pr.sh <title> <body-file> [base] [head] [repo]

Example:
  scripts/create-pr.sh \
    "docs: update setup guide" \
    .github/pr-bodies/default.md \
    main \
    my-branch \
    njfio/github-native-delivery-template
USAGE
  exit 1
fi

title="$1"
body_file="$2"
base="${3:-main}"
head="${4:-$(git branch --show-current)}"
repo="${5:-}"

if [ ! -f "$body_file" ]; then
  echo "Body file not found: $body_file" >&2
  exit 1
fi

cmd=(gh pr create --title "$title" --body-file "$body_file" --base "$base" --head "$head")
if [ -n "$repo" ]; then
  cmd+=(--repo "$repo")
fi

"${cmd[@]}"
