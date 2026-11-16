# T027: Collaboration design spec (review gate for the impl tasks)

> **STATUS: pending.** Wave 2 (design only — **no implementation in this task**).
> Closes the *design* half of **S1**; implementation tasks are authored here after
> review. Their IDs are assigned by `taskpad new` at authoring time — T028/T029/T030
> already exist (storybook, SQL observability, Sentry/OTel); do not pre-reserve numbers.

## Goal

Finalize `specs/arch-hardening-specs/30-collaboration.md` (the draft landed with this
plan) into an implementation-ready design by **resolving its open calls with the user**
and then **author the implementation task files** (taskpad) so collaboration work can
be delegated session-by-session.

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
  admin/activity §5, task decomposition table §6, open calls §7).
- `specs/arch-hardening-specs/31-realtime-sse.md` — SSE realtime design (D-7):
  transport (§3), resume/resync (§4), fanout authorization (§5), client ladder (§6),
  presence (§7), test plan (§12), open calls (§13).
- `specs/arch-hardening-specs/32-comments-mentions.md` — comments/mentions design
  (D-7): schema (§1), API (§2), **permission matrix (§3)**, mention validation (§4),
  transactions/activity coupling (§6), open calls (§11).
- `specs/arch-hardening-specs/33-notifications.md` — inbox design (D-7): fanout rules
  (§2), watch model (§3), API (§4), purge/retention (§1, §7), open calls (§11).
- `specs/arch-hardening-specs/00-overview.md` — finding S1; D-1…D-6; global DoD;
  D-7 decision + amendment log (SSE/comment/notification scope promotion).
- `specs/planning-artifacts/architecture.md` (original FR43–49 admin requirements,
  entity sketches), `specs/planning-artifacts/prd.md:256` (user collaboration),
  `backend-rs/migrations/0001_baseline.sql` (current ownership model),
  `backend-rs/src/common/ids.rs` (Role enum — platform admin today),
  `specs/api-rewrite-rs-specs/30-db-schema.md` + `20-api-contract.md` (amendment
  targets for the implementation tasks), `backend-rs/tests/README.md`, `AGENTS.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `specs/arch-hardening-specs/30-collaboration.md` | Ratify the O-1…O-6 resolutions already recorded in §7 (user sign-off 2026-11-16; walk each: confirm or override); review O-7…O-21 defaults; refine §6 task table (rows A–L, J–L added by D-7) into final IDs/ACs; amend with review outcomes |
| `specs/tasks/T0xx-*.md` … | **NEW** (via `taskpad new`) — one file per §6 row (A–L), full house template, cross-user negative tests as standing AC. IDs come from taskpad (next free ≥ T031); do not hand-pick numbers |
| `specs/tasks/status.yaml` | Updated by `taskpad new` (depends edges per §6) |
| `specs/arch-hardening-specs/00-overview.md` | Annotate S1 design-half closed; wave-2 table pointer |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson (design-gate pattern) |

**Must NOT appear in the diff:** any `backend-rs`/`frontend` source change (design +
task authoring only), migrations.

## Implementation Steps

1. **Re-verify S1 against the tree** (fresh grep for `team|share|member|permission` in
   `backend-rs/src`, confirm no drift since the review — AGENTS.md: verify against the
   tree, don't trust the spec).
2. **Walk each open call with the user**: O-1…O-6 are already resolved and recorded
   (2026-11-16 sign-off — confirm they are ratified, don't re-litigate); O-7…O-21 carry
   recorded defaults (owned by `31`/`32`/`33`) — walk each: confirm or override;
   record decisions.
3. **Design review pass:** challenge the draft on: 404-vs-403 existence-hiding
   consistency with current handlers; list-filter SQL (indexed membership joins — no
   Rust post-filtering); personal-workspace backfill determinism; 409 conflict wire
   shape (T52 impact); activity-log synchronous-write cost inside txs. Adjust the file.
4. **Author the implementation tasks** via `taskpad new` per §6 rows A–L (each with
   Goal/Depends/Spec References/Files/TDD steps/Constraints/AC citing S1 + cross-user
   negative tests), wire depends edges (A → B → {C → {D, F, G → H}, E → I} per §6).
5. **Amendment map:** each task lists which `api-rewrite-rs-specs` files it will amend
   (T52 same-PR discipline) so contract work is pre-planned, not bolted on.
6. Update `00-overview.md` wave table; lesson.

## Constraints

- **No implementation** — this task ends at reviewed design + authored tasks (the
  whole point is delegable sessions after this gate).
- Realtime is in scope via **D-7 (user decision 2026-11-16, supersedes the D-2
  deferral clause)** — the design is `31-realtime-sse.md` (SSE + cache invalidation,
  no WebSockets). Do not re-litigate the transport; resolve O-7…O-21 at review.
- Migration strategy stays forward-only, backup-gated (T011), both engines.
- Design must keep single-user behavior intact (personal workspace — no forced
  workflow change for solo users).
- No commit without approval.

## Acceptance Criteria

- [ ] S1 re-verified against current tree (grep evidence pasted).
- [ ] O-1…O-6 ratified with user (decisions already recorded in `30-collaboration.md` §7);
      O-7…O-21 decided and recorded in `31`/`32`/`33`/`30` §7 (no open call left
      silently adopted).
- [ ] Design review outcomes incorporated (404/403, list-filter indexing, backfill,
      409 shape, activity-log cost — each explicitly addressed).
- [ ] Implementation tasks authored via `taskpad new` with depends edges per §6
      (A → B → {C → {D, F, G → H}, E → I}); each cites S1, carries
      cross-user negative tests as standing AC, and names its T52 amendment targets.
- [ ] `00-overview.md` annotated (S1 design-half closed); `taskpad status` shows the
      wave-2 graph; lesson appended.

## Notes

- **2026-11-16 amendment:** Wave-2 design scope was promoted by user decision
  (realtime via SSE; comments/mentions; in-app notifications). This task's review set
  now includes resolving O-1…O-6 (done and recorded in `30-collaboration.md` §7 —
  user sign-off 2026-11-16) plus O-7…O-21 (defaults recorded, owned by `31`/`32`/`33`);
  the implementation rows are A–L (J = SSE transport + presence, K = comments/mentions/
  notifications backend, L = frontend). O-1…O-6 are no longer open items for the review
  agenda — confirm they are ratified, then author the task files.
  (filled in during/after implementation)

## Lessons

(pending)
