# agents.md — GitHub-Native Software Delivery Contract

This file is the operating contract for all human and AI agents working in this repository.

Its purpose is to make planning, design, implementation, testing, review, release, and post-release learning fully traceable inside GitHub, with no dependency on external project-management systems.

The contract uses the words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** in their normal normative sense.

---

## 1. Contract goals

All work in this repository MUST satisfy these goals:

1. **Single source of truth**: durable requirements, design decisions, plans, tasks, review decisions, validation evidence, and release history live in GitHub.
2. **Traceability**: every shipped change can be traced from requirement to spec to epic to story to task to PR to release.
3. **Small, reviewable increments**: work is decomposed into the smallest independently testable and mergeable slices.
4. **Built-in quality**: tests, validation, and rollout evidence are part of delivery, not follow-up work.
5. **Safe integration**: merging is gated by review, automated validation, and deployment readiness.
6. **Explicit closure**: nothing is “done” until review comments, validation, and release obligations are explicitly closed.
7. **Minimal, elegant implementation**: code is kept as small, direct, and comprehensible as possible.
8. **Reuse before invention**: existing repository patterns, modules, workflows, and contracts are reused or extended before new ones are introduced.
9. **Explicit failure behavior**: errors fail hard and are surfaced clearly; silent degradation is not acceptable by default.
10. **Purposeful code only**: no speculative scaffolding, no dead additions, and no disconnected implementation islands.

---

## 2. Core engineering principles

These principles apply across the full lifecycle and are merge-blocking expectations, not style suggestions.

### 2.1 Minimality and elegance

All code MUST be maximally minimal for the approved scope.

This means agents MUST:

- prefer deleting code over adding code when deletion solves the problem
- prefer extending an existing well-fitting path over introducing a parallel one
- choose the smallest change that fully satisfies the requirement
- avoid unnecessary wrappers, indirection, abstractions, layers, and configuration
- make control flow, ownership, and data flow obvious to the next maintainer

A change is not “better” because it is more clever, more generic, or more configurable. It is better only if it is simpler, clearer, and sufficient.

### 2.2 Reuse before invention

Before adding any new file, module, helper, abstraction, workflow, or interface, the implementing agent MUST inspect the repository for an existing place to extend.

Agents MUST:

- reuse established repository patterns unless there is a documented reason not to
- consolidate duplicate logic instead of adding a second implementation
- prefer existing shared utilities when they are a natural fit
- justify every new abstraction in the PR when an existing one could have been extended

New abstractions MUST NOT be added for hypothetical future use alone.

### 2.3 Hard-fail error handling

Errors MUST hard fail and be surfaced explicitly.

For exception-based languages, errors MUST be raised with a meaningful type and message. For non-exception-based languages, errors MUST be returned or propagated using the language’s explicit failure mechanism and MUST fail the current operation.

Agents MUST:

- surface invalid state, contract violation, dependency failure, and data inconsistency explicitly
- preserve useful context when re-raising or translating errors
- fail at the appropriate boundary instead of silently continuing
- treat logging as supplementary evidence, not as a substitute for failure propagation

Swallowing errors, returning misleading success values, or “best effort” continuation without approval is prohibited.

### 2.4 No fallbacks unless explicitly human-authorized

Fallbacks, failovers, degradations, silent defaults, shadow paths, rescue paths, and “try old way if new way fails” behavior MUST NOT be introduced unless a human explicitly requests them in the Requirement, Spec, or Story.

If a fallback is intentionally authorized, the authorizing artifact MUST define:

- why the fallback is needed
- exactly when it activates
- expected behavior while active
- how it is observed or alerted on
- when it is removed if temporary
- how it is tested

If no explicit human authorization exists, the correct behavior is to fail fast and surface the error.

### 2.5 No stubs, pseudo code, or placeholder production logic

Committed repository code MUST be real, executable, and complete for the scope it claims to implement.

Production code MUST NOT contain:

- pseudo code
- fake implementations presented as real behavior
- placeholder branches that will “be filled in later”
- dead scaffolding for future work
- commented-out code kept “just in case”
- TODO/FIXME/placeholder markers in runtime-critical paths as a substitute for implementation

Language-idiomatic abstract interfaces or test-only doubles MAY exist where appropriate, but runtime behavior in merged code MUST NOT depend on unimplemented placeholders.

### 2.6 No code islands

Every non-test code addition MUST be integrated, reachable, and purposeful in the same PR or already-existing runtime path.

Agents MUST NOT add:

- orphaned utilities with no real caller
- alternate implementations that are never wired in
- unused files, configs, flags, or adapters
- speculative modules added “for later”
- generated or copied code that does not serve a live path

When a new path replaces an old path, the old path SHOULD be removed in the same PR whenever safe. If it cannot be removed immediately, its temporary coexistence MUST be explicitly documented and tracked.

### 2.7 One behavior, one authoritative home

There SHOULD be one authoritative implementation of each behavior.

When the same behavior exists in more than one place, the implementing agent MUST either:

- consolidate it in the current PR, or
- create a linked follow-up issue with explicit risk acceptance if immediate consolidation is unsafe

Duplication is technical debt the moment it is created.

---

## 3. GitHub is the system of record

The following GitHub primitives are the authoritative system of record for this repository:

- **Discussions** for discovery, RFCs, open questions, design debate, and postmortems.
- **Issues** for requirements, specs, epics, stories, tasks, bugs, spikes, and follow-up work.
- **Sub-issues** for decomposition and hierarchy.
- **Issue dependencies** for sequencing and blocked-by/blocking relationships.
- **Projects** for portfolio, roadmap, backlog, iteration, review, release, and status tracking.
- **Milestones** for release containers.
- **Pull requests** for implementation review, design adjustments, and merge decisions.
- **CODEOWNERS and PR reviews** for ownership and approval.
- **Actions** for automation, CI, test execution, policy enforcement, validation, and release workflows.
- **Environments** for deployment approval, stage gating, and deployment history.
- **Releases** for shipped scope and release notes.

No decision that affects scope, architecture, testing, security, migration, rollout, failure behavior, fallback behavior, or release MAY exist only in chat, email, or verbal discussion. It MUST be captured in GitHub.

---

## 4. Required artifact model

Every substantial change MUST fit this artifact chain:

**Requirement → Spec → Epic → Story → Task → PR → Release**

Small changes MAY collapse parts of the chain, but they MUST still have a driving issue and a linked PR.

### 4.1 Requirement

A **Requirement** describes the problem to solve and the outcome to achieve.

A Requirement MUST include:

- problem statement
- user or business outcome
- success metric or acceptance outcome
- constraints and assumptions
- reliability and failure expectations when material
- non-goals
- owning team or owner
- priority

A Requirement MUST NOT prescribe implementation details beyond hard constraints.

### 4.2 Spec

A **Spec** describes how the requirement will be satisfied.

A Spec MUST include:

- linked Requirement
- scope and non-scope
- proposed solution
- existing components, patterns, or workflows to reuse or extend
- explicit justification for any new abstraction, module, or interface
- alternatives considered when material
- architecture and component impact
- data model / schema / migration impact
- API, interface, or contract changes
- security, privacy, and permissions impact
- observability and operational impact
- explicit error behavior and failure semantics
- explicit fallback behavior, but only if human-authorized
- rollout and rollback plan
- cleanup or deletion plan for replaced code paths when relevant
- test strategy
- acceptance criteria

For non-trivial work, the Spec SHOULD also be represented as a repository document under `docs/specs/` and linked from the Spec issue.

### 4.3 Epic

An **Epic** is a delivery container that groups related Stories required to deliver a coherent capability.

An Epic MUST include:

- linked Spec
- scope boundary
- exit criteria
- target milestone or release
- ordered Stories or dependencies

### 4.4 Story

A **Story** is a user-visible or system-visible increment that can be demonstrated and accepted.

A Story MUST include:

- linked Epic
- concrete acceptance criteria
- negative-path or error-path expectations when material
- integration points affected
- dependencies
- validation approach
- definition of done specific to the change

### 4.5 Task

A **Task** is the smallest implementation unit that should normally map to a single PR.

A Task MUST include:

- linked Story
- implementation objective
- files or subsystem area if known
- expected integration points
- reuse or extension target if known
- test notes
- completion criteria

### 4.6 Bug

A **Bug** issue MUST include:

- expected behavior
- actual behavior
- reproduction steps
- severity / impact
- suspected root cause if known
- explicit failure behavior expectation
- regression test expectation

### 4.7 Spike / Investigation

A **Spike** MAY be used for time-boxed research. It MUST end in one of three outcomes:

- a Spec
- a decision to not proceed
- a follow-up issue with narrowed scope

A Spike MUST NOT remain open-ended.

### 4.8 Follow-up issue

A **Follow-up** issue MAY be created when a valid concern is intentionally deferred.

A Follow-up issue MUST include:

- why it was deferred
- why merge is still safe without it
- target owner
- target milestone or iteration when possible
- explicit link back to the originating PR and comment thread

A Follow-up issue MUST NOT be used to justify merging placeholder code, dead code, or an unauthorized fallback.

---

## 5. GitHub-native artifact mapping

This repository SHOULD use the following GitHub-native mapping:

- **Issue types** if available: `Requirement`, `Spec`, `Epic`, `Story`, `Task`, `Bug`, `Spike`, `Follow-up`.
- If issue types are not used, the Project MUST include a single-select custom field named **Artifact Type** with the same values.
- **Sub-issues** express hierarchy.
- **Issue dependencies** express blocked-by / blocking relationships.
- **Milestones** express release grouping.
- **Projects** express workflow status, planning metadata, and views.

Where possible, the Project SHOULD expose these fields:

- `Status`
- `Artifact Type`
- `Priority`
- `Risk`
- `Team`
- `Area`
- `Iteration`
- `Target Date`
- `Start Date`
- `Effort` or `Size`
- `Milestone`
- `Linked PR`
- `Environment`

Prefer **fields** over label sprawl. Labels SHOULD be reserved for lightweight cross-cutting metadata such as `blocked`, `hotfix`, `follow-up`, `docs`, or `security`.

---

## 6. Lifecycle phases

### Phase 0 — Discovery

Use a **Discussion** when work is still exploratory, open-ended, or decision-seeking.

Discovery MUST produce one of:

- a Requirement issue
- a Spec issue
- a decision to not pursue the work

If a Discussion becomes actionable, create the corresponding Issue from it and link both.

### Phase 1 — Requirement definition

No implementation work MAY begin until a Requirement exists, except for trivial repository maintenance.

A Requirement exits this phase only when:

- the problem is clear
- the desired outcome is clear
- the owner is clear
- the success condition is clear
- any material reliability or failure expectation is clear
- the Requirement is placed in the Project

### Phase 2 — Specification

Any work with meaningful product, architectural, integration, migration, security, rollout, fallback, or failure-handling implications MUST have a Spec before coding begins.

A Spec exits this phase only when:

- acceptance criteria are explicit
- risks are documented
- rollout and rollback are documented
- test strategy is documented
- reuse versus new abstraction decisions are documented
- error behavior is documented
- any fallback is explicitly human-authorized and documented, or explicitly absent
- cleanup of replaced paths is documented when relevant
- unresolved questions are closed or explicitly tracked

### Phase 3 — Planning and decomposition

The implementing agent MUST break approved work into:

- one Epic when the change spans multiple Stories, and
- one or more Stories, and
- one or more Tasks per Story

Planning exits this phase only when:

- all near-term Tasks are created
- dependencies are captured
- blocked work is explicitly marked
- expected integration points are known
- the next mergeable slice is unambiguous
- hidden placeholder work is not required to make the slice mergeable

### Phase 4 — Implementation

Implementation MUST be driven by a Task issue.

During implementation, agents MUST:

- create a branch from the driving issue when practical
- open a **Draft PR early** for visibility
- keep the PR scoped to a single mergeable intent
- extend or modify existing code before adding parallel code paths where that is a good fit
- wire new code end-to-end rather than leaving disconnected pieces
- remove obsolete code in the same PR whenever safe
- update the issue and PR descriptions when scope changes
- create additional sub-issues instead of silently expanding scope

Draft status does not permit pseudo code, dead scaffolding, or fake production behavior in committed repository code.

### Phase 5 — Verification

Before a PR is ready for final review, the change MUST have:

- all required automated checks passing locally or in CI
- required tests added or updated
- error-path validation added or updated when behavior changed
- validation evidence attached or linked
- documentation updated when behavior or usage changes
- migrations and rollback steps documented when relevant
- no unauthorized fallback behavior introduced
- no orphaned or unreachable non-test code introduced
- no placeholder production logic left in the merge path

### Phase 6 — Review and comment closure

Review includes:

- human review
- code owner review where applicable
- Greptile review
- automated checks
- deployment-readiness review when applicable
- simplicity, reuse, and failure-handling review

No PR MAY leave this phase while unresolved blocking comments or failed checks remain.

### Phase 7 — Merge

A PR MAY be merged only when:

- linked issue traceability is correct
- required approvals are present and current
- all required checks pass on the latest code
- all required conversations are resolved
- Greptile obligations are closed
- any required deployments have succeeded
- no unauthorized fallback is present
- no placeholder production logic remains
- the change is integrated and purposeful rather than speculative or orphaned

### Phase 8 — Release

A merged change is not considered fully complete until it is released through the repository’s release path.

Release exits this phase only when:

- release notes exist
- deployment evidence exists
- smoke validation is complete
- milestone scope is updated

### Phase 9 — Post-release validation and learning

After release, the responsible agent MUST:

- confirm acceptance criteria in the target environment
- confirm observed error behavior matches the intended contract when material
- close the driving issue only after validation is complete
- create follow-up issues for non-blocking gaps
- create a postmortem Discussion or issue when incidents or significant surprises occurred

If a temporary human-authorized fallback was introduced, its removal MUST remain actively tracked until complete.

---

## 7. Mandatory PR contract

Every PR MUST:

- be linked to a driving issue
- state what it changes and why
- state which existing code, pattern, or workflow it reused or extended
- justify any new file, abstraction, module, or interface
- state what duplicate, obsolete, or replaced code was removed, or why it could not yet be removed
- list test and validation evidence
- list negative-path or failure-path validation when material
- list rollout / rollback notes when relevant
- identify risks and follow-ups
- explicitly declare whether any fallback was introduced and link the authorizing artifact if yes
- use closing keywords for the driving issue when appropriate

A PR MUST NOT be treated as the place where scope is invented. Scope belongs in Issues and Specs.

### 7.1 Draft PR rule

A PR SHOULD stay in **Draft** until:

- the branch has meaningful content for review
- the PR body is filled in
- the driving issue is linked
- the implementation approach still matches the Spec

A Draft PR MAY be incomplete as a feature slice, but committed code in the repository branch MUST still be real code, not pseudo code or fake production behavior.

### 7.2 PR size rule

PRs SHOULD be as small as possible while remaining coherent.

If a PR is too large to review safely, the implementing agent MUST split it into smaller Tasks and PRs unless a strong reason is documented in the PR body.

### 7.3 Scope-change rule

If implementation reveals new scope, one of the following MUST happen before continuing:

- update the Requirement / Spec / Story, or
- create a new linked issue, or
- stop and move the unexpected work to a follow-up issue

Silent scope expansion is prohibited.

### 7.4 New abstraction justification rule

Any new abstraction, helper, adapter, workflow, or service boundary introduced in the PR MUST be justified in the PR body.

The justification MUST explain:

- why existing code was not the right place to extend
- what duplication or complexity the abstraction removes
- how the abstraction is integrated now

A single-use abstraction with no clear boundary is presumed unnecessary unless justified.

---

## 8. Greptile gate — mandatory phase before merge

This repository enforces a dedicated **Awaiting Greptile** phase.

### 8.1 Entry to Awaiting Greptile

A PR enters **Awaiting Greptile** at the later of:

- PR creation, or
- marking the PR Ready for Review

### 8.2 Minimum wait rule

The PR MUST wait at least **3 minutes** in **Awaiting Greptile** before it is eligible for merge.

During this wait window, final approval and merge MUST NOT occur.

### 8.3 If Greptile has not responded

If Greptile has not posted its review or status by the end of the wait window, the implementing agent MUST:

1. comment `@greptileai` on the PR, and
2. wait for Greptile to respond before merge

### 8.4 Material updates after Greptile review

If any **material change** is pushed after Greptile’s latest review, the PR MUST re-enter **Awaiting Greptile**.

A material change includes any change to:

- application logic
- tests
- database or schema changes
- migrations
- public or internal interfaces
- permissions, auth, or security behavior
- CI or deployment logic
- dependency manifests or lockfiles
- generated code that affects runtime behavior
- fallback or failure behavior

A purely mechanical rebase, conflict resolution with no behavior change, or comment-only change MAY be treated as non-material if the PR author states so in the PR.

### 8.5 Acceptable resolution states for Greptile comments

Every Greptile comment MUST end in exactly one acceptable state:

1. **Fixed in this PR**
2. **Not applicable / false positive**, with explicit rationale in the thread
3. **Deferred to a linked Follow-up issue**, with human reviewer agreement that merge is still safe

“Will handle later” without a linked issue is not an acceptable resolution.

### 8.6 Thread resolution rule

Every Greptile thread MUST be replied to and resolved before merge.

No PR may merge while:

- a Greptile thread is unresolved
- a Greptile-required status check is pending or failing
- a material post-review commit lacks a fresh Greptile review

### 8.7 Greptile findings and design quality

Greptile findings about duplication, dead code, unreachable code, missing integration, silent error handling, or unauthorized fallback behavior are blocking unless explicitly resolved under the acceptable resolution states above.

### 8.8 Strongly recommended repository setting

If available for your Greptile configuration, enable automatic re-review on PR updates so every material push triggers a fresh review. If that is not enabled, the PR author MUST trigger re-review manually.

---

## 9. Human review contract

### 9.1 Ownership

Every code path with a designated owner MUST have code owner review when touched.

### 9.2 Approval rules

A PR MUST receive at least one human approval.

High-risk changes SHOULD require at least two approvals, including a domain owner. High-risk changes include:

- authentication or authorization
- payments or billing
- security-sensitive flows
- destructive data changes
- schema migrations
- infrastructure or deployment pipeline changes
- public API or contract changes
- production incident hotfixes

### 9.3 Staleness rule

If new material commits are pushed after approval, approvals MUST be considered stale and the PR MUST be re-reviewed.

### 9.4 No self-approval

The author of a PR MUST NOT be the sole approver for merge.

### 9.5 No side-channel approvals

Approvals given in chat or verbally do not count. Approval MUST exist in the PR review history.

### 9.6 Mandatory reviewer questions

Human reviewers MUST actively check:

- Is this the smallest change that solves the approved problem?
- Did the author reuse or appropriately extend existing code?
- Did the author avoid unnecessary new abstractions or files?
- Are errors surfaced explicitly and correctly?
- Was any fallback introduced without explicit human authorization?
- Does every new non-test code path have a real integration point?
- Is any duplicate, obsolete, or dead code left behind unnecessarily?
- Is there any pseudo code, placeholder logic, or commented-out code that should block merge?

If the answer to any blocking question is “no” or “unclear,” the PR MUST remain in review.

---

## 10. Testing and validation contract

Testing is mandatory. The test burden scales with risk.

### 10.1 Minimum expectations

Every code change MUST include one or more of:

- updated automated tests
- a justified explanation for why automated tests are not applicable
- alternative validation evidence captured in the PR

Manual testing MAY supplement automation, but it MUST NOT replace automatable regression coverage when practical.

### 10.2 Positive-path and error-path coverage

When behavior changes, validation MUST cover both:

- intended successful behavior
- intended failure behavior

If the change introduces or modifies error handling, tests or validation MUST prove that errors surface correctly and do not silently degrade.

### 10.3 Required by change type

- **Bug fix**: add or update a regression test whenever practical.
- **Logic change**: add or update unit tests.
- **Boundary or component interaction change**: add or update integration tests.
- **API / event / schema change**: add or update contract tests and compatibility validation.
- **Migration**: include forward validation, backward/rollback notes, and data-safety checks.
- **UI change**: include screenshots, recordings, or visual proof in addition to tests where useful.
- **Performance-sensitive change**: include benchmark or performance validation evidence.
- **Security-sensitive change**: include explicit permission and abuse-path validation.
- **Fallback added by explicit human authorization**: add tests for activation conditions, active behavior, observability, and safe exit conditions.

### 10.4 CI evidence

All required validation SHOULD run in GitHub Actions and report as required checks.

Build logs, coverage summaries, screenshots, recordings, benchmark outputs, generated bundles, policy reports, or migration logs SHOULD be uploaded as workflow artifacts when they are too large or noisy for the PR body.

### 10.5 Quality guards

Where supported by the language and repository, CI SHOULD include checks for:

- lint and static analysis
- unused code or dead code detection
- unreachable code detection
- duplicate-code detection or maintainability checks
- forbidden placeholder markers in production paths
- dependency and manifest consistency

### 10.6 Release validation

Before closing the driving issue after deployment, the responsible agent MUST confirm:

- deployment succeeded in the intended environment
- smoke checks passed
- acceptance criteria are satisfied in the target environment
- intended error behavior is preserved in the target environment when material
- no emergency rollback is required

---

## 11. Integration and dependency contract

### 11.1 Interface changes

Any change to an external API, internal service boundary, event schema, shared library contract, or critical runtime interface MUST be called out in:

- the Spec
- the Story acceptance criteria
- the PR description

### 11.2 Integration rule

Every new non-test module, function, adapter, workflow, or config MUST identify its live integration point.

If an addition has no live caller, no live activation path, and no validation path, it MUST NOT be merged.

### 11.3 Compatibility

Breaking changes MUST have an explicit compatibility strategy, such as:

- versioning
- phased rollout
- dual-write / dual-read
- feature flags
- migration window
- coordinated release plan

Temporary dual paths MUST have a removal plan and, if not removed in the same PR, a linked follow-up issue.

### 11.4 Dependency changes

Dependency changes MUST include:

- why the dependency changed
- risk assessment when material
- compatibility notes when material
- validation of lockfile or manifest changes

Dependency changes MUST NOT be used as a substitute for integrating with existing repository conventions unless that exception is documented.

When repository features permit it, dependency review SHOULD be a required check.

---

## 12. Branch, merge, and deployment governance

The default branch and release branches MUST be protected by **branch protection rules or rulesets**.

### 12.1 Required merge gates

The protected branch configuration SHOULD require:

- pull request before merge
- required approvals
- stale approval dismissal on new commits
- code owner review when applicable
- approval of the most recent reviewable push
- required status checks
- conversation resolution before merge
- merge queue for busy or critical branches
- deployments to required environments before merge, when used
- no bypass for normal development

### 12.2 Required quality checks

Required status checks SHOULD include, where supported by the repository:

- CI build and test checks
- policy checks for linked issue and PR template completion
- dead-code / unused-code / unreachable-code checks
- checks for forbidden placeholder markers in production code
- dependency or supply-chain review checks

### 12.3 Merge method

This repository SHOULD use one consistent merge method per protected branch. If merge queue is enabled, its configured merge method is authoritative.

### 12.4 Merge queue rule

If merge queue is enabled, required GitHub Actions workflows MUST also listen to the `merge_group` event.

### 12.5 Environment rule

If staging or production environments are configured, deployment jobs for those environments SHOULD require approval and SHOULD record deployment history in GitHub.

---

## 13. Required Project workflow states

The Project SHOULD include, at minimum, these states:

- `Intake`
- `Needs Spec`
- `Ready`
- `In Progress`
- `Blocked`
- `In Review`
- `Awaiting Greptile`
- `Awaiting Validation`
- `Ready to Merge`
- `Merged`
- `Validating`
- `Done`

Suggested transitions:

- Requirement created → `Intake`
- Requirement accepted / Spec approved → `Ready`
- Task actively implemented → `In Progress`
- Draft PR opened → `In Review`
- PR ready or updated materially after review → `Awaiting Greptile`
- Greptile + human review complete, checks green → `Ready to Merge`
- PR merged → `Merged`
- deployed but not yet validated → `Validating`
- acceptance confirmed → `Done`

---

## 14. Required GitHub views

The Project SHOULD expose at least these saved views:

1. **Intake** — open Requirements and Specs not yet ready
2. **Roadmap** — Epics and Stories on a roadmap layout
3. **Current Iteration** — work grouped by Status and filtered to active iteration
4. **Blocked** — all blocked items grouped by blocker or owner
5. **In Review** — PR-linked work actively under review
6. **Awaiting Greptile** — all PRs or tasks waiting on Greptile closure
7. **Release Readiness** — items by milestone with unresolved blockers
8. **Post-release Follow-ups** — all follow-up issues created during review or rollout

---

## 15. Templates required in the repository

The repository SHOULD include GitHub-native templates for:

- `Requirement`
- `Spec`
- `Epic`
- `Story`
- `Task`
- `Bug`
- `Spike`
- `Follow-up`
- `Pull Request`

If GitHub Issue Forms are used, each template MUST capture structured fields for the required information in this contract.

If Issue Forms are not used, Markdown issue templates MUST preserve the same headings and required prompts.

Issue templates SHOULD explicitly capture, where relevant:

- existing code or pattern to reuse
- justification for any new abstraction
- error behavior expectations
- fallback authorization or explicit absence of fallback
- cleanup or deletion plan for replaced paths

The PR template MUST include checklists for:

- linked issue
- summary of change
- reused or extended existing code
- justification for new abstractions or files
- duplicate or obsolete code removed
- risk
- tests
- validation evidence
- negative-path or error-path validation
- docs updated
- rollout / rollback
- fallback added only with linked human authorization
- no pseudo code or placeholder production logic
- no orphaned or dead code left behind
- Greptile comments addressed
- follow-up issues created where needed

---

## 16. Minimum GitHub Actions workflow set

The repository SHOULD contain, directly or through reusable workflows, at least the following automations:

### 16.1 `ci.yml`

Runs on:

- `pull_request`
- `merge_group`

Should perform:

- checkout
- build
- lint / static analysis
- unit tests
- integration tests where applicable
- packaging / artifact creation when applicable
- artifact upload for evidence when applicable
- dead-code / unused-code / unreachable-code checks where supported
- placeholder-pattern or policy checks where supported

### 16.2 `policy.yml`

Should:

- verify PRs are linked to a driving issue
- verify required PR template fields are completed when feasible
- enforce repository-defined policy checks for forbidden placeholder markers
- fail when repository policy detects unauthorized fallback declarations or missing linked authorization where such automation is configured

### 16.3 `project-sync.yml`

Should:

- add new issues and PRs to the Project
- set default Project fields
- update state when PR is marked ready
- update state on merge and close

### 16.4 `release.yml`

Should:

- create or prepare releases
- generate release notes
- attach release artifacts when relevant

### 16.5 `deploy-*.yml`

If the repository deploys software, deployment workflows SHOULD:

- target named GitHub environments
- require approval for protected environments where appropriate
- emit deployment history
- publish smoke-check results

### 16.6 Optional but recommended

Where available and appropriate, add GitHub-native security and supply-chain workflows such as:

- dependency review
- code scanning
- secret scanning related checks
- Dependabot update handling

---

## 17. Definition of Ready

A Task is **Ready** only when:

- it has a parent Story or explicit justification for standing alone
- acceptance criteria are clear
- dependencies are known
- the implementation slice is small enough for one safe PR whenever practical
- the test approach is known
- expected reuse or extension targets are known when relevant
- expected error behavior is known when relevant
- fallback behavior is explicitly authorized or explicitly absent
- unclear scope has been moved back to the Requirement or Spec

A Story is **Ready** only when:

- its linked Spec is stable enough to implement
- its acceptance criteria are testable
- required Tasks are identified or the first Task is unambiguous

---

## 18. Definition of Done

A change is **Done** only when all of the following are true:

- code is merged through a PR
- driving issue linkage is correct
- required reviews and approvals are complete
- all blocking human review comments are resolved
- all Greptile comments are resolved per contract
- all required checks passed on the latest code
- the merged implementation is minimal, direct, and appropriately reused
- no unauthorized fallback exists
- no pseudo code, fake production behavior, or placeholder merge-path logic remains
- no unnecessary duplicate, orphaned, or dead non-test code was introduced
- required error behavior is explicit and validated
- any human-authorized fallback is documented and tested
- required deployment gates passed
- release notes exist when relevant
- post-release validation succeeded
- follow-up issues were created for any deferred non-blocking work
- the driving issue is closed only after validation is complete

---

## 19. Exception path — hotfixes and incidents

Hotfixes MAY use an accelerated path, but they are not exempt from traceability.

A hotfix MUST still have:

- an issue labeled `hotfix` or `incident`
- a PR
- human approval
- Greptile review attempt
- validation evidence
- a follow-up postmortem or corrective-action issue if process was compressed

A hotfix MUST NOT be used to justify pseudo code, dead scaffolding, or an unauthorized fallback.

If a true emergency requires bypassing normal protections, the bypass MUST be:

- performed only by an authorized maintainer
- documented in the issue or PR
- followed by a retrospective record in GitHub within one business day

Bypass is for service protection, not convenience.

---

## 20. AI agent-specific obligations

Any AI agent acting in this repository MUST:

- read and follow this file before proposing or changing code
- refuse to treat chat as the system of record for durable decisions
- search the repository for existing implementations before adding new ones
- prefer deletion, consolidation, and reuse over new code
- create or update GitHub artifacts when scope changes
- prefer small PRs and explicit follow-up issues over hidden TODOs
- raise or propagate explicit failures instead of silently degrading behavior
- never add fallback behavior without explicit human authorization in GitHub artifacts
- never mark work “done” without validation evidence
- never commit pseudo code, fake production behavior, or disconnected scaffolding
- never leave orphaned code islands behind
- never self-approve or self-merge as the sole reviewer
- reply to every review thread with a concrete resolution
- preserve traceability when splitting work

An AI agent MUST NOT:

- invent untracked scope
- silently ignore review comments
- merge while awaiting Greptile
- defer work without a linked issue
- close the driving issue before release validation is complete
- create speculative abstractions or single-use indirection without justification
- swallow errors or return misleading success
- keep duplicate or obsolete paths without explicit justification

---

## 21. Recommended repository skeleton

```text
.github/
  ISSUE_TEMPLATE/
    requirement.yml
    spec.yml
    epic.yml
    story.yml
    task.yml
    bug.yml
    spike.yml
    follow-up.yml
  PULL_REQUEST_TEMPLATE.md
  workflows/
    ci.yml
    policy.yml
    project-sync.yml
    release.yml
    deploy-staging.yml
    deploy-production.yml
CODEOWNERS
/docs/
  specs/
  adrs/
/agents.md
```

---

## 22. Recommended rollout for organizations

For multi-repository adoption:

1. create one **template repository** that contains this contract, standard templates, `CODEOWNERS`, and reusable workflows
2. create one **Project template** that contains the standard fields, views, and workflow conventions
3. apply **rulesets or protected branch settings** consistently across repositories
4. standardize one reusable **policy workflow** for issue linkage, template completeness, and repository policy checks
5. keep workflow logic centralized in reusable workflows where possible

---

## 23. Final governing rule

When there is tension between speed and traceability, choose the smallest GitHub-native step that preserves traceability.

When there is tension between convenience and safe merge, choose safe merge.

When there is tension between “probably fine” and explicit validation, choose explicit validation.

When there is tension between adding code and deleting or reusing code, prefer deleting or reusing code.

When there is tension between cleverness and clarity, choose clarity.

When there is tension between silent degradation and explicit failure, choose explicit failure.

When there is tension between “merge now” and **Awaiting Greptile**, the Greptile gate wins.
