# Performance runbook

## Drag profile (F1)
- Playwright perf specs tagged `@perf`. Run locally: `npx playwright test --grep @perf`.
- CI: `playwright test --grep @perf --workers=1 --retries=0`.
- Throttle via CDP: `Emulation.setCPUThrottlingRate` 4x, try/finally restore.
- Paced pointer input: single-step `mouse.move` interleaved with awaited rAF.
- Warm-up: 1 discarded + 3 measured runs → median. `PERF_BUDGET_SCALE` env var (CI sets measured value; local default 1).

## Column scroll profile (F5, statistical)
- Spec: `frontend/e2e/perf-scroll.spec.ts` (`@perf`). Measures column scroll frame delta on a dense virtualized column.
- Run: `cd frontend && npx playwright test e2e/perf-scroll.spec.ts --workers=1 --retries=0`
- Paced scroll: hover column `[data-testid="column-card-list-scroll"]`, `mouse.wheel` in small deltas interleaved with `requestAnimationFrame`. Setup/reset scroll excluded from measured window.
- Frame metrics are Tier B/statistical; render counts from `window.__PERF__` are deterministic dev-mode signals when present. Use same machine/profile for before/after comparison.

## Frontend profiler
- `window.__PERF__` store (timings/ids only). Dev-gated (`import.meta.env.DEV`).
- Verify absence in prod: `grep -R "__PERF__" frontend/dist` must be empty.
- Playwright probe (dev mode): `page.evaluate(() => window.__PERF__?.reset())` before a scripted interaction, `page.evaluate(() => window.__PERF__?.snapshot())` after. Helpers: `resetPerfInPage`/`snapshotPerfInPage`/`perfCountsById` in `frontend/e2e/perf-helpers.ts`. Render counts are deterministic; durations are environment-sensitive.

## Criterion (local/manual)
```
cd backend-rs && cargo bench --bench order_key
```

## Gungraun (CI gate)
```
cd backend-rs && cargo bench --bench order_key_callgrind
# compare: cargo bench --bench order_key_callgrind -- --baseline=<name> --callgrind-limits='ir=0%'
# re-baseline: --save-baseline=<name> (same PR, with justification)
```
Requires valgrind + gungraun-runner. Counts only comparable on identical toolchain+valgrind+runner+arch. `Ir` blind to cache locality.

### Baselines (name `main`)
- Committed under `backend-rs/benches/baselines/` (gungraun tree with `@main`
  files). CI sets `GUNGRAUN_HOME=$GITHUB_WORKSPACE/backend-rs/benches/baselines`.
- Recorded Ir (2026-11-20): `bench_key_between_middle` 416394,
  `bench_keys_between_ten` 441852, `bench_needs_rebalance` 414350.
- Pinned env (assert in `.github/workflows/backend-rs.yml`): runner
  **`ubuntu-26.04`** + valgrind **3.26.0** (`valgrind --version | grep -q
  'valgrind-3.26.0'`) + gungraun-runner **0.20.0** + toolchain **1.99.1**
  (`backend-rs/rust-toolchain.toml`).
  Bump → re-baseline with `--save-baseline=main` in the same PR + record the
  new Ir values here (runbook + `perf-budgets.md` + workflow assert move
  together).
- `EntryPoint::None` in `order_key_callgrind.rs`: gungraun's default
  `--toggle-collect='*::__gungraun_wrapper_mod::*'` (DEFAULT_TOGGLE in
  gungraun-runner 0.20) never fires on this toolchain (callgrind records 0 Ir
  even with the exact demangled name) — with no toggle, callgrind collects
  from program start.
- Red-proof (2026-11-20): artificial spin loop in `bench_key_between_middle`
  → +48.03% Ir (416394 → 616397) → `Gungraun result: Regressed`; loop
  reverted, gate green again.

## k6
See scripts/load/README.md.

## EXPLAIN dump
Plan guards in `backend-rs/tests/api/query_plans.rs` (shape only, no cost).

## Re-baseline procedure
On toolchain/valgrind/runner bump: re-record baseline with `--save-baseline`, document justification in PR.
