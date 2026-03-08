# .github/workflows/AGENTS.md

Scope: `.github/workflows/**`

- Prefer deterministic checks with clear failure messages.
- Keep required check names stable.
- Policy checks should support progressive rollout (`STRICT_POLICY`).
- Do not leak secrets in logs.
- Security checks should fail closed in strict mode.
