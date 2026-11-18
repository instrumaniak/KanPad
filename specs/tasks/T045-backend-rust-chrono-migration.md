# T045: backend-rust chrono migration

> **STATUS: pending.** Unblocks all backend CI: `cargo deny check advisories`
> fails on every branch since RUSTSEC-2026-0543 (`chrono is unmaintained`,
> no safe upgrade) began firing on 2026-11-17. The CI remediation merge adds a
> bound ignore for RUSTSEC-2026-0543 to `deny.toml` whose removal condition is
> **this task** — landing this task means deleting that ignore.

## Goal

Remove `chrono` from the dependency tree entirely by migrating `backend-rs`
to the `time` crate (recommended replacement per the advisory; jiff has no
sqlx integration and would need manual decode everywhere).

- sqlx feature `chrono` → `time` (sqlx 0.9 supports both; when both are
  enabled `time` wins for `query!` — which is exactly why the in-tree session
  store needed its `to_naive_utc` bridge and the old `AS "col:
  chrono::NaiveDateTime"` overrides). After the swap there is exactly one
  datetime crate in the tree.
- MySQL mapping after the swap: `DATETIME(6)` ↔ `time::PrimitiveDateTime`,
  `TIMESTAMP(6)` ↔ `time::OffsetDateTime`.
- **The wire format is frozen.** Every timestamp reaches the API as
  `format_timestamp(...)` output — `YYYY-MM-DDTHH:MM:SS.ffffffZ`
  (micros always present, trailing `Z`). Timestamps are `String` in the
  `ToSchema` derives, so `openapi.json`, `schema.d.ts`, and the frontend are
  untouched by this task.
- **No database migration.** Column types are unchanged; only the Rust-side
  decode/encode types change.
- `deny.toml`: delete the RUSTSEC-2026-0543 ignore (its removal condition is
  this task). Re-run the full `cargo deny check`.

Verification of reachability (2026-11-19): ~53 `chrono` references in
`src/` across `common/{time,session_store,validation,card_summary}.rs` and
`features/{boards,cards,checklists,columns,labels,notes,projects,tags}/{store,types}.rs`
plus `cards/routes.rs:216`, and 4 references in
`tests/api/sql_portability.rs` (`chrono::Utc::now().naive_utc()` binds).

## Depends On

None technically. The `deny.toml` ignore added by the CI-remediation merge
is the removal target; it must be in place (or this lands in one shot with
it).

## Phase

1

## Critical

No — but until this lands (or the bound ignore does), backend CI is red on
every branch including main.

## Spec References

- `backend-rs/deny.toml` — `[advisories] ignore` (RUSTSEC-2026-0543 entry;
  removal condition = this task) and the file's own rule that the allow-list
  must be **re-derived from a fresh `cargo deny check` after any dependency
  change**.
- RUSTSEC-2026-0543 — chrono unmaintained, all versions; recommended
  successors: `jiff`, `time`.
- sqlx 0.9 docs (Context7 `/transact-rs/sqlx/v0.9.0`, 2026-11-19): `time`
  cargo feature = "date and time types from the `time` crate (alternative to
  `chrono`)"; `time` is **preferred over `chrono` by the `query!` macros
  when both are enabled**; `sqlx.toml` `preferred-crates` is NOT needed once
  chrono is fully removed.
- `backend-rs/src/common/session_store.rs` — module docs explain the
  `time` → `chrono` bridge this task deletes (`to_naive_utc`).
- `backend-rs/src/common/time.rs` — `format_timestamp`, the single choke
  point for the wire format.
- `specs/tasks/status.yaml` — T045 entry already recorded (phase 0, pending).
- `docs/lessons/rust-backend.md` / `docs/lessons/ci-quality.md` — lesson
  targets.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/Cargo.toml` | sqlx `chrono` → `time`; `time = { version = "0.3", features = ["parsing", "formatting", "macros"] }` explicit; delete `chrono = "0.4"` |
| `backend-rs/Cargo.lock` | Regenerated; `chrono` (and now-unused transitive deps) gone |
| `backend-rs/deny.toml` | **Delete** the RUSTSEC-2026-0543 ignore; re-derive `check` output |
| `src/common/time.rs` | `format_timestamp(&time::PrimitiveDateTime)`; fixed-micros shape via `time::format_description!` (`[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z`); unit-test fixture rebuilt with `time` constructors |
| `src/common/session_store.rs` | Delete `to_naive_utc` + chrono imports; bind `record.expiry_date` (OffsetDateTime) directly in the upsert; `expiry_date > ?` binds `OffsetDateTime::now_utc()`; test fixtures (`chrono::TimeZone` etc.) move to `time` |
| `tests/api/sql_portability.rs` | 4 binds: `OffsetDateTime::now_utc()` |
| `src/common/validation.rs` | `validate_due_date`: RFC 3339 via `OffsetDateTime::parse(&Rfc3339)`; `YYYY-MM-DD` via `time::Date::parse` + `Time::MIDNIGHT`; naive `YYYY-MM-DDTHH:MM:SS[.f]` via `PrimitiveDateTime::parse` — identical accept/reject behavior |
| `src/features/cards/types.rs` | `parse_due_date` → time-based; same three accepted shapes; same `Option<PrimitiveDateTime>` semantics |
| `src/features/cards/routes.rs` | `:216` `Option<Option<chrono::NaiveDateTime>>` → `PrimitiveDateTime` |
| `src/common/card_summary.rs` | Struct fields + the three `AS "…: chrono::NaiveDateTime"` builder overrides → `time::PrimitiveDateTime` (or drop the overrides if `build_query_as` decodes type-driven — verify with a live run; keep whichever makes `FromRow` pass) |
| `src/features/{boards,cards,checklists,columns,labels,notes,projects,tags}/store.rs` | `use chrono::NaiveDateTime` → `use time::PrimitiveDateTime`; struct fields; `FromRow` derives unchanged |
| `src/features/{boards,projects}/types.rs` | Unit-test fixtures: `NaiveDateTime::parse_from_str(…)` → `PrimitiveDateTime::parse` with an equivalent format description (note the fixtures use a **space** separator `"%Y-%m-%d %H:%M:%S%.f"` — preserve the same literal or move to the canonical `T` shape consistently) |
| `.sqlx/` / CI `sqlx prepare --check` (no file change) | query! output types change (DATETIME→PrimitiveDateTime) — re-run `cargo sqlx prepare --check --workspace --all -- --locked --all-targets` against a scratch MySQL |
| `docs/lessons/rust-backend.md` (or `ci-quality.md`) | Dated, tagged lesson: advisory out-of-band breakage; the choke-point design (`format_timestamp`) is why the wire stayed stable |
| `specs/tasks/status.yaml` | T045 → `done` on completion |

**Must NOT appear in the diff:** any change to `openapi.json` (prove with
`git diff --exit-code backend-rs/openapi.json`), `schema.d.ts`, or anything in
`frontend/`; any DB migration; any `.up.sql`/`.down.sql`; seconds-precision
truncation; re-adding `chrono` "just for tests"; Tz awareness.

## Implementation Steps

TDD-ish: keep the wire-shape unit test red→green first, then convert module by module.

1. **Baseline.** `DATABASE_URL=<scratch MySQL> cargo deny check advisories` —
   reproduce the failure locally. Confirm no OTHER advisories fire (if the
   fresh run names anything else, handle in the same PR or bound it — do not
   land a deny.toml that is red on arrival).
2. **Manifest.** Cargo.toml edits above; `cargo update`/`cargo check` against
   the scratch MySQL (sqlx macros need `DATABASE_URL` at compile time; there
   is no `.sqlx/` offline cache in this repo and no `sqlx.toml` — do not
   invent one).
3. **Wire choke point first.** `common/time.rs`: rewrite `format_timestamp`
   and its unit test. Red → green. The assertion
   `"2026-10-11T12:34:56.123456Z"` must pass unchanged — this is the contract.
4. Session store: delete `to_naive_utc`; bind `OffsetDateTime` directly
   (this is what the original `tower-sessions-sqlx-store` did); update the
   module docs that explain the old bridge — they describe history, rewrite
   them to describe the present. Test fixtures move to `time` (`Record`
   `expiry_date` is already `OffsetDateTime`). sql_portability.rs binds change
   the same way.
5. Parsing paths: `validation.rs::validate_due_date`,
   `cards/types.rs::parse_due_date`, `cards/routes.rs`. Same three accepted
   shapes, same UTC-naive results, same rejects. Port the unit tests; add an
   RFC 3339 with non-UTC offset case if missing (must land as UTC-naive).
6. Stores & row structs: mechanical `NaiveDateTime` → `PrimitiveDateTime`.
   `card_summary.rs` overrides → `time::PrimitiveDateTime`; verify the builder
   decodes against a live scratch DB.
7. **Delete the deny.toml ignore.** `cargo deny check` (all checks) green.
8. Full gate: `fmt --check`, `clippy --workspace --all-targets -- -D
   warnings`, `cargo sqlx prepare --check …`, `cargo llvm-cov --workspace
   --all-targets --fail-under-lines 85`, openapi snapshot diff = empty.
9. CI on the merge ref must be green on **both** engines (gnu/mysql leg and
   the mariadb leg). Lessons; `status.yaml`; **no commit without approval.**

## Constraints

- Wire format frozen (`format_timestamp` output shape is the contract; utoipa
  schemas unchanged).
- No schema/migration changes; `sessions.expiry_date` stays `TIMESTAMP(6)`,
  all `created_at`/`updated_at` stay `DATETIME(6)`.
- Microsecond precision preserved everywhere; UTC only; no `Tz`.
- One datetime crate in the tree afterwards (`time`); `chrono` must not
  survive in Cargo.lock.
- MariaDB 11.4 + MySQL 8.4 both green (TIMESTAMP decode must behave
  identically).
- `sqlx prepare --check` is part of done; a green local `cargo test` alone is
  insufficient.
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] `grep -ri chrono backend-rs/ --include='*.toml' --include='*.rs'` →
      zero hits (docs/comments included).
- [ ] `cargo deny check advisories` passes **without** the
      RUSTSEC-2026-0543 ignore (it is deleted, not aged out).
- [ ] Wire contract: `format_timestamp` unit test asserts
      `2026-10-11T12:34:56.123456Z`; `git diff --exit-code backend-rs/openapi.json`
      is empty; frontend untouched.
- [ ] DATETIME ↔ `PrimitiveDateTime`, TIMESTAMP ↔ `OffsetDateTime` decode
      verified by the existing suite on both MySQL 8.4 and MariaDB 11.4
      (incl. `sql_portability` session upsert).
- [ ] `validate_due_date`/`parse_due_date` accept exactly the same inputs as
      before (RFC 3339 with offset → UTC-naive; `YYYY-MM-DD` → midnight UTC;
      naive `YYYY-MM-DDTHH:MM:SS[.f]`).
- [ ] `sqlx prepare --check`, clippy `-D warnings`, fmt,
      `cargo llvm-cov --fail-under-lines 85` all green.
- [ ] Lesson appended; `status.yaml` T045 → done.

## Notes

**Why `time`, not `jiff` (2026-11-19):** `time` 0.3.55 is already in the tree
(direct dep for `tower-sessions`' `OffsetDateTime`; `time-macros` present), and
sqlx 0.9 decodes `DATETIME`/`TIMESTAMP` natively with its `time` feature.
`jiff` would require hand-written decode/encode or `jiff-sqlx` (immature).
`time` is the advisory's own recommended successor.

**Why not just keep the ignore:** the ignore is a hole in the supply-chain
gate bound to this task; the file's header forbids stale ignores.

**Watch out:** with both `time` and `chrono` enabled sqlx prefers `time` —
do not land an intermediate state where both are enabled and the stores split
across two crates (that was the pre-session-store world this repo already
escaped once; the `AS "col: …"` overrides in `card_summary.rs` are the last
fossils of it).

**Local dev DB:** use `backend-rs/docker-compose.test.yml` scratch MySQL, not
the dev database; the e2e throwaway schema is fine too if isolated.

**CI context (2026-11-19):** the failure was not caused by any code change —
main, dev and both dependabot PRs went red simultaneously when the advisory
landed. The frontend axe failure (markdown-preview link contrast) was
introduced by 936513b3 and is fixed separately in the same remediation
(light-theme `--label-blue` → `#2563EB`).

## Lessons

(pending — record: RUSTSEC-2026-0543 arrived out-of-band and red-lighted
main + dev + two dependabot PRs simultaneously; a green-yesterday CI is not
evidence of a green-today one. The `format_timestamp` choke point is what
made this migration wire-invisible.)
