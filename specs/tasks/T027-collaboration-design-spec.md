# T027: Collaboration design spec (review gate for T028+)

> **STATUS: pending.** Wave 2 (design only — **no implementation in this task**).
> Closes the *design* half of **S1**; implementation tasks T028+ are authored here
> after review.

## Goal

Finalize `specs/arch-hardening-specs/30-collaboration.md` (the draft landed with this
plan) into an implementation-ready design by **resolving its open calls with the user**
and then **author the T028+ task files** (taskpad) so collaboration work can be
delegated session-by-session.

This is the architecture gate for the product's core shift: single-user →
organization-wide cross-functional collaboration (100+ users, D-2).

## Depends On

(None — parallel-safe with Waves 0/1. Implementation depends on this.)

## Phase

2

## Critical

Yes — S1 is the strategic finding; every collaboration task descends from this design.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` — **the artifact this task
  finalizes** (domain model §1, authz kernel §2, migration §3, concurrency §4,
  admin/activity §5, provisional task table §6, open calls §7).
- `specs/arch-hardening-specs/00-overview.md` — finding S1; D-1…D-6; global DoD;
  out-of-scope list (realtime deferred).
- `specs/planning-artifacts/architecture.md` (original FR43–49 admin requirements,
  entity sketches), `specs/planning-artifacts/prd.md:256` (user collaboration),
  `backend-rs/migrations/0001_baseline.sql` (current ownership model),
  `backend-rs/src/common/ids.rs` (Role enum — platform admin today),
  `specs/api-rewrite-rs-specs/30-db-schema.md` + `20-api-contract.md` (amendment
  targets for T028+), `backend-rs/tests/README.md`, `AGENTS.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `specs/arch-hardening-specs/30-collaboration.md` | Resolve open calls O-1…O-6 (record decisions + rationale); refine §6 task table into final IDs/ACs; amend with review outcomes |
| `specs/tasks/T028-*.md` … `T036-*.md` | **NEW** (via `taskpad new`) — one file per §6 row, full house template, cross-user negative tests as standing AC |
| `specs/tasks/status.yaml` | Updated by `taskpad new` (depends edges per §6) |
| `specs/arch-hardening-specs/00-overview.md` | Annotate S1 design-half closed; wave-2 table pointer |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson (design-gate pattern) |

**Must NOT appear in the diff:** any `backend-rs`/`frontend` source change (design +
task authoring only), migrations.

## Implementation Steps

1. **Re-verify S1 against the tree** (fresh grep for `team|share|member|permission` in
   `backend-rs/src`, confirm no drift since the review — AGENTS.md: verify against the
   tree, don't trust the spec).
2. **Walk each open call O-1…O-6 with the user** (defaults are marked in
   `30-collaboration.md` §7 — confirm or override); record decisions in the file.
3. **Design review pass:** challenge the draft on: 404-vs-403 existence-hiding
   consistency with current handlers; list-filter SQL (indexed membership joins — no
   Rust post-filtering); personal-workspace backfill determinism; 409 conflict wire
   shape (T52 impact); activity-log synchronous-write cost inside txs. Adjust the file.
4. **Author T028–T036** via `taskpad new` per §6 (each with Goal/Depends/Spec
   References/Files/TDD steps/Constraints/AC citing S1 + cross-user negative tests),
   wire depends edges (T028→T029→{T030,T032}→… per §6).
5. **Amendment map:** each task lists which `api-rewrite-rs-specs` files it will amend
   (T52 same-PR discipline) so contract work is pre-planned, not bolted on.
6. Update `00-overview.md` wave table; lesson.

## Constraints

- **No implementation** — this task ends at reviewed design + authored tasks (the
  whole point is delegable sessions after this gate).
- Realtime stays out of scope (D-blocked in `00-overview.md`) — don't let the design
  drift into SSE/WS planning.
- Migration strategy stays forward-only, backup-gated (T011), both engines.
- Design must keep single-user behavior intact (personal workspace — no forced
  workflow change for solo users).
- No commit without approval.

## Acceptance Criteria

- [ ] S1 re-verified against current tree (grep evidence pasted).
- [ ] O-1…O-6 resolved with user and recorded in `30-collaboration.md` (no open calls
      left unresolved or silently defaulted).
- [ ] Design review outcomes incorporated (404/403, list-filter indexing, backfill,
      409 shape, activity-log cost — each explicitly addressed).
- [ ] T028–T036 authored via `taskpad new` with depends edges; each cites S1, carries
      cross-user negative tests as standing AC, and names its T52 amendment targets.
- [ ] `00-overview.md` annotated (S1 design-half closed); `taskpad status` shows the
      wave-2 graph; lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
