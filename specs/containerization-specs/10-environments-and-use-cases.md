# 10 — Environments and use cases

Per-use-case plan for reusable containerization. **Default:** reuse images, layers,
containers/services, and caches. **Exception (explicit):** throwaway/tmpfs or
disposable databases and test data where isolation/ephemerality is required.

## Use-case matrix

| Use case | Current state | Target (reusable) approach | Persistence model | Key artifacts / cross-links |
|---|---|---|---|---|
| **1. Self-host appliance (prod-like)** | `compose.yaml` builds `frontend` + `rust`, runs bundled DB (MariaDB default / MySQL option), applies migrations at boot, uses persistent named DB volume; digest-pinned bases and locked/offline build contract in Dockerfiles. | Keep compose/Dockerfiles as deploy contract; reuse BuildKit/buildx caches and pinned images for rebuilds; support future prebuilt image swap without changing runtime contract. | **Persistent:** DB volume (`mariadb-data`/`mysql-data`), intentionally built images. **No** baked secrets/DB URL. | `compose.yaml`, `frontend/Dockerfile`, `backend-rs/Dockerfile`, `frontend/nginx.conf`, `docs/self-hosting.md`, `docs/deployment.md` |
| **2. Local backend integration/unit tests (Rust/sqlx)** | Native `cargo test` against DBs from `backend-rs/docker-compose.test.yml` (`:3337` MySQL, `:3338` MariaDB, tmpfs, fixed names). `#[sqlx::test]` creates/drops ephemeral DBs. | Start/reuse DB services (`up -d --wait`) instead of recreating ad hoc; keep cargo registry/git/target caches persistent and mounted/defined (`CARGO_HOME`, `CARGO_TARGET_DIR`). Use CI-parity tool image only when host toolchain parity is needed. | **Ephemeral (exception):** DB storage tmpfs, per-test DBs/schemas disposable. **Persistent:** cargo caches, target cache, source/tool images. | `backend-rs/docker-compose.test.yml`, `backend-rs/tests/README.md`, `AGENTS.md`, `.github/workflows/backend-rs.yml` |
| **3. Local frontend tests / Storybook / lint / typecheck** | Native Node/npm workflows; Playwright/browser deps cached on host/CI where configured. | Reuse npm cache and Playwright browser cache; optional pinned Node/Playwright tool image for sandbox/CI parity (not required by default). | **Persistent:** npm cache, browser cache, `node_modules` strategy as appropriate. **No** throwaway DB requirement. | `frontend/`, `.github/workflows/frontend-storybook.yml`, `frontend/package.json` |
| **4. End-to-end tests (Playwright)** | CI builds `kanpad-rust:ci`, runs throwaway API container + disposable `kanpad_e2e` on service DB; local helper `frontend/scripts/e2e-local.sh` provisions/migrates `kanpad_e2e` and runs Playwright; config supports `E2E_BASE_URL` pinning. | Reuse built API image and cached frontend deps/browsers; standardize on tmpfs-backed test DB service or disposable schema/container for e2e data; mirror CI env/guards locally via reusable tooling/scripts (future impl). | **Ephemeral (exception):** e2e DB/schema/data and per-run e2e containers. **Persistent:** API/frontend images, build caches, npm/browser caches, source artifacts. | `.github/workflows/backend-rs.yml` (`docker-e2e`), `frontend/scripts/e2e-local.sh`, `frontend/playwright.config.ts`, `frontend/e2e/api-base.ts`, `backend-rs/docker-compose.test.yml` |
| **5. CI parity (backend/frontend gates)** | CI encodes authoritative pins and command order (fmt/clippy/migrate/prepare/check/test/coverage/openapi/build/e2e/gungraun, Storybook checks). | Provide reusable local environments/images that match CI versions for reproducing failures; mount caches and repo; run same gate commands. Do not replicate CI-only secrets/services. | **Mostly persistent caches/tool envs;** CI job containers/services are ephemeral per run. Local reproductions should prefer reusable tool env + tmpfs/throwaway DBs where DBs are involved. | `.github/workflows/backend-rs.yml`, `.github/workflows/frontend-storybook.yml` |
| **6. Perf benches and gungraun baselines (critical)** | Callgrind benches in `backend-rs/benches/order_key_callgrind.rs`; baselines committed in `backend-rs/benches/baselines/`; env-sensitive to valgrind/toolchain/arch; prior attempts used throwaway bootstrap installs. | Use **pinned reusable tool image/container** (Rust + valgrind + gungraun-runner matching CI target) with mounted repo, cargo caches, target cache, and `GUNGRAUN_HOME`; run save/compare in same env. No per-run apt/rustup/runner installs. | **Persistent:** committed baselines, cargo caches, target cache, tool image. **Ephemeral:** none required (bench is CPU/tooling-focused; if a future bench needs DB, use tmpfs/disposable DB). | `docs/perf-runbook.md`, `backend-rs/benches/baselines/`, `backend-rs/benches/order_key_callgrind.rs`, `backend-rs/Cargo.toml`, `.github/workflows/backend-rs.yml` |
| **7. Load testing (k6)** | `scripts/load/*` with safety guards, seed script, compose throttle override `scripts/load/compose.load.yaml` (never edit prod compose). | Reuse appliance/images and any k6/tool images; apply override-only config; run against loopback/approved targets with disposable/seeded data stores. | **Ephemeral (exception, as needed):** load-test DB/data/schema or tmpfs-backed target DB for reproducible disposable runs. **Persistent:** scripts, k6 image, app images/caches. | `scripts/load/README.md`, `scripts/load/compose.load.yaml`, `scripts/load/seed.mjs`, `scripts/load/board-smoke.js`, `compose.yaml` |
| **8. DB migration, compatibility, and ops drills** | `backend-rs/docker-compose.test.yml` for MySQL/MariaDB local legs; runbooks in `docs/data-migration-live.md`, `docs/mariadb-compat.md`. | Reuse test DB services or isolated disposable DB containers for drills; keep drill data non-persistent unless explicitly intended; validate against MySQL 8.4 and MariaDB 11.4. | **Ephemeral by default:** drill DB instances/data (tmpfs or removed volumes/containers). **Persistent:** migration files, runbooks, tooling images/caches. | `backend-rs/docker-compose.test.yml`, `docs/data-migration-live.md`, `docs/mariadb-compat.md`, `backend-rs/migrations/` |
| **9. SQLx/OpenAPI/typegen/tooling maintenance** | CI runs `sqlx prepare --check`, OpenAPI snapshot/generation checks, frontend `gen:api --check`/tsc/build gates. | Run in pinned Rust/Node envs (host or reusable tool images) with persistent caches; ensure `.sqlx/` and generated artifacts remain consistent per repo process. | **Persistent:** caches, committed `.sqlx/`/`openapi.json`/generated types when intentionally updated. **Ephemeral:** temp DBs if needed for prepare/migration verification. | `.github/workflows/backend-rs.yml`, `backend-rs/.sqlx/`, `backend-rs/openapi.json`, `docs/perf-runbook.md` (tooling context as needed) |

## Throwaway/tmpfs policy (explicit)

### Allowed / required ephemeral
- **Test DB storage:** tmpfs-backed MySQL/MariaDB data directories for integration and
  compatibility testing (`backend-rs/docker-compose.test.yml` pattern).
- **Per-test databases:** databases/schemas created and dropped by `sqlx::test` or
  equivalent test harness behavior.
- **E2e databases/schemas:** disposable `kanpad_e2e` (or equivalent) data for local and
  CI e2e runs; e2e containers may be ephemeral per run.
- **Load/perf-smoke/drill datasets:** disposable seeded data and temporary DB instances
  used for non-production validation.
- **One-off run containers:** acceptable only when startup is cheap and no repeated
  environment bootstrap is involved (e.g. running a command in an already-built image with
  mounted caches). Do not use as an excuse for repeated tool installs.

### Must be reusable / persistent by default
- **Images:** prod images and versioned tooling images.
- **Caches:** cargo registry/git, cargo target (appropriately scoped), npm cache,
  Playwright browser cache, BuildKit/build caches.
- **Committed artifacts:** `.sqlx/`, lockfiles, migrations, OpenAPI snapshots/generated
  contracts as governed by repo process, gungraun baselines and logs under
  `backend-rs/benches/baselines/`.
- **Production/self-host data:** named DB volumes for appliance deployments.

## Environment-specific notes

### Test DB services (`backend-rs/docker-compose.test.yml`)
- Treat as **reusable services with ephemeral storage**: prefer `docker compose -f backend-rs/docker-compose.test.yml up -d --wait [test_db|mariadb]` and reuse running healthy containers; stop when not needed.
- Keep fixed ports/names unless a documented collision-avoidance strategy is introduced; do not spawn duplicate ad-hoc MySQL/MariaDB containers for the same purpose.
- Any local tooling that assumes persisted DB files across `down`/recreate is incompatible with tmpfs by design (and should not be introduced).

### E2e
- DB/data isolation is required; **image/build/cache reuse is required** for efficiency.
- CI topology (built API image on `:3001`, Playwright-managed SPA, disposable `kanpad_e2e`) is the parity reference; local runs may use native SPA/API but must preserve throwaway-DB and safety constraints.

### Perf/gungraun
- Baselines are comparable only on matching environments (valgrind version, toolchain, arch, runner config). A pinned tool image/container is the standard mechanism for repeatable save/compare.
- If Docker execution requires elevated runtime allowances for valgrind/callgrind, use the minimal documented settings (see `30-lifecycle-operations-and-cleanup.md`); do not rebuild/reinstall tools per attempt.

### Load
- Preserve all guardrails in `scripts/load/README.md` and `scripts/load/seed.mjs` (loopback default, explicit remote opt-in, no dotenv autoload, throwaway DB naming/host checks, throttle changes only via `compose.load.yaml`).
- Compose runs production-like cookies/security posture; follow existing runbook guidance for HTTP vs TLS contexts.