# T019: Docs/README/proxy sync

> **STATUS: pending.** Wave 0. Closes **C1** (MEDIUM) + doc half of **C3**.
> **Depends on T012** (docs must describe the repaired compose appliance).

## Goal

The repo's docs still describe the deleted NestJS world and the deploy story is manual
and duplicated:

1. **README rewrite:** Rust stack (it still says NestJS/Fastify/TypeORM), correct build
   commands (drop `cd backend`, `npm run build:release`, `node migrate.js`), **both**
   deploy paths (D-1: Docker appliance + cPanel), link `docs/deployment.md` +
   `specs/arch-hardening-specs/40-operations.md`.
2. **Single proxy owner:** today `scripts/app-proxy.js` + three divergent
   `release/app-proxy{,-v2,-plus}.js` copies. Target: one tracked
   `scripts/app-proxy.js` (ephemeral-port version) is the source of truth; document it
   per the T007 contract (Host unchanged, XFF append, X-Forwarded-Proto);
   `release/` copies become build artifacts or are deleted.
3. **Stale Nest rollback references:** `docs/cutover.md:14` ("retain
   `kanpad-nest:final` as rollback") and `docs/data-migration-live.md:722` — Nest is
   deleted (`4051be68`); rollback story is backup-restore only (T011).
4. **Prod DB version:** fold in T011's `SELECT VERSION()` result — correct
   `docs/mariadb-compat.md:3` (10.6.28) vs `specs/security/06-…:77` (11.4.x) conflict.
5. **Specs index:** add `arch-hardening-specs/` to `specs/README.md` (if not already
   landed with this plan).

## Depends On

T012 (its appliance must exist to be documented).

## Phase

0

## Critical

No — but README is the first thing a delegatee/agent reads; wrong instructions cost
every session.

## Spec References

- `specs/arch-hardening-specs/40-operations.md` §1 (topology table), §6 (version
  check), §7 (deploy sequence + proxy consolidation design).
- `specs/arch-hardening-specs/00-overview.md` — findings C1, C3; D-1; global DoD.
- `README.md` (the defect — lines 14-18 stack, 47-131 deploy), `docs/deployment.md`
  (authoritative two-origin topology), `docs/cutover.md:14`,
  `docs/data-migration-live.md:722`, `docs/mariadb-compat.md:3`,
  `specs/security/06-infrastructure-deployment.md:77`,
  `specs/README.md` (index), `scripts/app-proxy.js` vs `release/app-proxy*.js`.

## Files to Create/Modify

| File | Change |
|---|---|
| `README.md` | Rust stack; both deploy paths; correct commands; feature-list accuracy check; link runbooks |
| `docs/deployment.md` | Only if proxy references need pointing at the single owner (topology unchanged) |
| `docs/cutover.md`, `docs/data-migration-live.md` | Annotate Nest-rollback references as retired (backup-restore only) — annotate, don't rewrite history |
| `docs/mariadb-compat.md`, `specs/security/06-…` | Correct prod version to T011's recorded `SELECT VERSION()` |
| `scripts/app-proxy.js` | Confirm it's the canonical version (ephemeral port, T007 contract) |
| `release/app-proxy*.js` | Delete or mark as generated artifacts (gitignored dir — decide: stop shipping copies; document regeneration) |
| `specs/README.md` | Index bullet for `arch-hardening-specs/` |
| `docs/lessons/ci-quality.md` | Dated, tagged lesson (docs drift + proxy duplication) |

**Must NOT appear in the diff:** topology changes to `docs/deployment.md`'s two-origin
model, app code.

## Implementation Steps

1. Inventory every stale claim (grep `backend/`, `NestJS`, `migrate.js`,
   `build:release`, `kanpad-nest`) — paste the list as evidence (C1 proof).
2. Rewrite README (stack, both paths, commands, links); keep it lean — runbooks carry
   detail.
3. Proxy consolidation: diff the copies, keep the tracked `scripts/app-proxy.js`,
   document the T007 contract checklist next to it; remove/quarantine `release/` copies.
4. Cutover/data-migration annotations (Nest rollback retired).
5. Version correction from T011's drill record (if T011 not done yet: leave a dated
   TODO anchored to T011's AC — do not guess the version).
6. `specs/README.md` index bullet.
7. Lesson.

## Constraints

- Annotate spec history, don't rewrite it (house rule from T008).
- Both deploy paths documented as first-class (D-1) — neither "the" way.
- Don't invent facts: commands must be verified by running them (or marked untested).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Grep inventory of stale Nest/`backend/` claims pasted; README no longer contains
      them (closes **C1** docs half).
- [ ] README documents both deploy paths with verified commands + links to
      `docs/deployment.md` and `40-operations.md`.
- [ ] One canonical proxy script documented with the T007 contract; divergent copies
      removed or marked generated (closes rest of **C1**).
- [ ] Nest-rollback references annotated as retired (backup-restore only, → T011).
- [ ] Prod DB version consistent across `mariadb-compat.md` and security spec (or
      anchored TODO → T011) (doc half of **C3**).
- [ ] `specs/README.md` indexes `arch-hardening-specs/`; lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
