# 00-overview — Reusable containerization: constitution

> Plan date: 2026-11-21. This file is the constitution for KanPad's containerization
> strategy: why, scope, principles, current-state inventory, gaps, locked decisions, and
> the definition of done for implementation tasks spawned from this bundle. Per-use-case
> details live in `10`, image/build details in `20`, and lifecycle/cleanup in `30`.

## Why

KanPad already uses containers for the self-host appliance, local throwaway test DBs,
and CI build/e2e flows. The gap is **reuse**: ad-hoc `docker run --rm` + repeated
`apt`/`rustup`/`cargo install`/tool downloads waste limited disk and network, are slow,
and produce non-reproducible environments for tooling whose outputs are
environment-sensitive (notably gungraun/callgrind baselines depend on
valgrind/toolchain/arch). The strategy must make reuse the default while explicitly
allowing **ephemeral/tmpfs databases and disposable test data** where appropriate.

## Scope

**In scope**
- Reusable, versioned container/image strategy for: self-host/prod-like appliance builds,
  local dev/test parity, backend/frontend CI parity, Dockerized e2e, perf benches and
  gungraun baseline generation/compare, load testing (k6), and DB migration/ops drills.
- Build efficiency (BuildKit/buildx, layer/cache mounts, cache reuse) and
  disk/network hygiene for constrained environments.
- Persistence model: what is cached/reused vs what is throwaway/ephemeral (with explicit
  tmpfs DB exceptions).
- Image/tool version pinning and alignment with CI/workflow/manifests.
- Lifecycle: create, reuse, update, rebaseline, retire, and safe cleanup.

**Out of scope**
- Runtime/application behavior changes, schema/query/contract changes, or product features.
- Implementing this plan now (no Dockerfiles/compose/workflow/script edits in this step).
- Replacing native dev workflows; devcontainers/remote IDE config (none exist today).
- New infra platforms (k8s, swarm, hosted registries) unless later adopted explicitly.
- Weakening existing security/build contracts (digest pins, `--locked`, `SQLX_OFFLINE`
  query cache, no baked secrets/`DATABASE_URL`, load/seed safety guards).

## Principles (locked)

| # | Principle | Implication |
|---|-----------|-------------|
| P-1 | **Reuse over rebuild** | Prefer cached images/layers, named services/containers, and mounted caches over repeated installs in throwaway containers. |
| P-2 | **Build once, run many** | Package repeatable toolchains into versioned local tool images (future `docker/tooling/`) instead of per-run bootstrapping. |
| P-3 | **Ephemeral data, durable environments** | Throw away **DBs/test data** when runs require isolation/speed (tmpfs/disposable schema/container); keep tool images, dependency caches, target caches, browser caches, and committed artifacts reusable. |
| P-4 | **Parity by pinning** | Local parity envs must pin the same key versions as CI/manifests for the task (Rust, Node, valgrind, gungraun-runner, sqlx-cli, etc.); env-sensitive outputs are produced only in the pinned env. |
| P-5 | **Prod images stay minimal & reproducible** | Keep deploy images lean and contract-compliant; do not add dev/test/perf tooling to them. |
| P-6 | **Constrained-host hygiene** | Minimize packages/downloads, avoid container/image sprawl, and use targeted cleanup (no blanket destructive prune on shared hosts). |
| P-7 | **Do not duplicate canon** | This bundle defines strategy/decisions; commands/contracts remain owned by existing runbooks, workflows, scripts, and compose files unless a task deliberately moves them. |

## Current-state inventory (verified)

| Area | Artifact(s) | Key behaviors (today) |
|---|---|---|
| Self-host appliance | `compose.yaml`, `frontend/Dockerfile`, `frontend/nginx.conf`, `backend-rs/Dockerfile`, `.env.example`, `docs/self-hosting.md`, `docs/deployment.md` | Same-origin appliance (`frontend` nginx `/api` → `rust:3001`), `rust` runs `migrate run && serve`, required env via `:?`, MariaDB 11.4 default/MySQL 8.4 option, **persistent** DB named volume, digest-pinned bases, musl-static backend build with `SQLX_OFFLINE=true` + BuildKit cargo cache mounts, frontend `VITE_API_URL` build arg, no baked secrets/`DATABASE_URL`. |
| Build contexts | `backend-rs/.dockerignore`, `frontend/.dockerignore` | Exclude `target/`, env files, artifacts; keep `.sqlx/`, migrations, manifests/sources needed for builds; frontend re-includes `.env.example` only. |
| Local test DBs (throwaway) | `backend-rs/docker-compose.test.yml` | `test_db` `mysql:8.4` on `3337:3306` (db `kanpad_rs_test`, fixed name `kanpad-rs-testdb`), `mariadb` `mariadb:11.4` on `3338:3306` (db `kanpad`, fixed name `kanpad-rs-mariadb`), **tmpfs** `/var/lib/mysql` (1G), healthchecks; `docker compose ... up -d --wait` / `down`. `sqlx::test` creates/drops ephemeral per-test DBs. |
| CI (backend-rs) | `.github/workflows/backend-rs.yml` | Service DBs (MySQL/MariaDB legs), fmt/clippy/migrate/sqlx-prepare/checks/tests/coverage/openapi/gen-api/build, Buildx + `docker/build-push-action` with GHA cache to build `kanpad-rust:ci`, smoke/migrate/e2e in throwaway containers against disposable `kanpad_e2e`, Playwright e2e with pinned `E2E_BASE_URL`, artifact uploads; pinned toolchain/actions/rust tooling gates (incl. gungraun compare). |
| CI (frontend) | `.github/workflows/frontend-storybook.yml` | Node-based Storybook/lint/type/build/test flow (no Docker orchestration). |
| E2e (local helper) | `frontend/scripts/e2e-local.sh`, `frontend/playwright.config.ts`, `frontend/e2e/api-base.ts`, `frontend/.env.example` | Provisions/migrates throwaway `kanpad_e2e`, runs Playwright; supports pinning running API/SPA via `E2E_BASE_URL`/`E2E_SPA_URL`/`E2E_DATABASE_URL`; enforces test env/throttle expectations. |
| Perf/gungraun | `docs/perf-runbook.md`, `docs/perf-budgets.md`, `backend-rs/benches/order_key_callgrind.rs`, `backend-rs/benches/baselines/**`, `backend-rs/Cargo.toml` | Callgrind benches via gungraun (`dev-dep 0.20`), baselines committed under `benches/baselines/`; compare/save driven by `GUNGRAUN_HOME` and valgrind; CI gungraun gate is env-sensitive. Criterion bench target also exists. |
| Load | `scripts/load/README.md`, `scripts/load/compose.load.yaml`, `scripts/load/seed.mjs`, `scripts/load/board-smoke.js` | k6 flows against loopback by default, strict safety guards (no remote without opt-in, no dotenv autoload, throwaway DB naming/host checks), throttle raise only via compose override. |
| Test harness/conventions | `backend-rs/tests/README.md`, `BACKEND-RS-ORCHESTRATION.md`, `AGENTS.md` | Shared test DB conventions, tmpfs-for-testing guidance, explicit DB-persistence assertions, MySQL/MariaDB compatibility requirements. |

## Gaps and risks

| Gap/Risk | Impact | Target resolution |
|---|---|---|
| No reusable pinned tool image/container for valgrind+Rust+gungraun-runner | Repeated downloads/installs, wasted time/space/network, non-reproducible baselines, blocked CI parity | `kanpad/tool-rust-perf` (and related tooling images) with mounted caches and repo; single build, many runs (`20`, `30`). |
| Host/CI valgrind mismatch vs committed baselines | `cargo bench -- --baseline=main`/compare fails or produces misleading diffs; baseline churn | Pin valgrind to CI target for baseline env; regenerate baselines in that env; update pins/docs/baselines together (`30`). |
| Ad-hoc container lifecycle (throwaway + rerun installs, stray containers) | Disk/network waste, orphaned containers/images | Named reusable services/containers, `up -d --wait`/`exec` reuse, targeted cleanup (`30`). |
| No single strategy doc for cross-cutting reuse | Drift between appliance, test DBs, CI, e2e, perf, load practices | This bundle as canonical plan; cross-link existing runbooks/workflows/scripts. |
| Valgrind/callgrind under Docker may need extra runtime permissions | Baseline generation can fail (e.g. `setarch`/personality/callgrind errors) | Document minimal required flags/capabilities in tooling usage (seccomp/ptrace considerations) without broad privilege escalation (`30`). |
| Cache/target incompatibility across toolchains/OS/arch | Corrupt or slow builds, confusing failures | Scope caches/target dirs by toolchain/profile/arch as needed; document conventions (`30`). |

## Locked decisions (plan-level)

- **D-C01 — Dedicated location:** `specs/containerization-specs/` is the canonical plan (this bundle). `specs/README.md` indexes it. Living operational details remain in existing docs/workflows/scripts and are cross-linked, not duplicated.
- **D-C02 — Reuse-first default:** all repeatable non-data setup (toolchains, runners, package installs, browser installs, image builds) must be reusable via pinned images/layers and/or persistent caches/containers. `docker run --rm` bootstrap flows that reinstall tools are disallowed for routine use once tooling images exist.
- **D-C03 — Ephemeral DB/test data exception:** disposable, tmpfs-backed, or schema-per-run databases are the standard for local/CI integration, e2e, and perf/load smoke/drill data (including `backend-rs/docker-compose.test.yml`, `sqlx::test` ephemeral DBs, and `kanpad_e2e` flows). Caches/tool images/prod data volumes/committed artifacts are not ephemeral by default.
- **D-C04 — Prod vs tooling separation:** deploy images (`frontend/`, `backend-rs/`) remain minimal and contract-compliant. Reusable tooling images (future `docker/tooling/`) are separate, versioned, and optimized for parity/caching.
- **D-C05 — Pin source of truth:** CI workflows and committed manifests/lockfiles define authoritative versions; tooling image tags must encode and track them. Environment-sensitive artifacts (gungraun baselines) are generated/compared only in the pinned environment.
- **D-C06 — Safe cleanup:** cleanup is targeted and project-scoped; blanket `docker system prune -a` / global volume prune is not the default on constrained/shared hosts.
- **D-C07 — Future implementation location:** reusable non-prod container assets SHOULD live under `docker/tooling/` (Dockerfiles, README, optional compose helpers, version pin manifest) unless a concrete constraint requires another path (record amendment if so).

## Definition of done (for future implementation tasks from this plan)

Each implementation task spawned from this bundle must:
- Cite the specific section(s) implemented and record evidence (commands/outputs/artifacts) in the task/spec notes.
- Preserve P-1…P-7 and D-C01…D-C07 (or record an explicit amendment here).
- Keep throwaway DB/data behavior for test/e2e/load-smoke/drill flows (tmpfs/disposable) unless a task explicitly and deliberately changes it with rationale.
- Maintain CI/local parity for pinned tooling; for gungraun/valgrind or similar env-sensitive outputs, generate and verify in the same pinned env and update baselines/pins/docs together.
- Avoid destructive, non-targeted cleanup; document any new caches/images/volumes and their lifecycle.
- Respect repo rules: no secrets baked into images, no hand-edited `_sqlx_migrations`, DB changes compatible with MySQL 8.4 and MariaDB 11.4 where applicable, and no commit/push without explicit user approval.

## Amendment log

- **2026-11-21 — Initial plan (design only):** created `specs/containerization-specs/`
  (`README.md`, `00-overview.md`, `10-environments-and-use-cases.md`,
  `20-image-catalog-and-build-strategy.md`, `30-lifecycle-operations-and-cleanup.md`)
  and indexed it from `specs/README.md`. Establishes reuse-first container/tooling
  strategy, explicit tmpfs/throwaway DB exceptions, gungraun baseline reproducibility
  requirements, and proposed `docker/tooling/` implementation location. No runtime or
  workflow implementation performed.