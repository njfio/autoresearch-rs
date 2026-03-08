# GitHub-Native Full-Lifecycle Delivery Template

Template repo for running software delivery in GitHub with strict traceability and merge safety.

## Included
- `AGENTS.md` authoritative contract
- Issue forms: Requirement, Spec, Epic, Story, Task, Bug, Spike, Chore, Follow-up, Incident
- Discussion templates: RFC + Discovery
- PR templates with traceability, validation, fallback and Greptile disposition sections
- Reusable workflows + entry workflows for CI, Policy, Validate, Project Sync, Release, Deploy, CodeQL
- Security baselines: Dependabot + CodeQL
- `CODEOWNERS`, `greptile.json`, release categorization, changelog scaffold
- Bootstrap scripts for branch protection/rulesets/setup

## AI agent instructions
1. Read `AGENTS.md` first.
2. Work from issue/spec artifacts; no untracked scope.
3. Reuse existing code paths before creating new abstractions.
4. Fail explicitly; no fallback unless human-authorized in GitHub artifacts.
5. Provide validation evidence and resolve all review + Greptile comments.

## Setup a new repo from this template
1. Use template to create repo.
2. Replace any placeholders in `AGENTS.md` if present for your fork.
3. Run bootstrap:
   ```bash
   scripts/bootstrap-template.sh owner/repo solo
   ```
   (`solo` for single-maintainer, `team` for multi-reviewer.)
4. Wire project variables for project sync if using ProjectV2:
   - `PROJECT_ID`
   - `PROJECT_STATUS_FIELD_ID`
   - `PROJECT_STATUS_INTAKE_OPTION_ID`
   - `PROJECT_STATUS_IN_REVIEW_OPTION_ID`
   - `PROJECT_STATUS_DONE_OPTION_ID`
5. Set `STRICT_POLICY=true` when ready for strict policy enforcement.
6. Replace placeholder build/test/deploy commands in workflows.

## Required check names baseline
- `CI / ci`
- `Validate / validate`
- `Policy / policy`
- `Dependency Review / dependency-review`
- `greptile-wait-gate / wait-for-greptile-window`

## Optional scripts
- `scripts/create-pr.sh` — create PR with `--body-file`
- `scripts/set-branch-protection.sh owner/repo solo|team`
- `scripts/apply-ruleset.sh owner/repo`
- `scripts/bootstrap-template.sh owner/repo solo|team`

## Progressive disclosure and context protection
- Root `AGENTS.md` defines hard global policy.
- Nested `AGENTS.md` files add path-scoped rules in `docs/`, `scripts/`, and `.github/workflows/`.
- Use `skills/context-protection/SKILL.md` for handling sensitive/internal context.
- PRs include a context classification section (`public|internal|sensitive`) and redaction note.
