# T036: Collab: optimistic concurrency (version + 409)

> **STATUS: pending.** Phase 2. Wave 2 / row F. Takes cards and notes off
> last-write-wins for *content* writes. Position-only writes stay LWW (T010's
> fractional `sort_key`s; concurrent same-gap drops acceptable, documented
> in T010 D3).

## Goal

Optimistic concurrency on **cards and notes** (the two objects two people edit),
per `30-collaboration.md` §4:

1. Migration: `version INT NOT NULL DEFAULT 1` on `cards` and `notes`.
2. **Bump `version` on every content write** (title/description/body/due/
   status-like fields — not on position-only updates).
3. Requests that mutate content may send **`expected_version`** (optional — its
   absence preserves today's single-user behavior); mismatch → **409** with a
   dedicated **`conflict` problem+json** carrying the current resource
   (**O-4 resolved 2026-11-16**: problem-type *extension*, not `errors[]`).
4. **Frontend:** on 409 — refetch + toast "updated by someone else" + reload
   affordance; **never a silent overwrite**.
5. Once T037 (G) lands, `activity_log` doubles as the conflict-audit trail —
   no extra table. (T036 depends only on T033; if it lands before T037, the
   conflict trail starts existing when T037 lands — the 409 flow works either
   way.)

## Depends On

T033 (C) — enforcement on the board/card surface first, so the 409 path lives on
authorized routes.

## Phase

2

## Critical

No.

## Spec References

- `specs/arch-hardening-specs/30-collaboration.md` §4 (optimistic concurrency —
  `version` column or `updated_at` CAS; `If-Match`-style `expected_version`;
  409 + `conflict` problem type; bump on every content write; drag/move stays
  LWW; frontend 409 UX; **O-4 resolved: problem+json extension**, amended into
  `20-api-contract.md` via T52).
- `specs/arch-hardening-specs/00-overview.md` — finding **S1** (concurrency half);
  global DoD (both engines; persistence re-read; additive wire).
- `specs/api-rewrite-rs-specs/20-api-contract.md` — **amendment target** (T52):
  `conflict` problem type on card/note PATCH; `expected_version` optional field.
  `schema.d.ts` regen in the same PR.
- `specs/api-rewrite-rs-specs/30-db-schema.md` — **amendment target**: `version`
  columns on cards + notes.
- `backend-rs/src/error.rs` (dedicated 409 arm), `features/cards/`,
  `features/notes/`, frontend `features/cards/` (card detail panel),
  `features/notes/` (editor).
- `specs/arch-hardening-specs/32-comments-mentions.md` §2 — **comments do NOT
  get CAS** (conversational; `edited_at` informational) — do not extend this
  task to comments.
- `backend-rs/tests/README.md`.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/000N_collab_version_columns.sql` | **NEW** — `cards.version`, `notes.version INT NOT NULL DEFAULT 1` (additive; both engines) |
| `backend-rs/src/features/cards/{store.rs,routes.rs}` | content-write paths: bump + compare `expected_version`; position-only paths untouched |
| `backend-rs/src/features/notes/{store.rs,routes.rs}` | same |
| `backend-rs/src/error.rs` | `conflict` problem type arm (409, carries current resource JSON) |
| `specs/api-rewrite-rs-specs/{20-api-contract.md,30-db-schema.md}` | **AMEND** (T52 same-PR) |
| Frontend: card detail + notes editor | send `expected_version`; on 409 → toast + refetch + reload affordance (no silent overwrite) |
| `backend-rs/tests/api/optimistic_concurrency.rs` | **NEW** — black-box: stale `expected_version` → 409 + current resource; fresh → 200 + version bumped; absent → legacy behavior |
| `docs/lessons/rust-backend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** comments CAS (explicitly out), `updated_at` CAS
replacement of `version` (spec picks `version`), position-write behavior changes,
contract breaks (additive only).

## Implementation Steps

1. Enumerate **content-write** paths (card update: title/description/
   due_date/status/etc.; note update: title/body/etc.) vs **position-only**
   paths — the bump applies only to the former; paste the path list in §Notes.
2. Migration (additive; both engines); verify `DEFAULT 1` backfills legacy rows.
3. **TDD red:** stale `expected_version` → 409 with `conflict` problem body
   containing the *current* resource + new `version`; matching `expected_version`
   → 200 and `version` incremented by 1; absent `expected_version` → 200 with
   legacy last-write-wins semantics (documented); version bumps on content
   writes and **not** on move/reorder (assert via move test).
4. Frontend: include `expected_version` from the query cache on edits; 409
   handler → invalidate + toast "updated by someone else" + reload button; no
   silent overwrite; optimistic update rolls back on 409.
5. Cross-user negative: B stale-version on A's board → **404** (never reaches
   the 409 path — authz first, then concurrency).
6. T52 regen (openapi.json + `schema.d.ts`); contract doc amendments.
7. Both engines; `sqlx prepare --check`; persistence re-read everywhere; lesson.

## Constraints

- **409 carries the current resource** (`conflict` problem type) — no
  `errors[]` map (O-4). Extension of problem+json only.
- `expected_version` is **optional**; absence = legacy behavior (no forced
  upgrade of single-user flows).
- Position-only writes (move/reorder) are LWW — do not add versioning there.
- Comments excluded by design (`32` §2).
- Authz (T032/T033) runs **before** version compare — existence hiding first.
- No commit/push without explicit user approval.

## Acceptance Criteria

- [ ] Migration lands both engines; legacy rows default `version = 1`.
- [ ] Stale `expected_version` → 409 with `conflict` body carrying the current
      resource (red-first proven); fresh → 200 + bumped version; absent → 200
      legacy semantics. Closes the **S1** concurrency half.
- [ ] Position-only writes do **not** bump `version` (test proven).
- [ ] Frontend: 409 → toast + invalidate + reload affordance; optimistic
      update rolled back; no silent overwrite (component tests).
- [ ] Cross-user stale write → 404 (authz precedes concurrency) — test added.
- [ ] T52 same-PR: `20-api-contract.md` (`conflict` problem type,
      `expected_version`) + `30-db-schema.md` + `schema.d.ts` regen.
- [ ] Both engines green; no test weakened; persistence re-read on all
      mutation tests.
- [ ] Lesson appended to `docs/lessons/rust-backend.md`.

## Notes

(filled in during/after implementation — paste the content-write path list and
the 409 problem-body example)

## Lessons

(pending)
