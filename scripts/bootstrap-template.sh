#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <owner/repo> <solo|team>" >&2
  exit 1
fi
repo="$1"
mode="$2"

# 1) Label taxonomy
labels=(
"kind:requirement|0E8A16"
"kind:spec|1D76DB"
"kind:epic|5319E7"
"kind:story|A371F7"
"kind:task|C5DEF5"
"kind:bug|D73A4A"
"kind:spike|FBCA04"
"kind:chore|BFDADC"
"kind:security|B60205"
"kind:incident|E99695"
"priority:p0|B60205"
"priority:p1|D93F0B"
"priority:p2|FBCA04"
"priority:p3|0E8A16"
"risk:low|0E8A16"
"risk:medium|FBCA04"
"risk:high|B60205"
"blocked|000000"
"hotfix|D93F0B"
"follow-up|5319E7"
)
for kv in "${labels[@]}"; do
  n="${kv%%|*}"; c="${kv##*|}"
  if gh api "repos/$repo/labels/$n" >/dev/null 2>&1; then
    gh api -X PATCH "repos/$repo/labels/$n" -f color="$c" >/dev/null
  else
    gh api -X POST "repos/$repo/labels" -f name="$n" -f color="$c" >/dev/null
  fi
done

# 2) Enable auto merge and template-safe vars
gh api -X PATCH "repos/$repo" -f allow_auto_merge=true >/dev/null

gh variable set STRICT_POLICY --repo "$repo" --body "false" >/dev/null || true

# 3) Branch protection preset
scripts/set-branch-protection.sh "$repo" "$mode"

echo "Bootstrap complete for $repo ($mode mode)."
