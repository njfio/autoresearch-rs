# GitHub Project Template Blueprint

Use this blueprint to create the matching GitHub Project for this repo contract.

## Fields
- Status (single-select): Intake, Needs Spec, Ready, In Progress, Blocked, In Review, Awaiting Greptile, Awaiting Validation, Ready to Merge, Merged, Validating, Done
- Artifact Type (single-select): Requirement, Spec, Epic, Story, Task, Bug, Spike, Follow-up
- Priority (single-select): P0, P1, P2, P3
- Risk (single-select): Low, Medium, High
- Team (single-select/text)
- Area (single-select/text)
- Iteration (iteration field)
- Target Date (date)
- Start Date (date)
- Effort/Size (number/single-select)
- Milestone (text)
- Linked PR (text)
- Environment (single-select): none, staging, production

## Saved Views
1. Intake
2. Roadmap
3. Current Iteration
4. Blocked
5. In Review
6. Awaiting Greptile
7. Release Readiness
8. Post-release Follow-ups

## Recommended Automations
- New Requirement/Spec -> Status=Intake
- New Task -> Status=Ready
- Draft PR opened -> Status=In Review
- PR ready for review / material update -> Status=Awaiting Greptile
- Checks + reviews complete -> Status=Ready to Merge
- PR merged -> Status=Merged
- Deployed -> Status=Validating
- Acceptance validated -> Status=Done
