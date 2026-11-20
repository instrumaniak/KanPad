# Performance budgets registry

Single source of truth for every perf budget: value, tier, enforcing test,
last-measured date, NFR mapping (or explicit dev-mode tripwire label).

Freshness rule: any threshold change must update `last-measured` in the same PR.

## Registry

| # | Name | Value | Tier | Enforcing test | Last measured | NFR mapping |
|---|---|---|---|---|---|---|
| B0 | Bundle initial JS (gzip) | 267768 B (budget 310000 B) | A | `npm run budget` | 2026-11-21 (local build) | NFR1 (indirect) |
| F1 | Drag frame delta (maxDt, median) | cross 250.6ms / same 290.8ms / throttled-4x 1254.2ms (budget 3500*scale) | B | `perf-drag.spec.ts` | 2026-11-21 (local compose `localhost:5173`, @perf 5/5 pass) | NFR3 drag <100ms (dev-mode tripwire harness) |
| F1 | Input latency p95 (all-frame event probe) | eventP95 cross 13.5ms / same 13.5ms / throttled-4x 12.6ms (budget 50*scale); LoAF worst-case logged informationally | B | `perf-drag.spec.ts` | 2026-11-21 (local compose `localhost:5173`, @perf 5/5 pass) | NFR3 (all-frame probe; LoAF worst-case informational) |
| F2 | navToNetworkIdleMs (dev-mode) | 1068ms measured (budget 3000*scale) | B | `perf-load-heap.spec.ts` | 2026-11-21 (local compose `localhost:5173`, @perf) | dev-mode tripwire — NOT NFR-enforcing (NFR2 deferred) |
| F3 | Heap growth delta (10 cycles) | -0.48MB measured (budget +5MB*scale) | B | `perf-load-heap.spec.ts` | 2026-11-21 (local compose `localhost:5173`, @perf) | dev-mode tripwire |
| F4 | BoardView render count | <=2 (jsdom) | A | `render-budget.test.tsx` | 2026-11-21 (local vitest 878/878) | identity-stability (uncompiled React) |
| F4 | ColumnCardList render count | <=2 (jsdom) | A | `render-budget.test.tsx` | 2026-11-21 (local vitest 878/878) | identity-stability (uncompiled React) |
| B1 | order_key Ir (gungraun) | committed baseline | A | `order_key_callgrind` | 2026-11-21 local pass (no change; valgrind 3.18.1, runner 0.20.0); 2026-11-20 baseline Ir: key_between 416394, keys_between_ten 441852, needs_rebalance 414350 | T010 sort-key math |
| B2 | EXPLAIN plan shape (no ALL/index, key required) | index used | A | `query_plans` | 2026-11-21 (local runtime pass: MySQL 8.4 testdb `127.0.0.1:3337`, 4/4; CI covers both DB legs) | hot SQL |
| NFR1 | page <2s | deferred/unenforced | — | none | deferred/unenforced | deferred (prod load timing) |
| NFR2 | board <1s | deferred/unenforced | — | none | deferred/unenforced | deferred (prod load timing) |
