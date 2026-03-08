#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <owner/repo> <solo|team>" >&2
  exit 1
fi

repo="$1"
mode="$2"

if [ "$mode" = "solo" ]; then
  last_push=false
else
  last_push=true
fi

cat > /tmp/protection.json <<JSON
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "CI / ci",
      "Validate / validate",
      "Policy / policy",
      "Dependency Review / dependency-review",
      "greptile-wait-gate / wait-for-greptile-window"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": true,
    "required_approving_review_count": 1,
    "require_last_push_approval": ${last_push}
  },
  "restrictions": null,
  "required_linear_history": false,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": true
}
JSON

gh api -X PUT "repos/${repo}/branches/main/protection" --input /tmp/protection.json >/dev/null
echo "Applied ${mode} protection mode to ${repo}"
