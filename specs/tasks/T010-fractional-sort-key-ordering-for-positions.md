# T010: Fractional sort-key ordering for positions

> **STATUS (2026-11-15): PLAN reviewed & corrected — not started.** Cross-checked against the
> codebase, the CI workflow and upstream sources (see §Review notes 2026-11-15). Two open calls
> remain (release strategy, new-endpoint clamp semantics); DB target is now MySQL 8.4 LTS /
> MariaDB 11.4 LTS per AGENTS.md.

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
- `AGENTS.md` — E2E persistence-verification rule, MySQL (8.4 LTS) & MariaDB (11.4 LTS)
  rule, TDD, no-commit-without-ask.
- `docs/lessons/rust-backend.md` — lesson to append.
- Research (verified 2026-11-15): drawdb.app/blog/reorderable-list-sql (integer vs fractional
  trade-off table, MySQL `ORDER BY` renumber trick), LexoRank/Jira bucket rebalance thresholds,
  Figma `fractional-indexing` (rocicorp — README + source read), rezee.app benchmark
  (renumbering vs midpoint), crates.io / docs.rs for `lexicon_fractional_index` 0.0.3, MariaDB
  11.4 docs (`ROW_NUMBER`, `ELT`, collations), MySQL 8.4 LTS manual (Online DDL, locking reads).

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/migrations/0003_sort_key_fractional_ordering.sql` | **NEW** — add `sort_key` (see D4); backfill partitioned by parent (`ROW_NUMBER() OVER (PARTITION BY <parent> ORDER BY position, id)`); `UNIQUE` constraints `UQ_columns_board_sort_key`, `UQ_cards_column_sort_key`, `UQ_checklist_items_checklist_sort_key`; drop `position` + old `IDX_*_position` ×3 tables |
| `backend-rs/src/common/order_key.rs` | **NEW** — key library wrapper: `key_between`, `keys_between`, rebalance-threshold helper (two-phase rewrite — see D2) |
| `backend-rs/src/common/mod.rs` | register `order_key` |
| `backend-rs/Cargo.toml` | `lexicon_fractional_index` not adopted as a dependency by default — **vendor** (verified 2026-11-15: 0.0.3, MIT, 284 LOC, stale since 2022-01-17; drop the dep if vendoring — see D2) |
| `backend-rs/src/common/card_summary.rs` | board-wide fetch: `ORDER BY column_id, sort_key`, drop `position` from select, assign per-column index in `assemble_summaries` |
| `backend-rs/src/common/` (retry helper) | move/extract `is_transient_lock_error` from `cards/routes.rs`; `1213 | 1205` + `1062` scoped to `UQ_*_sort_key` constraint names (see D3) |
| `backend-rs/src/features/columns/{store,routes,types}.rs` | read path (computed position), append/insert/delete/move via keys, **new `position` on `UpdateColumnInput`**, delete `close_column_gap`/`max_position_for_update` |
| `backend-rs/src/features/cards/{store,routes,types}.rs` | read path, create/move/delete/sort/bulk-move via keys, delete `open_card_gap`/`close_card_gap`/`shift_cards_up`/`shift_cards_down`/`lock_cards_for_update`/`fetch_card_spot_for_update`→single-row lock, `max_position_for_update`→last-key read, retry matcher gains constraint-scoped `1062` |
| `backend-rs/src/features/checklists/{store,routes,types}.rs` | read path, item create/delete (serialized append — see T010f) + **new `position` on `UpdateChecklistItemInput`** |
| `backend-rs/src/features/boards/store.rs` | `insert_default_columns` (3 keys via `keys_between`), `fetch_board_columns` (`ORDER BY sort_key`, computed position) |
| `.github/workflows/backend-rs.yml` | bump `backend-mariadb` service `mariadb:10.6` → `mariadb:11.4` (10.6 is EOL 2026-07-06; AGENTS.md targets 11.4 LTS) **and extend that leg to run the new migration/backfill + ordering suites** — otherwise the full-matrix AC is unverifiable in CI (it currently runs only `sql_portability` + `auth_session` + session-store). May split to a prerequisite chore; then scope the MariaDB CI AC to `migrate run` + `sql_portability` |
| `backend-rs/.sqlx/` | regenerated via `cargo sqlx prepare` (CLI 0.9 pin; DB named `kanpad`, MySQL only — T009 rule) |
| `backend-rs/tests/api/` | new suite(s) for migration/backfill + adjacent-unchanged proofs (e.g. `sort_key_migration.rs`); update `cards_move.rs`, `columns_crud.rs`, `columns_sort.rs`, `columns_move_cards.rs`, `checklist_items.rs`, `golden.rs`, `e2e_columns_cards.rs`, `collation.rs`, `sql_portability.rs` as needed |
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

1. **T010a — order-key module.** crates.io facts verified 2026-11-15: `lexicon_fractional_index`
   0.0.3, MIT, 284 LOC, single maintainer, last publish 2022-01-17; API = `key_between` +
   `n_keys_between` + `float64_approx`, and the algorithm is byte-compatible with rocicorp
   `fractional-indexing` (same `BASE62_DIGITS` alphabet, same head-length rules, same
   `validate_order_key` — e.g. keys whose float part ends in `0` are rejected). Default: **vendor**
   the algorithm into `src/common/order_key.rs` with attribution (rocicorp is CC0); keep the
   crate as reference only. Unit/property tests: strict `a < between(a,b) < b`, `n_keys_between`
   sorted & count-exact, empty bounds (prepend/append), repeated same-gap growth,
   rebalance-threshold trigger at 64 chars (two-phase — see D2), **interop round-trip: adjacent
   backfilled keys (`c000`, `c001`, …) feed `key_between` and yield a strictly-between valid key**.
2. **T010b — migration `0003` + backfill test.** Ordered statements per D4; every backfill
   window partitions by parent (`PARTITION BY board_id` / `column_id` / `checklist_id`). Test:
   seed rows with gaps **and** duplicate positions, set `sort_key = ''`, run the backfill
   statement, assert order preserved (position, id tiebreak), keys unique per parent, first key
   per parent is `c000`, keys strictly monotonic per parent; **round-trip assertion** via the
   (vendored) `key_between` — sampled adjacent pairs must yield a strictly-between valid key,
   proving SQL-generated keys parse and interoperate; plus a fresh-boot test (migrate → CRUD
   works). Green on **both** MySQL 8.4 and MariaDB 11.4 LTS (CI matrix incl. the bumped
   `backend-mariadb` leg).
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
   Extend the shared retry matcher (extracted to `common/`) to `1213 | 1205` plus `1062`
   **scoped to the `UQ_*_sort_key` constraint names** (match the `for key '…'` suffix in the
   MySQL error message); unrelated unique violations must not be retried. Fall back to
   workspace-wide `1062` retry only if scoping proves unworkable in `AppError`, and record the
   cost (5× backoff retries on permanent duplicates like `card_labels`) in §Notes.
   **Crown-jewel tests:** for every operation,
   capture all sibling `sort_key`s before/after and assert they are byte-identical (the core
   no-adjacent-impact proof) + one persistence assertion per data-modifying endpoint (AGENTS.md
   rule) + existing `cards_move.rs` race/retry tests stay green.
5. **T010e — columns write path + move endpoint.** Default-column inserts via `keys_between`,
   delete without gap close, **new `PATCH /columns/{id}{position?}`** (clamp `[0, N-1]`,
   between-neighbors, single-row write). Append paths keep today's serialization semantics by
   default: last-key read `ORDER BY sort_key DESC LIMIT 1 FOR UPDATE` + key computation in Rust
   + INSERT in one tx (a single-record lock — same concurrency contract as today's
   `MAX(position) FOR UPDATE`); do NOT adopt the unlocked-read + `1062`-retry pattern here
   unless cards-only retry proves out. Tests incl. persistence.
6. **T010f — checklist-items write path + move endpoint.** Create at index via neighbors
   (fixes verbatim-duplicate bug) — append keeps serialized last-key `FOR UPDATE` (same
   contract as (5)); delete gapless (now correct by design), **new
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
   on MySQL **and** MariaDB 11.4 LTS (bumped `backend-mariadb` leg), frontend suite (must pass
   untouched), typegen check.

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
- **MySQL 8.4 LTS and MariaDB 11.4 LTS** must pass every migration, query and test (prod =
  MariaDB 11.4 LTS). Verified 2026-11-15: `ROW_NUMBER() … PARTITION BY` and `ELT` supported on
  both; `ascii_bin` collation present on both (MariaDB 11.4 id 65, sortlen 1); MySQL 8.4 does
  ADD/DROP COLUMN as `ALGORITHM=INSTANT` (metadata-only, concurrent DML permitted); drops of the
  old non-unique indexes are INPLACE + concurrent DML. MariaDB 11.4 DDL characteristics —
  verify on the CI leg, don't assume.
- **sqlx discipline**: never hand-edit `_sqlx_migrations`; regenerate `.sqlx/` with the pinned
  0.9 CLI **on MySQL, database named `kanpad`** (T009 rules); `SQLX_OFFLINE=true` in test runs.
- **Test harness rules** (`backend-rs/tests/README.md`): explicit imports (never glob), shared
  helpers only from `helpers.rs`/`wire.rs`, ephemeral ports via `wire::bind_blackbox()`, every
  `assert_problem` takes a `what` label, session SQL scoped `WHERE id = ?`.
- **Persistence verification** for every PATCH/POST/DELETE: one assertion via a *different* API
  call or DB query (AGENTS.md E2E rule).
- Backfill must be deterministic under today's duplicate positions (`ORDER BY position, id`,
  partitioned by parent).

## Acceptance Criteria

- [ ] Migration `0003` applies clean on fresh **and** existing (pre-populated, gapped,
      duplicate-position) databases on MySQL 8.4 LTS and MariaDB 11.4 LTS; backfill is
      `PARTITION BY`-ed per parent; backfill test proves order preserved, keys unique per
      parent, first key per parent `c000`, monotonic per parent, no `position` column remains,
      **and the vendored `key_between` round-trip holds for sampled adjacent pairs**.
- [ ] **Adjacent-unchanged proof tests** green for cards: create-at-index, same-column move
      (up/down), cross-column move, delete, `sort_order` rewrite, bulk move-cards — siblings'
      `sort_key`s byte-identical before/after; only the intended row(s) change.
- [ ] New endpoints `PATCH /columns/{id}{position?}` and `PATCH /checklist-items/{id}{position?}`
      work, clamp per §Notes decision 2, and each has a persistence assertion.
- [ ] Existing suites green unchanged in behaviour: `cards_move.rs` (race/retry),
      `columns_sort.rs`, `columns_move_cards.rs`, `checklist_items.rs`, `golden.rs`,
      `e2e_columns_cards.rs`.
- [ ] Duplicate-key races retry (`1062` added to the shared matcher, **scoped to the
      `UQ_*_sort_key` constraint names** — unrelated unique violations are not retried); column
      / checklist-item append paths keep their serialized last-key `FOR UPDATE` design (or this
      AC and D3 are amended together); same-card column race still restarts the tx (no
      whole-column lock remains in the codebase).
- [ ] CI `backend-mariadb` leg runs `mariadb:11.4` with the migration/backfill + ordering
      suites green (or this AC is scoped to `migrate run` + `sql_portability` by explicit
      maintainer decision, recorded in §Notes).
- [ ] Rebalance guard rewrites a parent's keys in **two phases** (park at temporary unique
      keys, then assign final even keys) — forced-bloat test proves no transient unique
      violation and evenly-distributed final keys; the `> 64`-char trigger fires.
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
  NOT NULL`; library `lexicon_fractional_index` facts verified 2026-11-15: 0.0.3, MIT, 284 LOC,
  single maintainer, stale (last publish 2022-01-17); API = `key_between` + `n_keys_between` +
  `float64_approx`; algorithm byte-compatible with rocicorp `fractional-indexing` (same
  `BASE62_DIGITS = 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz`; head chars
  `a-z` → int-part length `head - 'a' + 2` — so `c` means 3 digits — and `A-Z` →
  `'Z' - head + 2`; `validate_order_key` rejects keys whose float part ends in `0`). Key format
  is case-sensitive; byte order under `ascii_bin` is exactly the alphabet order, so the
  collation is load-bearing. **Default: vendor the algorithm** into `src/common/order_key.rs`
  with attribution (rocicorp is CC0); crate kept as reference/fixture, not a dependency.
  Rejected: `lexorank` (bucket machinery), `fractional_index` (no n-keys helper), float midpoint
  (silent ties ~50 inserts), linked list (broken-chain risk), hand-rolled midpoint math
  (edge-case surface). Rebalance guard: generated key > 64 chars → regenerate the parent's
  keys evenly **in two phases** (phase 1 park every row at a unique temporary key, e.g.
  `tmp-<id>`, outside the base62 key space; phase 2 assign final even keys — a single-phase
  even rewrite transiently violates `UNIQUE(parent, sort_key)` when row *i*'s new key equals an
  un-rewritten row's old key). Jira rebalances at 128; we catch it at 64.
- **D3 — concurrency.** `UNIQUE(parent, sort_key)` turns race losers into `1062` → retry.
  `is_transient_lock_error` (currently private in `cards/routes.rs`) moves to `common/` and the
  matcher becomes `1213 | 1205` + **`1062` scoped to the `UQ_*_sort_key` constraint names**
  (match the `for key '…'` suffix in the MySQL error message; unrelated unique violations —
  e.g. `card_labels`, `UQ_labels_user_name` — are not retried). If scoping proves unworkable in
  `AppError`, fall back to workspace-wide `1062` retry and accept (and record) pointless
  retries on permanent duplicates. Lock only the moving row (`SELECT … FOR UPDATE WHERE id = ?`)
  preserving same-card-race `Restart`; neighbor reads stay unlocked (last-write-wins for
  concurrent same-gap drops — acceptable, per Figma/Rezee); append = `ORDER BY sort_key DESC
  LIMIT 1 FOR UPDATE` — a single-index-record lock (InnoDB locking reads lock the index records
  the scan encounters; `LIMIT 1` on the unique composite index encounters one. This is *not*
  meaningfully narrower than today's `MAX(position) FOR UPDATE`, which is also effectively
  single-record — the win is single-row writes with zero sibling rewrites, not lock width);
  clamp upper bound from plain `COUNT(*)`. Columns and checklist-item append paths keep their
  current serialization semantics: serialized last-key `FOR UPDATE` + INSERT in one tx (no new
  `1062` path for them).
- **D4 — one migration, ordered statements:** add `sort_key` (DEFAULT `''`) → backfill
  (`ROW_NUMBER() OVER (PARTITION BY <parent> ORDER BY position, id)` → `CONCAT('c', ELT×3
  base62)` mapping row 1 → value 0, i.e. first key per parent is `c000`; head `c` + 3 base62
  digits is a valid fractional key — verified 2026-11-15 against the library source:
  `get_int_len('c') == 4` — and monotonic for up to 62³ = 238,328 rows per parent) → `UNIQUE`
  constraints `UQ_columns_board_sort_key(columns.board_id, sort_key)`,
  `UQ_cards_column_sort_key(cards.column_id, sort_key)`,
  `UQ_checklist_items_checklist_sort_key(checklist_items.checklist_id, sort_key)` → drop
  `position` + old indexes. Single release, seconds of downtime (default — see open call 1).
  `ROW_NUMBER() … PARTITION BY` and `ELT` verified supported on MySQL 8.4 LTS and MariaDB 11.4
  LTS (MariaDB docs, row_number.md / elt.md).
- **D5 — behaviour normalizations recorded in `CONTRACT-CHANGES.md`:** checklist-item
  positions become contiguous computed indexes (were verbatim-with-holes); duplicate positions
  become impossible; deletes never renumber siblings anywhere; all order queries gain `, id`
  tiebreakers.

### Review notes (2026-11-15)

Independent verification pass (codebase audit + web/source research; corrections above merged
into the plan):

- **`lexicon_fractional_index` health** (crates.io / docs.rs, read 2026-11-15): 0.0.3 MIT, 284
  LOC, single maintainer, last publish 2022-01-17. Algorithm byte-identical to rocicorp
  `fractional-indexing` source: same `BASE62_DIGITS`, head rules (`get_int_len`: `a-z` →
  `head-'a'+2`, `A-Z` → `'Z'-head+2`), same `validate_order_key` quirks. `n_keys_between` exists
  (recursive midpoint split, even distribution). Decision: vendor with attribution; crate is a
  fixture, not a dependency. Freshly verified that head `c` + 3 base62 digits is a valid
  3-digit integer part (not `c` = length 3 — head counts include the head char).
- **D4 backfill under-partitioned** → fixed: every `ROW_NUMBER()` is `PARTITION BY` the parent.
  Without it, uniqueness held globally but the "62³ rows **per parent**" bound silently
  degraded to per-table and the monotonicity claim relied on incidental `(position, id)`
  ordering. Also: `ELT` returns NULL out-of-range → NOT NULL abort → loud failure, never
  silent corruption (still true, and now per-parent bounded).
- **Round-trip interop test was missing** → added (T010a/T010b): SQL-generated keys must
  parse and compare correctly under the vendored key logic — the one thing the backfill must
  never break.
- **`ascii_bin` collation confirmed** on both targets (MariaDB 11.4 `SHOW COLLATION`: id 65,
  `sortlen 1`, compiled; MySQL 8.4 native). Byte order `0-9 < A-Z < a-z` == `BASE62_DIGITS`
  order — case-sensitive keys order correctly. Any `*_ci` collation would silently reorder.
- **InnoDB locking reads** lock the index records the scan encounters (MySQL 8.4 manual
  §17.7.2.4); `… LIMIT 1` on a unique composite index encounters one record, so D3's "narrower
  than `MAX(…) FOR UPDATE`" claim was overstated — both are effectively single-record. Kept the
  design (single-row writes), corrected the rationale.
- **CI `backend-mariadb` leg ran only fmt + clippy + `migrate run` + online `cargo check` +**
  `sql_portability` + `auth_session` + session-store — the ordering suites never ran on
  MariaDB. Leg pins `mariadb:10.6`, which is **EOL since 2026-07-06**. Fixed: leg bumped to
  `mariadb:11.4` and extended to run the new suites; alternatively scope the MariaDB CI AC to
  `migrate run` + `sql_portability` (explicit decision, recorded in §Notes if split).
- **`1062` retry matcher was workspace-wide by default** → scoped to `UQ_*_sort_key`
  constraint names (the MySQL error message carries the key name; keep matching by error
  number, extract the `for key` suffix).
- **Rebalance regeneration would have transiently violated the unique index** → two-phase
  (park at temporary keys, then assign).
- **Columns/checklist-item append paths were unspecified for concurrency** → keep today's
  serialized `last-key FOR UPDATE` contract; cards keep the `1062`-retry pattern (only them).
- **MariaDB 10.6 → 11.4 LTS** everywhere per AGENTS.md (MySQL 8.4 LTS / MariaDB 11.4 LTS;
  11.4 community support to 2029-05-29). Note: MariaDB 10.6 is EOL; no task owns the CI bump —
  folded into this task's file table; may split to a prerequisite chore.
- Not re-litigated (accepted as-is): drawdb/LexoRank/Figma/rezee research conclusions, wire
  contract (D1), clamp semantics default, single-release default. `T006` notes `caching_sha2_password`
  exists in MariaDB only from 11.4.9/11.8.4/12.1 — irrelevant to CI's native-auth root login.

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
- Adopting MariaDB 12.x (or MySQL 9.x) as a target — 11.4 LTS is the floor per AGENTS.md.
- Retrying `1062` for constraints other than `UQ_*_sort_key` (matcher stays scoped).
- Splitting the `mariadb:11.4` CI bump into its own task — if done, scope the MariaDB CI AC
  to `migrate run` + `sql_portability` explicitly.
