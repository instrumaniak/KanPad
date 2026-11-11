# T016: Fix bundle — route chunks + lazy mermaid + CI size budget

> **STATUS: pending.** Wave 0. Closes **P1** (HIGH), **P2** (HIGH), CI half of **C2**.

## Goal

The source is correctly lazy (9 `React.lazy` routes, lazy mermaid/markdown) but the
**production build defeats it**: `dist/` has 5 JS files, one dynamic import, and the
3.1 MB mermaid chunk (~815 KB gzip) is `modulepreload`ed + statically imported from the
entry → **~1.29 MB gzip initial JS on every cold load, including `/login`**.

1. **Diagnose** the rolldown `codeSplitting.groups` capture (suspect: the catch-all
   `app` group at `vite.config.ts:52-56` merging entry modules; mermaid group statically
   linked from the merged entry). Red-proof the current state first.
2. **Fix** the chunk config so route-level splitting survives and mermaid is reachable
   only via dynamic import. Target: **< 300 KB gzip initial**.
3. **CI budget gate:** script fails the build if initial gzip > 300 KB or any
   `modulepreload` chunk > 150 KB gzip; wire into the CI `web` job after `vite build`.
   Red-proof the gate.

## Depends On

(None. T017 depends on this — both edit the CI workflow.)

## Phase

0

## Critical

No — perf/UX, not correctness; but it's the biggest perceived-performance win available.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Bundle (P1/P2 design: diagnose step,
  groups fix, budget gate, edge cases).
- `specs/arch-hardening-specs/00-overview.md` — findings P1, P2, C2; global DoD.
- `frontend/vite.config.ts:30-61` (the groups config), `frontend/dist/` (measured
  evidence: 5 files, `index.html:24` modulepreload, static import in entry),
  `frontend/src/App.tsx:11-37` (lazy routes), `frontend/src/features/notes/mermaid-diagram.tsx:9-11`.
- CI: `.github/workflows/backend-rs.yml` `web` job.

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/vite.config.ts` | Fix `codeSplitting.groups` (verify semantics against current rolldown docs — do not guess); lower `chunkSizeWarningLimit` post-fix |
| `frontend/scripts/check-bundle-budget.mjs` | **NEW** — parse `dist/`, fail over budget (300 KB initial gzip; no modulepreload > 150 KB gzip) |
| `frontend/package.json` | `budget` script wired; (dep moves to `devDependencies` are **T018's**) |
| `.github/workflows/backend-rs.yml` | Run budget script after `vite build` in `web` job |
| Tests | Budget-script unit test (fixture over/under budget); build-output assertions as pasteable evidence |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** `package.json` dependency moves (T018), source lazy
imports (already correct), Playwright config (T017).

## Implementation Steps

1. **Red-proof current state:** run `npm run build`; paste `ls dist/assets`,
   `grep -c 'import(' dist/assets/index-*.js`, `grep modulepreload dist/index.html`,
   and gzip sizes (evidence = P1/P2 confirmed).
2. **Diagnose:** identify the static `entry → mermaid` edge (bundler artifact — grep
   says no source import). Use rolldown debug/`--sourcemap` or a chunk-visualizer step.
   Fix the groups: entry modules stay in entry; `React.lazy` boundaries survive;
   mermaid group only reachable dynamically.
3. **Rebuild:** assert per-route chunks exist (≥ several named chunks, entry no longer
   statically imports mermaid), `modulepreload` list excludes mermaid, initial gzip
   < 300 KB. Paste the numbers.
4. **Budget script + CI wiring:** red-proof by temporarily making mermaid eager →
   script exits 1 → revert.
5. Visual smoke: `/login` fast; notes page with a mermaid diagram still renders
   (lazy boundary + `LazyLoadBoundary` fallback).
6. Full vitest + `tsc --noEmit`; lesson.

## Constraints

- Mermaid must remain dynamically imported (behavior: notes diagrams render).
- Do not hand-tune away React Compiler or change `staleTime`/runtime behavior — this is
  a bundling fix only.
- Rolldown/rolldown-vite group semantics: **verify against current docs** (AGENTS.md:
  verify plan by documentation).
- Budget numbers must have headroom (300 KB ≈ 4× a lean app) so the gate doesn't
  become noise; justify in Notes if adjusted.
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] Red-proof of the defect pasted (current 5-file dist + modulepreload mermaid).
- [ ] Post-fix `dist/`: route-level chunks exist; entry does not statically import
      mermaid; `modulepreload` excludes mermaid; **initial gzip < 300 KB** (closes
      **P1**, **P2** — paste measured sizes).
- [ ] Notes mermaid diagram renders (manual/e2e smoke).
- [ ] Budget gate in CI, red-proofed (over-budget build fails with a clear message)
      (closes CI half of **C2**).
- [ ] `chunkSizeWarningLimit` lowered to reflect reality; full frontend suite +
      `tsc --noEmit` green; lesson appended.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
