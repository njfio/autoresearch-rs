#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 <owner/repo>" >&2
  exit 1
fi
repo="$1"

owner="${repo%/*}"
name="${repo#*/}"

cat > /tmp/ruleset.json <<'JSON'
{
  "name": "default-main-ruleset",
  "target": "branch",
  "enforcement": "active",
  "conditions": {"ref_name": {"include": ["~DEFAULT_BRANCH"], "exclude": []}},
  "rules": [
    {"type": "pull_request", "parameters": {"required_approving_review_count": 1, "dismiss_stale_reviews_on_push": true, "require_code_owner_review": true, "require_last_push_approval": false, "required_review_thread_resolution": true}},
    {"type": "deletion"},
    {"type": "non_fast_forward"}
  ]
}
JSON

sed -i 's/~DEFAULT_BRANCH/refs\/heads\/main/' /tmp/ruleset.json

gh api -X POST "repos/${owner}/${name}/rulesets" --input /tmp/ruleset.json >/dev/null || {
  echo "Ruleset create failed (may already exist or require extra permissions)." >&2
  exit 1
}

echo "Ruleset applied to ${repo}"
