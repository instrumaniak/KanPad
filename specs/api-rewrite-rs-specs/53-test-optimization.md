# 53 — Test Suite Performance Optimization

> **Source of truth:** `50-tasks.md` conventions (one task = one PR, TDD, evidence checklist, ≤5 files)
> **Current state:** 21 `#[sqlx::test]` functions (DB-touching) + 55 `#[tokio::test]` functions (DB-free), 2 migrations
> **Target:** establish measured baselines first; then set runtime and compile-time targets based on achievable, repeatable improvements.

---

## Problem Analysis

| Metric | Current | Target | Root Cause |
|--------|---------|--------|------------|
| `#[sqlx::test]` suite runtime | TBD (measure baseline) | Set after baseline | Per-test isolated DB setup + migrations; actual cost unmeasured |
| Compile (no-op and clean) | TBD (measure baseline) | Set after baseline | Repeated migration metadata may contribute; quantify before claiming impact |
| CI test stage | TBD (measure baseline) | Set after comparison | Compare current `cargo test` with nextest using equivalent targets and environments |
| MySQL test connections | TBD (measure or derive from runner) | No tuning target yet | Pool maxima are upper bounds; actual concurrent connections depend on scheduling and test behavior |

**Key findings from research (investigated against actual codebase + web):**

- **Test inventory correction:** The codebase has **21 `#[sqlx::test]`** functions across 21 test modules (20 suite files × 1 + `helpers.rs` × 1). The previous estimate of 79 was an overcount. The remaining 55 test functions use `#[tokio::test]` and are **DB-free** (probe routes, CORS matrices, validation, throttling, spec comparison, SPA file I/O). They boot the app in-process via `tower::ServiceExt::oneshot` with no database connection. These must NOT be bundled into `#[sqlx::test]` suites — doing so would add unnecessary `CREATE DATABASE` + migration + teardown overhead to tests that currently need none.

- **Migrations:** The project has **2 migrations** (`0001_baseline.sql` ~220 lines, `0002_convert_to_utf8mb4.sql` ~35 lines). A shared `Migrator` is a plausible compile-time/binary-size optimization, but its effect must be measured. It does **not** avoid running migrations against each isolated test database, so it is not expected by itself to reduce DB suite runtime.

- **sqlx 0.8.6 connection management:** The [MySQL testing source](https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-mysql/src/testing/mod.rs) configures a shared test-management pool and per-test pools. Pool maxima are not a measured peak for this suite. Record observed MySQL connections and runner concurrency before changing either pool size or server limits. In particular:
  - A **shared master pool** with `max_connections(20)` handles DB creation/migration/cleanup; connections are closed immediately after use (`after_release → Ok(false)`)
  - Each test's pool (default `Pool` signature) gets `max_connections(5)` + `idle_timeout(1s)`
  - With four concurrently active tests, the configured upper bound is 20 test connections plus the shared management pool's limit; this is not evidence that 40 connections are reached in practice.
  - sqlx 0.8.6 **already includes** the nextest cleanup race fix (PR #3334) — `NEXTEST_RUN_ID` coordination
  - A lower per-test pool maximum is an option only if observed connection pressure or timeouts justify it; do not infer the need from configured maxima alone.

- **Shared migrator:** SQLx documents a library `static MIGRATOR` referenced by integration tests through the library crate name. These tests are modules under the `api` integration-test crate, so `crate::MIGRATOR` would resolve in the wrong crate; use `backend_rs::MIGRATOR` (or import it and use the imported name). This can reduce repeated migration macro expansion, but benchmark compile time and inspect generated output before describing the size of the win. `main.rs` has two additional macro call sites; changing them is optional and unrelated to the test optimization. Note: `helpers.rs` contains 3 occurrences of `#[sqlx::test` in comments/documentation but only 1 actual test function — the total of 21 is correct.

- **Migration change detection:** Per sqlx issue #4267, on stable Rust, migration file changes may not trigger recompilation (especially with sccache). A `build.rs` with `cargo:rerun-if-changed=migrations` is recommended.

---

## Implementation: Sub-tasks

> **Splitting guidance:** Each sub-task = one PR per `50-tasks.md` (≤5 files). Split T53a into reviewable batches if needed; do not combine migrator, pool, and CI runner changes into one PR. T53b remains a separate deferred decision.

### T53a — Shared Migrator + Static Reference (Core, low-risk)

**Goal:** Reference one library-owned migrator from SQLx integration tests and measure any compile-time or binary-size improvement. This change does not remove per-database migration execution.

**Files:** `backend-rs/src/lib.rs`, `backend-rs/build.rs` (new, only if needed for migration change tracking), and the 21 DB-backed suite modules. Do not change `main.rs` for this test-focused task.

```rust
// backend-rs/src/lib.rs
/// Shared migrator: one `static` instance embedded at compile time, referenced
// by all `#[sqlx::test(migrator = ...)]` invocations.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
```

**build.rs** (new file — migration change detection):

```rust
fn main() {
    // On stable Rust, migration file changes may not trigger recompilation
    // of the crate (sqlx issue #4267). This ensures correctness when editing
    // migration files without touching Rust source.
    println!("cargo:rerun-if-changed=migrations");
}
```

**All 21 `#[sqlx::test]` tests change:**

```rust
// Before:
#[sqlx::test(migrations = "./migrations")]
async fn my_test(pool: MySqlPool) { ... }

// After:
#[sqlx::test(migrator = "backend_rs::MIGRATOR")]
async fn my_test(pool: MySqlPool) { ... }
```

> **Decision:** Keep the `pool: MySqlPool` signature for this task. Do not change pool limits unless measurements show connection exhaustion or timeouts; if they do, evaluate a narrow pool/concurrency adjustment independently.

**Expected files (split into batches to honor the ≤5-file task convention):**
- `backend-rs/src/lib.rs` (add `pub static MIGRATOR`)
- 21 DB-backed modules under `backend-rs/tests/api/` (change the migration attribute to `migrator = "backend_rs::MIGRATOR"`)
- `backend-rs/build.rs` (new — required for migration change detection per sqlx issue #4267)

**AC:**
- [ ] `cargo fmt --check` ✅
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` ✅
- [ ] `cargo test -p backend-rs --test api` (21 DB tests + 55 DB-free tests all green) ✅
- [ ] `cargo sqlx prepare --check --workspace --all -- --all-targets` ✅
- [ ] Compare `cargo expand --test api` output before/after and confirm migration metadata is no longer repeated per test.
- [ ] Record warm no-op and clean `cargo test --workspace --all-targets --no-run` timings before/after; report the measured delta without presuming an improvement.
- [ ] Confirm SQLx test behavior still creates isolated databases and runs migrations for each DB-backed test.

---

### T53b — Consolidate DB Test Suites (Optional, medium-risk) — DEFER

**Decision: DEFER.** Consolidation could reduce database setup count, but the current suite has 21 DB-backed tests and no measured per-test setup cost. Do not use the previous unmeasured estimate of saved seconds as justification. Reconsider if profiling shows database creation/migration dominates total test time or the suite grows substantially. Any future design must preserve isolation and avoid cross-test interference through shared session storage.

> `10-architecture.md` line 84: "resource suites share one DB + transaction rollback (per-test `CREATE DATABASE` on MySQL is minutes at ~150 tests — see T07)."

**If consolidation is pursued later, scope is:**
- Merge ONLY same-feature DB test files (e.g., `boards_create.rs` + `boards_list.rs` + `boards_state.rs` → one `boards_crud` suite)
- Never merge DB-free `#[tokio::test]` files into `#[sqlx::test]` suites
- Use the existing pattern: single `#[sqlx::test]` function with sequentially-asserted scenarios (already the design)
- Transaction rollback between sub-tests is the pattern per `10-architecture.md`, but the existing single-test-per-file design already achieves isolation via per-test DB creation

---

### T53c — CI Pipeline: cargo-nextest + `.config/nextest.toml`

**Goal:** Evaluate nextest against the current `cargo test` command for elapsed time, reliability, and reporting. Adopt it in CI only if the comparison demonstrates a useful improvement and equivalent test coverage. Nextest uses CPU-based default concurrency and runs tests in separate processes; it may increase simultaneous DB setup and is not inherently faster for this workload.

**Files:** `.config/nextest.toml` (new), `backend-rs/.github/workflows/ci.yml`

**`.config/nextest.toml`:**

```toml
[profile.default]
# Fail-fast off so you see all results in one run
fail-fast = false
# Flag tests that take longer than this
slow-timeout = "30s"

# CI profile: retries for flaky DB tests, JUnit output
[profile.ci]
fail-fast = false
retries = 1
slow-timeout = { period = "30s", terminate-after = 2 }
[profile.ci.junit]
path = "junit.xml"
report-skipped = "all"
```

**CI workflow changes** (new file `backend-rs/.github/workflows/ci.yml` — does not exist yet):

```yaml
# After "Install cargo-nextest" step:
- name: Install cargo-nextest
  uses: taiki-e/install-action@cargo-nextest

# Replace the test step:
- name: cargo nextest run --workspace --all-targets --profile ci
  working-directory: backend-rs
  run: cargo nextest run --workspace --all-targets --profile ci
  timeout-minutes: 10

- name: Upload JUnit test results
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: nextest-junit
    path: backend-rs/target/nextest/ci/junit.xml
```

> **Note:** Benchmark with nextest defaults and observe MySQL connections and failures. If DB contention is demonstrated, use a nextest test group/concurrency limit for DB-backed tests; verify the filter selects exactly the intended tests with `cargo nextest list` before CI adoption. A CI retry can hide flakiness and lengthen runs, so keep retries only under an explicit reliability policy and report retried outcomes.
**AC:**
- [ ] Baseline and nextest comparison use equivalent targets, runner, DB service, and cache state.
- [ ] Adopt nextest in CI only if it improves the chosen metric without reducing coverage or reliability.
- [ ] If adopted, `.config/nextest.toml` and JUnit upload are committed/configured.
- [ ] Record `PoolTimedOut` failures and retries; do not treat retries alone as proof of reliability.

---

### T53d — MySQL Server Tuning for Tests (CI services)

**Goal:** No MySQL server tuning is planned by default. Consider a targeted change only after evidence shows server connection limits or durability settings are a material runtime bottleneck.

**Files:** `backend-rs/.github/workflows/ci.yml` (services.mysql options)

The CI already uses a GitHub Actions `services.mysql` block. No service configuration change is planned unless measurements identify a specific connection or durability bottleneck; if justified, benchmark one setting at a time and scope any durability relaxation to ephemeral test CI.

**AC (only if tuning is later justified):**
- [ ] Measurements identify the specific MySQL bottleneck before a setting is changed.
- [ ] Change one setting at a time and record runtime, connection peaks, and failure rate against baseline.
- [ ] Any relaxed durability setting is limited to ephemeral test CI and shown to improve the measured target.

---

### T53e — Migration Rollup (Optional, low-risk) — CONSIDER

**Goal:** Defer migration rollup unless profiling shows migration execution is a meaningful part of test runtime.

**Context:** With only 2 migrations, rolling them up is unlikely to be useful absent evidence that migration execution dominates. The second migration also serves the production charset conversion path and must remain valid for deployed databases.

**Option A — Keep as-is:** Retain both migrations unless measured runtime shows migration execution is worth addressing. Preserve the production charset-conversion path.

**Option B — Conditional skip:** Not part of this plan; avoid adding test-only migration behavior without a measured need and a safe production migration strategy.

**Decision:** Leave as-is. Re-evaluate only if profiling shows migration execution is a meaningful runtime cost; do not rewrite migration history as a speculative speed optimization.

---

## Evidence Checklist (per 00-overview.md Constitution)

> **Scope correction:** CI runs `cargo test --workspace --all-targets` (current) / `cargo nextest run --workspace --all-targets` (after T53c). The `--test api` filter only runs the integration test target, missing `#[cfg(test)]` unit tests in `src/`. Use `--workspace --all-targets` for equivalence.

| Check | Command | Expected |
|-------|---------|----------|
| Format | `cargo fmt --check` | Exit 0 |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| Tests | `cargo test --workspace --all-targets` | All workspace tests pass; record the test count from `cargo test --workspace --all-targets -- --list` rather than assuming the API integration-target count is the workspace total |
| SQLx | `cargo sqlx prepare --check --workspace --all -- --all-targets` | Exit 0 |
| Runtime baseline | `cargo test --workspace --all-targets` | Record total and DB-suite elapsed times before changes; keep environment and cache state consistent |
| Runtime comparison | Current `cargo test` and candidate `cargo nextest`, same runner and targets | Adopt nextest only if the selected metric improves without coverage or reliability regression |
| Compile | `time cargo test --workspace --all-targets --no-run` | Record warm no-op and clean build before/after; report actual delta |
| Code-gen | `cargo expand --test api 2>/dev/null | wc -c` | Compare output before/after; treat as supporting evidence, not a runtime result |

---

## Rollback Plan

If any change breaks test isolation:
1. **T53a (shared migrator):** Revert `#[sqlx::test(migrator = "backend_rs::MIGRATOR")]` → `#[sqlx::test(migrations = "./migrations")]`. The `pub static MIGRATOR` in `lib.rs` can stay (harmless). Remove `build.rs` if added and no longer needed for correct migration change tracking.
2. **T53c (nextest):** Revert CI step to `cargo test --workspace --all-targets`. Remove `.config/nextest.toml`.
3. **T53d (MySQL tuning):** Revert only the independently benchmarked service setting if it regresses performance or reliability.
4. **T53b (consolidation):** DEFERRED — no risk to roll back.

---

## Future: Template Database (sqlx 0.9+)

When `tower-sessions-sqlx-store` supports sqlx 0.9 (currently pinned to 0.8 per `10-architecture.md`):

```toml
# sqlx.toml
[macros.test]
use-template-db = true
```

Could remove some per-test database setup work if supported by the pinned SQLx and session-store versions. Revisit only after confirming version compatibility and measuring current setup cost; no speedup estimate is assumed here.

**Near-term alternative (already in-flight upstream):**
- **PR #4326** proposes `[migrate] default-migrator` configuration in `sqlx.toml` — if merged and available in the pinned version, it may reduce per-test attribute boilerplate. Verify upstream status before relying on it.
- **PR #4338** (WIP): new `#[sqlx::test]` architecture with pre-acquired `max_connections` tracked in the master database — improves nextest connection sharing. sqlx 0.8.6 already includes the cleanup race fix (PR #3334).

---

## Integration into 50-tasks.md

Add after T52 (Phase 6/7 boundary):

```markdown
## Phase 7 — Performance + Developer Experience (post-cutover, optional)

* **T53a Test Suite Performance: Shared Migrator.** Add `pub static MIGRATOR` to `lib.rs` (`sqlx::migrate!("./migrations")`); change DB-backed tests to `migrator = "backend_rs::MIGRATOR"`; add a migration change-tracking build script only if needed. No database runtime savings are assumed. Depends: T03, T07. Blocks: none. Split work to honor the ≤5-file task convention. AC: verify isolated DB/migration behavior and measure compile/codegen changes.
* **T53b Deferred:** Consider DB-suite consolidation only if profiling shows setup cost dominates or test count grows substantially; preserve isolation.
* **T53c Nextest evaluation.** Compare `cargo nextest` with current `cargo test` under equivalent conditions. Adopt in CI only if the selected performance/reliability metric improves without reduced coverage. Add configuration/reporting only if adopted.
* **T53d MySQL tuning evaluation.** Do not change service limits or durability settings without measured evidence of the corresponding bottleneck; benchmark changes independently if justified.
```

---

## Effort Estimate

| Sub-task | Hours | Risk | Notes |
|----------|-------|------|-------|
| T53a Shared migrator | TBD | Low | 21 DB-backed modules; split work to honor the ≤5-file task convention |
| T53b Consolidation | — | — | **DEFERRED** — not worth it at 21 tests |
| T53c CI/nextest evaluation | TBD | Low | Measure current and candidate runners before deciding on CI changes |
| T53d MySQL tuning evaluation | TBD | Low | No configuration change absent a measured bottleneck |
| T53e Migration rollup | — | — | Deferred unless profiling justifies it |
| **Total** | **TBD** | **TBD** | Establish baselines before estimating effort or savings |

---

## Success Criteria

- [ ] **Compile time**: baseline measured; `cargo expand --test api` output reduced (no per-test migration inlining)
- [ ] **CI**: Adopt nextest only after an equivalent-runner comparison supports the change; track failures/retries and preserve full target coverage
- [ ] **CI reporting**: Configure JUnit output if nextest is adopted
- [ ] **Correctness**: all workspace tests pass; the API integration target currently contains 76 tests (21 `#[sqlx::test]` + 55 `#[tokio::test]`), with unit tests counted separately
- [ ] **No behavior change**: API contracts, error shapes, pagination unchanged
- [ ] **DB-free tests untouched**: 55 API `#[tokio::test]` tests remain DB-free (not consolidated into `#[sqlx::test]`)
- [ ] **Migration assertions preserved**: every mutation re-read via GET or direct `SELECT`
