# T047: Containerization: reusable docker/tooling dev & test environment

> **STATUS: planned (design saved; no implementation yet).** Phase 0 (Architecture/infra).
> Self-contained infrastructure task — no task depends on it, but it *feeds* every
> other task by giving humans, agents, and CI one deterministic environment.
> Answers to the scoping questions are recorded in §Notes.

## Goal

Make KanPad's development, testing, and tooling environment **deterministic and
spawnable by anyone with Docker**, so that:

- A new contributor (or an AI agent) runs one command and gets a fully working
  stack — Rust toolchain, Node toolchain, databases, browsers, k6, valgrind,
  gungraun — with the **exact same pinned versions as CI**, instead of
  re-discovering host tools each session.
- Toolchains, dependency caches, build caches, and browser caches are **reused**
  across runs (build once, run many) instead of re-downloaded/reinstalled per
  container.
- Test/DB data remains **ephemeral (tmpfs/disposable)** per the existing policy
  (D-C03); only environments/caches/artifacts are durable.
- Environment-sensitive outputs (gungraun/callgrind baselines) are produced only
  in the pinned environment, eliminating host/CI valgrind drift.
- If an issue is fixed in this environment, it is fixed for **all** environments
  (local, agent, CI) because they share one pinned image.

This implements the design in `specs/containerization-specs/` (00/10/20/30) and
closes the gap that those specs explicitly left as "design/reference only."

## Depends On

(None hard. Soft coordination: T046 (perf harness — consumes tooling for
gungraun baseline regeneration), T026 (load testing — consumes `scripts/load/`),
T029 (workflow is "NO CHANGES" locked — announce edits).)

## Phase

0

## Critical

Yes — foundational for deterministic dev/test/CI parity.

## Spec References

- `specs/containerization-specs/README.md` (index)
- `specs/containerization-specs/00-overview.md` (constitution P-1…P-7, D-C01…D-C07, DoD)
- `specs/containerization-specs/10-environments-and-use-cases.md` (use-case matrix, tmpfs policy)
- `specs/containerization-specs/20-image-catalog-and-build-strategy.md` (image catalog, `docker/tooling/` layout, pins, build/cache strategy)
- `specs/containerization-specs/30-lifecycle-operations-and-cleanup.md` (reuse/run, cache/volume strategy, cleanup guardrails, troubleshooting)
- `AGENTS.md` (house rules, test conventions, commit policy)
- `AGENTS-local.md` (host cache paths; tmpfs guidance)
- `backend-rs/tests/README.md` (test harness rules, ephemeral ports, migration checksums)
- `backend-rs/docker-compose.test.yml` (throwaway tmpfs DB pattern to reuse)
- `compose.yaml` (prod appliance contract — must not weaken)
- `.github/workflows/backend-rs.yml` (CI pin source of truth)
- `docs/perf-runbook.md`, `docs/perf-budgets.md` (gungraun baseline reproducibility)
- `scripts/load/README.md`, `scripts/load/compose.load.yaml` (load-test guards)

## Files to Create/Modify

### New: `docker/tooling/` (implementation target per D-C07)

| File | Purpose |
|---|---|
| `docker/tooling/versions.env` | Authoritative pinned versions + source refs (Rust, Node, MySQL, MariaDB, Valgrind, gungraun-runner, sqlx-cli, cargo-llvm-cov, cargo-deny, Playwright, k6, OS base, image digests). |
| `docker/tooling/rust-ci.Dockerfile` | Combined CI+perf tooling image: Ubuntu 26.04 base, Rust 1.99.1 (rustup, clippy, rustfmt), build-essential/pkg-config/mysql client, sqlx-cli 0.9.0, cargo-llvm-cov 0.9.1, cargo-deny 0.20.2, gungraun-runner 0.20.0, valgrind 3.26.0. Build-time version asserts. |
| `docker/tooling/node-ci.Dockerfile` | Node 26 base + Chromium system deps for Playwright; browsers installed into a mounted cache volume at first run (lean image). |
| `docker/tooling/k6.Dockerfile` (optional) | Thin pin wrapper around `grafana/k6` latest-stable (or use upstream image directly in compose). |
| `docker/tooling/README.md` | Purpose, image tags, build/run commands, required mounts & env, seccomp flag for valgrind, UID/GID ownership handling, verify-env commands, targeted cleanup. |
| `docker/tooling/compose.dev.yaml` | Reusable dev/test services: `db-mysql` (tmpfs, :3337), `db-mariadb` (tmpfs, :3338), `tool-rust` (long-lived), `tool-node` (long-lived), `tool-k6`, with named cache volumes. |
| `docker/tooling/.dockerignore` | Strict context hygiene for tooling builds. |

### New: root-level orchestration

| File | Purpose |
|---|---|
| `Makefile` | Single entry point for humans/agents/CI: `dev-up`, `dev-down`, `dev-shell-rust`, `dev-shell-node`, `test-db-up`, `test-db-down`, `verify-env`, `ci-backend`, `ci-frontend`, `bench`, `e2e`, `clean-targeted`. |
| `scripts/check-dev-env.sh` | Host-side check (docker/buildx/compose present) + in-container version asserts; exits non-zero on mismatch. |
| `scripts/dev-exec.sh` | Helper to `docker compose exec` into the tool containers with correct UID/GID and env. |

### New: full-stack dev (Phase 3, in scope per Q1)

| File | Purpose |
|---|---|
| `docker/tooling/compose.dev-full.yaml` | Full hot-reload stack: Vite dev server (frontend) + `cargo watch`/`cargo run` (backend) + `db-mysql`/`db-mariadb`, source bind-mounted, caches mounted. Reuses prod contracts (no baked secrets/`DATABASE_URL`). |

### Modified

| File | Change |
|---|---|
| `AGENTS.md` | Point to `make dev-up` / `make dev-shell-rust` as the standard environment; document container-scoped cache paths. |
| `AGENTS-local.md` | Add container env section; reconcile host vs container `CARGO_TARGET_DIR` scoping (no cross-contamination). |
| `backend-rs/tests/README.md` | Add note: tool images provide CI-parity toolchain; tests still use tmpfs DBs and ephemeral ports. |
| `docs/perf-runbook.md` | Add "run in pinned tool image" section; gungraun baseline regeneration procedure in the tool image. |
| `docs/perf-budgets.md` | B1 row: record regenerated baseline Ir (valgrind 3.26.0) once produced in the tool image. |
| `.github/workflows/backend-rs.yml` | (Announced edit) add gungraun-runner/valgrind to tool image usage docs; optionally invoke tool image for perf gate. Coordinate with T029 lock. |
| `specs/containerization-specs/00-overview.md` | Amendment log entry when implementation lands. |
| `specs/tasks/status.yaml` | Add T047 entry (status/phase/depends) when taskpad authors it. |

## Implementation Steps

### Phase 1 — Foundation (deterministic tooling + test DBs)

1. **Create `docker/tooling/` skeleton** (`versions.env`, `.dockerignore`, `README.md` stub) per `20` §4. Record every pin + source ref in `versions.env`.
2. **Rust tooling image** (`rust-ci.Dockerfile`):
   - Base `ubuntu:26.04` (perf parity with CI runner; digest-pinned where available).
   - Install runtime/build deps only: `build-essential`, `pkg-config`, `ca-certificates`, `curl`, `default-mysql-client`, `valgrind` (assert `3.26.0`).
   - Install Rust 1.99.1 via rustup with `clippy`, `rustfmt`; set `RUSTUP_HOME=/usr/local/rustup`, `CARGO_HOME=/cargo`.
   - `cargo install --locked` sqlx-cli 0.9.0, cargo-llvm-cov 0.9.1, cargo-deny 0.20.2, gungraun-runner 0.20.0.
   - Build-time asserts: `rustc --version`, `valgrind --version`, `gungraun-runner --version`, `cargo sqlx --version`.
   - No secrets, no `DATABASE_URL`, no `apt-get upgrade`.
3. **Node tooling image** (`node-ci.Dockerfile`):
   - Base `node:26-bookworm`.
   - Install Chromium system deps (mirror `playwright install --with-deps`).
   - Set `PLAYWRIGHT_BROWSERS_PATH=/ms-playwright` mounted as a named volume; document first-run `npx playwright install chromium --only-shell`.
4. **`compose.dev.yaml`**:
   - `db-mysql`: reuse `backend-rs/docker-compose.test.yml` pattern (mysql:8.4, tmpfs, `:3337`, name `kanpad-rs-testdb`, healthcheck).
   - `db-mariadb`: mariadb:11.4, tmpfs, `:3338`, name `kanpad-rs-mariadb`.
   - `tool-rust`: image `kanpad/tool-rust-ci:<tag>`, bind-mount repo at `/work`, named volumes for cargo registry/git/target, env `CARGO_HOME=/cargo`, `CARGO_TARGET_DIR=/work/backend-rs/target` (container-scoped, distinct from host path), `GUNGRAUN_HOME=/work/backend-rs/benches/baselines`, `command: sleep infinity`, `user: "1000:1000"` (Q2), with writable cache dirs owned by UID 1000.
   - `tool-node`: image `kanpad/tool-node-ci:<tag>`, bind-mount repo at `/work`, named volumes for npm + Playwright browser cache, `working_dir: /work/frontend`, `command: sleep infinity`, `user: "1000:1000"`.
   - `tool-k6`: `grafana/k6` pinned latest-stable, mounts `scripts/load/`.
   - Named volumes for all caches (project-scoped names).
5. **`Makefile`** with targets listed above; `dev-up` = `docker compose -f docker/tooling/compose.dev.yaml up -d --wait`; `dev-down` = `down`; shells via `exec`.
6. **`scripts/check-dev-env.sh`** (host + in-container modes) and **`scripts/dev-exec.sh`**.
7. **Targeted cleanup**: document and script removal of known stray containers/images by name (addresses sprawl); no blanket `docker system prune -a`.

### Phase 2 — Adoption & parity

8. **Regenerate gungraun baselines** in the pinned tool image (valgrind 3.26.0): run `--save-baseline=main`, verify `--baseline=main --callgrind-limits='ir=0%'` passes, commit baselines, update `perf-budgets.md` B1 row + workflow assert. (Closes the 3.18.1→3.26.0 drift.)
9. **Update `AGENTS.md` / `AGENTS-local.md`**: standard env commands, container-scoped cache paths, tmpfs policy unchanged.
10. **Update `backend-rs/tests/README.md`**, `docs/perf-runbook.md` with tool-image run instructions.
11. **Announce workflow edits** (coordinate with T029 lock): document tool-image usage; optionally migrate perf gate to run in the image.
12. **Amend `specs/containerization-specs/00-overview.md`** amendment log.

### Phase 3 — Full-stack dev (in scope per Q1)

13. **`compose.dev-full.yaml`**: Vite dev server (`npm run dev`) + backend (`cargo run` or `cargo watch`) + DBs, source bind-mounted, caches mounted, env injected from `.env` (never baked), same-origin dev proxy preserved. Healthchecks and `--wait` for DBs.
14. **Document** hot-reload workflow and port assignments (`:5173` SPA, `:3001` API, `:3337`/`:3338` DBs) to avoid collisions with prod compose (`:5173` published port — coordinate or use alternate dev ports).

## Constraints

- **No secrets / `DATABASE_URL` baked** into any image or committed config; inject at runtime (preserve prod contract).
- **Prod images stay minimal** (D-C04): `frontend/Dockerfile` and `backend-rs/Dockerfile` are not modified except digest bumps if needed; tooling is separate under `docker/tooling/`.
- **Ephemeral DB/data exception** (D-C03): test DBs remain tmpfs/disposable; `sqlx::test` ephemeral DBs unchanged; `kanpad_e2e` disposable. Caches/tool images/artifacts are durable.
- **No blanket destructive cleanup** (D-C06): targeted, named removal only.
- **MySQL 8.4 + MariaDB 11.4 compatibility** preserved for all test flows.
- **No hand-edited `_sqlx_migrations`**; migrations unchanged.
- **UID 1000** for tool containers (Q2); cache dirs and repo writes must be writable by UID 1000; document ownership fixes.
- **Images stay local** (Q3): no registry push; build once per machine; document build command.
- **k6 latest stable** (Q4): pin the resolved version in `versions.env` at implementation time.
- **Valgrind needs minimal extra permissions**: `--security-opt seccomp=unconfined` for callgrind runs; document; avoid `--privileged`.
- **Cache scoping**: container `CARGO_TARGET_DIR` must be distinct from host paths to avoid cross-contamination (Q10 from review); do not reuse `/mnt/.../.cargo-target-kanpad`.
- **Workflow lock (T029)**: any `.github/workflows/backend-rs.yml` edit must be announced; prefer doc-only changes first.
- **No commit/push without explicit user approval** (repo rule); TDD/red-proof for any new gate; record evidence in §Notes.

## Acceptance Criteria

- [ ] `docker/tooling/` exists with `versions.env`, `rust-ci.Dockerfile`, `node-ci.Dockerfile`, `README.md`, `compose.dev.yaml`, `.dockerignore`.
- [ ] `make dev-up` starts healthy `db-mysql`, `db-mariadb`, `tool-rust`, `tool-node` (and `tool-k6`) with `--wait`.
- [ ] `make dev-shell-rust` gives a shell with `rustc 1.99.1`, `cargo`, `sqlx 0.9.0`, `cargo-llvm-cov 0.9.1`, `cargo-deny 0.20.2`, `gungraun-runner 0.20.0`, `valgrind 3.26.0`, `mysql` client — verified by `make verify-env` (exit 0).
- [ ] `make dev-shell-node` gives Node 26 + Playwright system deps; first-run browser install documented and cached in named volume.
- [ ] A backend gate runs end-to-end inside `tool-rust`: `cargo fmt --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings -A clippy::pedantic`, `cargo test --locked --workspace --all-targets` against `db-mysql`/`db-mariadb` (tmpfs) passes.
- [ ] A frontend gate runs inside `tool-node`: `npm ci`, `npm run lint`, `tsc --noEmit`, `npm test`, `npm run build`, `npm run budget` passes.
- [ ] Full-stack dev (Phase 3): `compose.dev-full.yaml` brings up Vite + backend + DBs; hot reload works; no secrets baked.
- [ ] Gungraun baseline regenerated in the tool image (valgrind 3.26.0) and committed; `--baseline=main --callgrind-limits='ir=0%'` passes in the same image; `perf-budgets.md` B1 row updated.
- [ ] `scripts/check-dev-env.sh` (host + container modes) passes on a clean machine with Docker.
- [ ] Targeted cleanup documented and verified to remove only named stray containers/images (no blanket prune).
- [ ] `AGENTS.md` / `AGENTS-local.md` updated to reference the standard environment.
- [ ] Spec amendment log updated in `00-overview.md`.
- [ ] No prod image contract weakened (digest pins, `--locked`, `SQLX_OFFLINE`, no baked secrets/`DATABASE_URL`).
- [ ] Stray container/image cleanup performed (named targets only) and recorded in §Notes.

## Notes

### Scoping decisions (recorded from user answers, 2026-11-21)

1. **Full stack in scope** — development → testing → anything must be deterministically
   runnable; Phase 3 hot-reload dev compose is included, not deferred.
2. **UID 1000** for tool containers; cache dirs and repo bind mounts writable by
   UID 1000; document `chown`/ownership fixes.
3. **Images stay local** — build once per machine, no registry push.
4. **k6 latest stable** — pin the resolved version in `versions.env` at
   implementation time.
5. **Task file authored by user** (`specs/tasks/T047-...md`); plan saved here; no
   implementation performed in this step.

### Review findings carried in (from containerization-specs review)

- G1: nothing implemented yet (this task closes it).
- G2: valgrind 3.18.1→3.26.0 baseline drift — close in Phase 2 step 8.
- G3: UID/GID ownership — resolved by Q2 (UID 1000) + documented handling.
- G4: agent entry point — `Makefile` targets + `scripts/dev-exec.sh`.
- G5: node tool image promoted from optional P2 to required (Phase 1).
- G6: env verification via `scripts/check-dev-env.sh` + `make verify-env`.
- G7: stray container/image cleanup — targeted, named, recorded.
- G8: compose helpers fully specified in Phase 1 step 4.
- G9: k6 version pinned at implementation time (Q4).
- G10: container-scoped `CARGO_TARGET_DIR` distinct from host paths.
- G11: full-stack dev compose included (Phase 3) per Q1.

### Implementation notes

(filled in during/after implementation — incl. build times, image sizes, cache
volume usage, docker-e2e duration delta if CI adopts, baseline Ir values,
cleanup inventory, and any amendments.)