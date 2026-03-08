# Skill: Context Protection & Progressive Disclosure

Use this skill whenever work touches sensitive context, logs, credentials, production incidents, private customer data, or cross-boundary messaging.

## Goals
- Minimize context exposure while preserving traceability.
- Keep sensitive content out of public artifacts by default.
- Require explicit classification and redaction decisions.

## Context classes
- `public`: safe to quote directly in issues/PRs/docs.
- `internal`: may be summarized; avoid raw dumps unless necessary.
- `sensitive`: never paste raw secrets/PII/private data into PRs/issues/logs.

## Required behaviors
1. Classify context first (`public|internal|sensitive`).
2. Prefer summary over verbatim content.
3. Redact tokens/secrets/identifiers before writing artifacts.
4. Store sensitive operational details in approved secret systems, not Git.
5. In PRs, include classification + redaction note.

## Redaction minimum
- API keys/tokens/passwords/private keys
- session cookies/auth headers
- personal identifiers unless explicitly required
- private customer payloads/log lines

## Escalation
If uncertain whether context is safe to disclose:
- treat as `sensitive`
- summarize minimally
- request explicit human approval before wider disclosure.
