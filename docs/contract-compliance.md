# Contract Compliance Matrix

## Contract + templates
- [x] `AGENTS.md`
- [x] Nested path policies: `docs/AGENTS.md`, `scripts/AGENTS.md`, `.github/workflows/AGENTS.md`
- [x] Context-protection skill: `skills/context-protection/SKILL.md`
- [x] Issue templates include required artifacts and stricter fields
- [x] PR templates include reuse/abstraction/error/fallback/dead-code checks + context classification

## Workflows
- [x] `ci.yml`
- [x] `validate.yml`
- [x] `policy.yml` (strict/permissive via `STRICT_POLICY` variable)
- [x] `project-sync.yml` (ProjectV2 wiring via repository variables)
- [x] `release.yml`
- [x] `deploy-staging.yml`
- [x] `deploy-production.yml`
- [x] `greptile-wait-gate.yml`
- [x] `dependency-review.yml`
- [x] `codeql.yml`
- [x] reusable workflow layer (`.github/workflows/reusable/*`)

## Security + release quality
- [x] Dependabot config
- [x] CodeQL config
- [x] Release categories config (`.github/release.yml`)
- [x] `CHANGELOG.md` scaffold

## Ops scripts
- [x] bootstrap script
- [x] branch protection mode presets (solo/team)
- [x] ruleset helper
- [x] PR creation helper (`--body-file`)
