# SETUP Playbook

## Fast path
```bash
scripts/bootstrap-template.sh owner/repo solo
```

## Strict mode rollout
- Start with `STRICT_POLICY=false`
- After first successful baseline release, set `STRICT_POLICY=true`

## Solo vs Team policy
- `solo`: disables last-push-approval to avoid self-deadlock
- `team`: enables last-push-approval for stronger review separation

## Ruleset migration
If your org prefers rulesets:
```bash
scripts/apply-ruleset.sh owner/repo
```
Then validate required checks and review settings in UI.

## Project sync wiring
Set repository variables:
- `PROJECT_ID`
- `PROJECT_STATUS_FIELD_ID`
- `PROJECT_STATUS_INTAKE_OPTION_ID`
- `PROJECT_STATUS_IN_REVIEW_OPTION_ID`
- `PROJECT_STATUS_DONE_OPTION_ID`

Without these, project-sync will no-op safely.

## Context protection rollout
- Keep `STRICT_POLICY=false` during initial setup.
- Train maintainers on `skills/context-protection/SKILL.md`.
- Require PR context classification.
- Flip `STRICT_POLICY=true` when team is ready to enforce hard-fail policy gates.
