# KanPad architecture hardening — plan index

> Sr. Software Architect review → phased remediation plan. Target: self-hosted by an
> organization for **cross-functional collaboration at 100+ users**, with **low
> infrastructure & maintenance cost** (zero new infra services: no Redis/k8s/queue;
> reuse MySQL). Companion to (not a replacement for) `specs/api-rewrite-rs-specs/`
> (backend rewrite) and `specs/security/` (security review) — findings from those are
> cited, not repeated.

## Reading order for a fresh agent

1. `00-overview.md` — constitution: scope, locked decisions (D-1…D-6), the **findings
   register** (R/P/S/C IDs with evidence), global DoD, wave overview.
2. `10-reliability.md` — Wave 0 design (findings R1–R8): current state → target design
   → edge cases → test plan per fix.
3. `20-performance.md` — Wave 1 design (R4, P1–P7, S2–S4): retry helper, board
   rendering, pagination, search, metrics, capacity methodology.
4. `30-collaboration.md` — Wave 2 **design only** (S1): workspace/ACL domain model,
   authorization kernel, migration strategy, optimistic concurrency, admin + activity
   log. Implementation tasks (T028+) are written after this design is reviewed.
5. `40-operations.md` — dual-path deployment (Docker appliance + cPanel runbook),
   backup/restore procedure + drill template, monitoring, secrets.

## Task list (source of truth = taskpad)

**`specs/tasks/` + `specs/tasks/status.yaml`** — tasks **T011–T027** for this plan
(T010 is a pre-existing task that gates T020). This directory deliberately has **no
`50-tasks.md`**: one task list only, so nothing can drift (repo lesson P2). The wave
table in `00-overview.md` is informational; run `taskpad status` / `taskpad next` /
`taskpad deps T0xx` for live state.

| Wave | Tasks |
|------|-------|
| 0 — reliability floor | T011 backups · T012 compose repair · T013 cache hygiene · T014 401/retry · T015 error states · T016 bundle · T017 Playwright · T018 secrets/deps · T019 docs sync |
| 1 — perf @100+ users | T010* sort keys → T020 retry helper · T021 render perf · T022 pagination · T023 FTS (conditional) · T024 metrics · T025 monitoring · T026 capacity |
| 2 — collaboration | T027 design spec → T028+ implementation (written after review) |

\* gates T020 (locked decision D-5).

## Fresh-agent protocol

1. Read `00-overview.md` (constitution + findings register).
2. Read your **one** task file in `specs/tasks/` + the spec sections it lists under
   "Spec References".
3. Implement TDD: failing test first, then code. No test deletion/weakening; red-proof
   any new gate.
4. Every POST/PATCH/DELETE test re-reads state via a second GET or direct sqlx query.
5. Run the global-DoD gates from `00-overview.md` and paste outputs as evidence.
6. Contract changes only via the T52 same-PR process. Report evidence in the PR/task log.
7. Append lessons to `docs/lessons/*.md`; fill the task's §Notes + check off ACs.
8. **No commit/push without explicit user approval.**

## Conventions

- Tasks follow the house template (Goal → Depends On → Phase → Critical → Spec
  References → Files → Implementation Steps → Constraints → Acceptance Criteria →
  Notes → Lessons), cite **finding IDs** in their ACs, and are created via
  `taskpad new` (`.taskpad` → `task-dir: specs/tasks`).
- Spec files are design/reference only — they never carry task status.
- If a spec file and the tree ever disagree, fix the disagreement and record it as an
  amendment in `00-overview.md` (same rule as `api-rewrite-rs-specs/00-overview.md`).
