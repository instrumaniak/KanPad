# T026: Capacity smoke + headroom doc

> **STATUS: pending.** Wave 1. Closes **S3** (MEDIUM). **Depends on T020, T021, T022**
> — measure *after* the fixes, not before.

## Goal

Replace "no capacity model anywhere" with **measured numbers** for the 100+ user target
(D-2): a reproducible load profile against the compose stack (T012), recorded results,
and the tuning knobs that mattered — so future "should we scale?" questions have data.

1. **Load script** committed (k6 preferred — single binary; fallback `oha`/`wrk` if
   unavailable): seeded DB (T07 fixture pattern) with N boards × M cards; ramp to
   **100+ concurrent users**: pre-seeded sessions (login storm respects the 5/min auth
   throttle — document the raise for test env only), board loads, card moves, notes
   lists.
2. **Measure:** p50/p95/p99 vs NFRs (page <2 s, board <1 s, API <500 ms, drag
   <100 ms), pool acquire wait (T024's histogram), 429/503 counts, DB CPU, and the
   pool/timeout interplay (10 conns × 30 s timeout starvation scenario — P7).
3. **Output:** `docs/capacity-headroom.md` — measured ceiling on the target box, the
   pool sizing rule (`replicas × DB_MAX_CONNECTIONS ≤ max_connections − margin`, minus
   one sweeper connection per instance), tuning knobs that moved the needle, and the
   trigger points for: keyset pagination (T022 open call), FTS (T023 gate), S2
   rate-limit scale-out.

## Depends On

T020, T021, T022 (and practically T012 for the compose substrate).

## Phase

1

## Critical

No — but it's the evidence layer for S3 and gates T023's decision.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Capacity smoke (design: tool,
  scenario, measures, edge cases: throttle skew, Argon2 CPU-bound by design) +
  §Deferred (S2/FTS/keyset triggers this doc feeds).
- `specs/arch-hardening-specs/00-overview.md` — finding S3, P7 interplay; D-2; global
  DoD.
- `backend-rs/.env.example` (pool/throttle knobs), `backend-rs/src/db.rs:48-59` (pool
  options), `backend-rs/src/lib.rs:72` (30 s timeout), NFR targets in
  `specs/planning-artifacts/architecture.md` (NFR1–NFR5),
  `specs/api-rewrite-rs-specs/53-test-optimization.md` (test DB patterns).

## Files to Create/Modify

| File | Change |
|---|---|
| `scripts/load/board-smoke.js` (k6) or `scripts/load/README` + script | **NEW** — the scenario (ramp, endpoints, assertions on latency thresholds) |
| `docs/capacity-headroom.md` | **NEW** — methodology, environment, results tables, ceiling, sizing rule, tuning knobs, revisit triggers |
| `specs/arch-hardening-specs/00-overview.md` | Annotate S3 as closed + record the trigger numbers (or note in task §Notes if spec annotation deferred) |
| `docs/lessons/ci-quality.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** production data or runs against prod (compose only),
permanent throttle changes (test-env-only, documented), app code (this is measurement —
fixes belong to their own tasks).

## Implementation Steps

1. Prereqs: T012 compose up; seed via T07-style fixtures at scale (write a seed script
   if fixtures are fixed-size — keep it test-only).
2. Tool pick (verify availability): k6 → else oha/wrk; justify in doc.
3. Scenario: ramp to 100+ VU over the mix above; auth sessions pre-created (document
   the test-env throttle raise); run 2–3 times for variance.
4. Capture: latency percentiles vs NFRs, error rates (429/503/5xx), pool acquire
   histogram (T024), DB CPU/memory, host memory.
5. Isolate the pool/timeout scenario: N concurrent slow queries → observe 11th-request
   503 (P7 interplay) — record as a known ceiling with the knob that relieves it
   (`DB_MAX_CONNECTIONS` vs DB `max_connections` headroom).
6. Write `docs/capacity-headroom.md` (results + sizing rule + triggers).
7. Annotate S3 closed; lesson.

## Constraints

- **Never run against production data/DB** (cost + safety); compose stack only.
- Test-env-only config deviations documented inline (throttle raise) — no permanent
  defaults changed without a task.
- Numbers must be reproducible: commit the script + exact seed + run commands.
- Argon2 login CPU cost is *by design* — measure and document, don't propose weakening
  `HASH_COST` (production floor `>= 2` is a security rule).
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Committed load script + seed instructions; run is reproducible (paste commands).
- [ ] Results: p50/p95/p99 vs NFR1–5 at 100+ VU; error counts; pool/DB observations
      (closes **S3** with evidence).
- [ ] Pool/timeout starvation scenario measured and documented with the relieving knob.
- [ ] `docs/capacity-headroom.md` includes: ceiling, sizing rule
      (`replicas × pool ≤ max_connections − margin − sweeper`), tuning knobs, revisit
      triggers for T022-keyset / T023-FTS / S2-scale-out.
- [ ] T023 decision informed (record go/defer in T023 §Notes); lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
