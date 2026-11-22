# Containerization specs — reusable Docker strategy (plan index)

> **Status: design/reference plan. No implementation in this bundle.** This folder
> defines the target containerization strategy for KanPad so environments can be
> built once and reused efficiently (dev parity, CI parity, test DBs, e2e,
> perf/gungraun baselines, load tests). Implementation work is tracked as tasks in
> `specs/tasks/` (authored via taskpad) and must cite the relevant section here.
> Spec files never carry task status.

## Why this exists

Recent work exposed avoidable waste: repeated throwaway container runs re-downloaded
and reinstalled OS packages, Rust toolchains, and tooling (e.g. valgrind +
gungraun-runner) instead of reusing a pinned, cached environment. Gungraun baselines
are environment-sensitive (valgrind/toolchain/arch), so ad-hoc environments also risk
non-reproducible baselines. This plan standardizes reusable images/containers and
cache mounts while preserving the repo's existing ephemeral/tmpfs test-DB patterns.

## Reading order

1. `00-overview.md` — constitution: goals/non-goals, principles, current-state
   inventory, gaps/risks, locked decisions, DoD for implementation tasks.
2. `10-environments-and-use-cases.md` — per use case (appliance, dev/test, CI,
   e2e, perf/gungraun, load, ops drills): current state, target reusable approach,
   persistence model, and **tmpfs/throwaway DB exceptions**.
3. `20-image-catalog-and-build-strategy.md` — prod vs tooling image catalog,
   proposed `docker/tooling/` layout (future impl), version pins/tags, Dockerfile
   and BuildKit/cache strategy.
4. `30-lifecycle-operations-and-cleanup.md` — reuse/run patterns, cache and
   volume strategy, update/rebaseline flow, disk/network hygiene, cleanup
   guardrails, and troubleshooting notes.

## Scope

**In scope (plan):** reusable container/image strategy for self-host appliance
builds, local integration/unit test DB services, frontend/backend CI parity, Docker
e2e flows, perf benchmarks and gungraun baseline generation/compare, load testing,
and DB migration/ops drills; caching/build efficiency; persistence vs ephemerality
policy; lifecycle/cleanup.

**Out of scope (this plan):** implementing Dockerfiles/compose/workflow/script
changes, regenerating/committing baselines, changing application behavior, adding a
devcontainer, adopting new orchestration platforms (k8s/swarm), or modifying
production deployment topology/contracts beyond documenting reuse of existing ones.

## Relationship to existing artifacts (do not duplicate; cross-link)

- Appliance/deploy contracts: `compose.yaml`, `frontend/Dockerfile`,
  `backend-rs/Dockerfile`, `frontend/nginx.conf`, `docs/self-hosting.md`,
  `docs/deployment.md`
- Local throwaway test DBs: `backend-rs/docker-compose.test.yml`
- CI: `.github/workflows/backend-rs.yml`, `.github/workflows/frontend-storybook.yml`
- E2e: `frontend/scripts/e2e-local.sh`, `frontend/playwright.config.ts`,
  `frontend/e2e/api-base.ts`
- Perf/runbooks and baselines: `docs/perf-runbook.md`, `docs/perf-budgets.md`,
  `backend-rs/benches/order_key_callgrind.rs`, `backend-rs/benches/baselines/`
- Load: `scripts/load/README.md`, `scripts/load/compose.load.yaml`,
  `scripts/load/seed.mjs`, `scripts/load/board-smoke.js`
- Test conventions: `backend-rs/tests/README.md`, `AGENTS.md`

## House rules

- **Design/reference only:** no task status here; live status in `specs/tasks/status.yaml`.
- **No implementation from this folder:** future tasks implement changes and record evidence.
- **Reuse-first, exception-aware:** default to reusable images/caches/containers; DB/test
  data may be ephemeral/tmpfs where specified (see `10-environments-and-use-cases.md`).
- **Pin and align:** tooling versions/images must align with CI/workflow and committed
  manifests; environment-sensitive artifacts (gungraun baselines) are regenerated only in
  the pinned env.
- **Amendments:** if tree/docs/specs disagree or decisions change, update in place and add
  a dated entry to the Amendment log in `00-overview.md`.
- **No commit/push without explicit user approval** (repo rule).