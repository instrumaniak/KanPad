# T010: Fractional sort-key ordering for positions

> **STATUS (2026-11-11): PLAN saved — not started.** Research + design complete; see §Notes for
> the design decisions and the two open calls (release strategy, new-endpoint clamp semantics).

## Goal

Replace the integer `position` renumbering scheme on `columns`, `cards` and `checklist_items`
with a **fractional sort key** (`sort_key`, string fractional index), so that every position
change becomes a **single-row write** that never touches adjacent rows, plus add the two missing
reorder endpoints (`PATCH /columns/{id}` with `position`, `PATCH /checklist-items/{id}` with
`position`).

**Why (findings from the current implementation):**

- `cards`: every create-with-position, delete, same-column move and cross-column move rewrites
  the positions of all affected neighbours (range `UPDATE … ±1` bands); a cross-column move
  shifts tails in **both** columns; each drag runs `lock_cards_for_update`
  (`SELECT id … ORDER BY position ASC FOR UPDATE`) which locks **every card row in both columns**
  and needs a 5-attempt retry loop (`1213`/`1205`) to survive gap-lock deadlocks.
- `columns`: delete renumbers all subsequent sibling columns; there is **no column-reorder
  endpoint at all**; `PATCH {sort_order}` rewrites every card position in the column.
- `checklist_items`: explicit-position insert writes **verbatim** (no gap open → silent
  duplicate positions), delete closes **no gap** (holes persist), and there is **no reorder
  endpoint**.
- All three tables have a **non-unique** `(parent, position)` index and every order query is a
  bare `ORDER BY position ASC` — ties are possible and order is non-deterministic under them.

**Web research conclusion (drawdb "Reorderable list in SQL", LexoRank/Jira, Figma fractional
indexing, tentaclelabs, Velocity):** integer renumbering is the right default only up to a few
hundred rows and stays O(band) per move; string fractional keys (LexoRank-style, base62,
binary collation) give O(1) single-row moves, never exhaust precision, are safe under
concurrency via `UNIQUE(parent, key)` + retry, and need rebalancing only when keys bloat
(Jira: 128 chars). Floats and linked lists are rejected outright. KanPad's scale would tolerate
integers, but the user chose the fractional scheme for correctness + concurrency headroom.

## Depends On

(None — no task dependency. Invokes the **T52 contract-amend process** as a step, not a dep.)

## Phase

0

## Critical

No — personal-scale app, no production urgency. It is a schema migration, so it lands as **one
release with a seconds-long migration** (default; see §Notes open call 1).

## Spec References

- `specs/api-rewrite-rs-specs/20-api-contract.md` — `columns`/`cards`/`checklists` endpoint
  sections (position semantics text to amend).
- `specs/api-rewrite-rs-specs/21-api-standard-shapes.md` — wire shapes (position stays an
  integer; only additive optional fields).
- `specs/api-rewrite-rs-specs/30-db-schema.md` — line 10 index list (non-unique
  `IDX_*_position` rationale to replace with `UNIQUE(parent, sort_key)`), line 24 lock-order
  rule ("lock reorder rows in stable `(column_id, position)` order" — becomes obsolete).
- `specs/api-rewrite-rs-specs/40-typegen.md` + `50-tasks.md` §T52 — same-PR contract process
  (spec + utoipa + `openapi.json` + typegen).
- `CONTRACT-CHANGES.md` — new entry (behaviour normalizations, new fields/endpoints).
- `backend-rs/tests/README.md` — test-harness rules (explicit imports, ephemeral ports,
  `assert_problem` labels, migration-checksum rule).
- `AGENTS.md` — E2E persistence-verification rule, MySQL+MariaDB rule, TDD, no-commit-without-ask.
- `docs/lessons/rust-backend.md` — lesson to append.
- Research: drawdb.app/blog/reorderable-list-sql (integer vs fractional trade-off table,
  MySQL `ORDER BY` renumber trick), LexoRank/Jira bucket rebalance thresholds, Figma
  `fractional-indexing` (rocicorp) format, rezee.app benchmark (renumbering vs midpoint).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/0003_sort_key_fractional_ordering.sql` | **NEW** — add `sort_key` (see D4), backfill, `UNIQUE(parent, sort_key)`, drop `position` + old `IDX_*_position` ×3 tables |
| `backend-rs/src/common/order_key.rs` | **NEW** — key library wrapper: `key_between`, `keys_between`, rebalance-threshold helper |
| `backend-rs/src/common/mod.rs` | register `order_key` |
| `backend-rs/Cargo.toml` | add `lexicon_fractional_index` (or vendor into `order_key.rs` — see D2) |
| `backend-rs/src/common/card_summary.rs` | board-wide fetch: `ORDER BY column_id, sort_key`, drop `position` from select, assign per-column index in `assemble_summaries` |
| `backend-rs/src/features/columns/{store,routes,types}.rs` | read path (computed position), append/insert/delete/move via keys, **new `position` on `UpdateColumnInput`**, delete `close_column_gap`/`max_position_for_update` |
| `backend-rs/src/features/cards/{store,routes,types}.rs` | read path, create/move/delete/sort/bulk-move via keys, delete `open_card_gap`/`close_card_gap`/`shift_cards_up`/`shift_cards_down`/`lock_cards_for_update`/`fetch_card_spot_for_update`→single-row lock, `max_position_for_update`→last-key read, retry matcher gains `1062` |
| `backend-rs/src/features/checklists/{store,routes,types}.rs` | read path, item create/delete + **new `position` on `UpdateChecklistItemInput`** |
| `backend-rs/src/features/boards/store.rs` | `insert_default_columns` (3 keys via `keys_between`), `fetch_board_columns` (`ORDER BY sort_key`, computed position) |
| `backend-rs/.sqlx/` | regenerated via `cargo sqlx prepare` (CLI 0.9 pin; DB named `kanpad`, MySQL only — T009 rule) |
| `backend-rs/tests/api/` | new suite(s) for migration/backfill + adjacent-unchanged proofs; update `cards_move.rs`, `columns_crud.rs`, `columns_sort.rs`, `columns_move_cards.rs`, `checklist_items.rs`, `golden.rs`, `e2e_columns_cards.rs`, `collation.rs`, `sql_portability.rs` as needed |
| `backend-rs/openapi.json` | regenerated (`cargo run -p backend-rs -- export-openapi`) |
| `frontend/src/lib/api/schema.d.ts` | regenerated (`npm run gen:api`) — additive optional fields only |
| `specs/api-rewrite-rs-specs/{20,21,30}-*.md` | T52 amendments (30: schema/indexes/lock rule) |
| `CONTRACT-CHANGES.md` | new entry (D5 normalizations + new endpoints) |
| `docs/lessons/rust-backend.md` | dated, tagged lesson |
| `specs/tasks/T010-*.md` | this file — AC checkboxes + §Notes filled in during implementation |

**Must NOT appear in the diff:** `_sqlx_migrations` (never hand-edited), `.env`/`DATABASE_URL`,
frontend feature code (existing features must pass **untouched** — regression proof of D1).

## Implementation Steps

TDD throughout: each step writes its failing tests first.

1. **T010a — order-key module.** Spike `lexicon_fractional_index` on crates.io (health,
   license, `key_between`/`n_keys_between` API); vendor the algorithm into
   `src/common/order_key.rs` if the crate is lacking. Unit/property tests: strict
   `a < between(a,b) < b`, `keys_between` sorted & count-exact, empty bounds (prepend/append),
   repeated same-gap growth, rebalance-threshold trigger at 64 chars.
2. **T010b — migration `0003` + backfill test.** Ordered statements per D4. Test: seed rows
   with gaps **and** duplicate positions, set `sort_key = ''`, run the backfill statement,
   assert order preserved (position, id tiebreak), keys unique per parent, contiguous computed
   indexes; plus a fresh-boot test (migrate → CRUD works). Green on **both** MySQL 8.4 and
   MariaDB 10.6 (existing CI matrix / `sql_portability.rs`).
3. **T010c — read path.** All `ORDER BY position` → `ORDER BY sort_key` (+ `, id` tiebreak);
   position computed at read: Rust-side `enumerate` for lists/pages (page = `offset + i`) and
   per-column/per-checklist counters for grouped fetches; correlated
   `COUNT(*) … sort_key < ?` in `fetch_*_owned` single-row reads. Tests: detail/list/page/board
   embed ordering + computed position correctness.
4. **T010d — cards write path.** Create (append via last-key read; insert-at-index via
   neighbors), move (single-row conditional `UPDATE … WHERE id = ? AND column_id = ?` +
   `COUNT(*)` clamp, replacing whole-column locks; same-card race still → `Restart`), delete
   (**no gap close**), `sort_order` rewrite (`keys_between(None, None, n)` + one `UPDATE …
   CASE`), bulk `move-cards` (`keys_between(Some(last), None, n)`, **no source close**).
   Extend retry matcher to `1213 | 1205 | 1062`. **Crown-jewel tests:** for every operation,
   capture all sibling `sort_key`s before/after and assert they are byte-identical (the core
   no-adjacent-impact proof) + one persistence assertion per data-modifying endpoint (AGENTS.md
   rule) + existing `cards_move.rs` race/retry tests stay green.
5. **T010e — columns write path + move endpoint.** Default-column inserts via `keys_between`,
   delete without gap close, **new `PATCH /columns/{id}{position?}`** (clamp `[0, N-1]`,
   between-neighbors, single-row write). Tests incl. persistence.
6. **T010f — checklist-items write path + move endpoint.** Create at index via neighbors
   (fixes verbatim-duplicate bug), delete gapless (now correct by design), **new
   `PATCH /checklist-items/{id}{position?}`**. Tests incl. persistence.
7. **T010g — T52 contract pass (same PR, mandatory).** Amend `20-api-contract.md`,
   `21-api-standard-shapes.md`, `30-db-schema.md` (index list + lock-order rule + concurrency
   rationale), `CONTRACT-CHANGES.md` entry → utoipa annotations → regenerate `openapi.json` →
   `npm run gen:api` → `tsc --noEmit` → `openapi_snapshot` test green →
   `gen:api -- --check` green.
8. **T010h — cleanup & gates.** Rewrite store module headers (new invariants), remove dead
   code, `cargo sqlx prepare` (regenerate `.sqlx/`), `cargo fmt --check`, `clippy --locked
   --workspace --all-targets -- -D warnings -A clippy::pedantic`, review `golden.rs`/`e2e_*`
   expectations (only genuinely-changed semantics — contiguity — may be updated), append lesson
   to `docs/lessons/rust-backend.md`, fill §Notes + check off ACs here. Full matrix: `cargo test`
   on MySQL **and** MariaDB, frontend suite (must pass untouched), typegen check.

## Constraints

- **No commit/push without explicit user approval** (AGENTS.md); then Conventional Commit,
  20–50 char summary line.
- **T52 same-PR rule**: spec + utoipa + `openapi.json` + `schema.d.ts` ship together — no
  standalone typegen PRs.
- **Wire contract frozen to integer `position`** (D1): responses compute it, requests accept it
  as a target index. No `sort_key`/`rank` leaks onto the wire; existing frontend feature code
  must not need edits.
- **`ascii_bin` collation is mandatory** for `sort_key` (base62 is case-sensitive; any `*_ci`
  collation silently reorders keys). Extend `collation.rs` to pin this.
- **Both MySQL 8.4 and MariaDB 10.6** must pass every migration, query and test (prod =
  MariaDB; window functions/`ELT`/`ascii_bin` are supported on both — verify, don't assume).
- **sqlx discipline**: never hand-edit `_sqlx_migrations`; regenerate `.sqlx/` with the pinned
  0.9 CLI **on MySQL, database named `kanpad`** (T009 rules); `SQLX_OFFLINE=true` in test runs.
- **Test harness rules** (`backend-rs/tests/README.md`): explicit imports (never glob), shared
  helpers only from `helpers.rs`/`wire.rs`, ephemeral ports via `wire::bind_blackbox()`, every
  `assert_problem` takes a `what` label, session SQL scoped `WHERE id = ?`.
- **Persistence verification** for every PATCH/POST/DELETE: one assertion via a *different* API
  call or DB query (AGENTS.md E2E rule).
- Backfill must be deterministic under today's duplicate positions (`ORDER BY position, id`).

## Acceptance Criteria

- [ ] Migration `0003` applies clean on fresh **and** existing (pre-populated, gapped,
      duplicate-position) databases on MySQL 8.4 and MariaDB 10.6; backfill test proves order
      preserved, keys unique per parent, no `position` column remains.
- [ ] **Adjacent-unchanged proof tests** green for cards: create-at-index, same-column move
      (up/down), cross-column move, delete, `sort_order` rewrite, bulk move-cards — siblings'
      `sort_key`s byte-identical before/after; only the intended row(s) change.
- [ ] New endpoints `PATCH /columns/{id}{position?}` and `PATCH /checklist-items/{id}{position?}`
      work, clamp per §Notes decision 2, and each has a persistence assertion.
- [ ] Existing suites green unchanged in behaviour: `cards_move.rs` (race/retry),
      `columns_sort.rs`, `columns_move_cards.rs`, `checklist_items.rs`, `golden.rs`,
      `e2e_columns_cards.rs`.
- [ ] Duplicate-key races retry (`1062` added to the retryable matcher); same-card column race
      still restarts the tx (no whole-column lock remains in the codebase).
- [ ] `cargo fmt --check` 0 · `clippy … -D warnings -A clippy::pedantic` 0 ·
      `cargo sqlx prepare --check` green · `.sqlx/` diff limited to changed queries.
- [ ] T52 set complete: specs amended, `CONTRACT-CHANGES.md` entry, `export-openapi`
      byte-fresh `openapi.json`, `npm run gen:api` + `-- --check` green, `tsc --noEmit` clean,
      `openapi_snapshot` green.
- [ ] **Frontend suite passes with zero feature-code edits** (proves wire compatibility).
- [ ] Lesson appended to `docs/lessons/rust-backend.md` (dated, tagged); §Notes below filled in.

## Notes

### Design decisions (locked at plan time)

- **D1 — wire stays integer `position`, computed at read.** The frontend uses integer
  `position` for optimistic updates and drag math everywhere (`drag-drop-context.tsx`,
  `column.tsx`), so responses compute a contiguous 0-based index (Rust `enumerate` for lists,
  correlated `COUNT` for single-row reads) and requests keep sending a target index that the
  server translates to a between-neighbors key. Exposing `rank` on the wire was rejected:
  total DTO + frontend churn for zero user-visible gain.
- **D2 — key format/library.** `sort_key VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin
  NOT NULL`; library `lexicon_fractional_index` (port of rocicorp/fractional-indexing —
  `key_between` + `keys_between`), vendor fallback if crates.io health is poor. Rejected:
  `lexorank` (bucket machinery), `fractional_index` (no n-keys helper), float midpoint
  (silent ties ~50 inserts), linked list (broken-chain risk), hand-rolled midpoint math
  (edge-case surface). Rebalance guard: generated key > 64 chars → regenerate the parent's
  keys evenly in the same tx (Jira rebalances at 128; we catch it earlier).
- **D3 — concurrency.** `UNIQUE(parent, sort_key)` turns race losers into `1062` → retry
  (extend `is_transient_lock_error` → `1213 | 1205 | 1062`); lock only the moving row
  (`SELECT … FOR UPDATE WHERE id = ?`) preserving same-card-race `Restart`; neighbor reads stay
  unlocked (last-write-wins for concurrent same-gap drops — acceptable, per Figma/Rezee);
  append = `ORDER BY sort_key DESC LIMIT 1 FOR UPDATE` (single-row lock, narrower than today's
  `MAX(…) FOR UPDATE` range lock); clamp upper bound from plain `COUNT(*)`.
- **D4 — one migration, ordered statements:** add `sort_key` (DEFAULT `''`) → backfill
  (`ROW_NUMBER() OVER (ORDER BY position, id)` → `CONCAT('c', ELT×3 base62)`; fixed 3-digit
  base62 after head `c` is a valid fractional key, monotonic for up to 62³ = 238,328 rows per
  parent) → `UNIQUE(parent, sort_key)` → drop `position` + old indexes. Single release,
  seconds of downtime (default — see open call 1).
- **D5 — behaviour normalizations recorded in `CONTRACT-CHANGES.md`:** checklist-item
  positions become contiguous computed indexes (were verbatim-with-holes); duplicate positions
  become impossible; deletes never renumber siblings anywhere; all order queries gain `, id`
  tiebreakers.

### Open calls (confirm at implementation start)

1. **Release strategy:** default = single release (one migration add→backfill→unique→drop,
   brief downtime). Alternative = two-release expand/contract (zero downtime, more ceremony).
   *Decision: single release unless the user says otherwise.*
2. **New-endpoint position semantics:** default = **clamp** (card-move convention —
   drag-driven, never 400) for both new endpoints, documented as a small intra-entity
   inconsistency (checklist-item *create* still 400s on negatives). Alternative = item-move
   400s on negatives to match item-create. *Decision: clamp unless the user says otherwise.*

### Out of scope

- Frontend drag UI for columns / checklist items (API surface only; typegen is additive).
- Exposing `sort_key`/`rank` on the wire.
- A periodic rebalance job (only the in-tx guard).
