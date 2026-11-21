# 20 — Image catalog and build strategy

Defines the target image catalog, version/tagging policy, and efficient build/caching
strategy. **Implementation target for non-prod tooling assets:** `docker/tooling/`
(future tasks; not created in this plan step).

## 1) Image categories

| Category | Purpose | Location (current/planned) | Contract |
|---|---|---|---|
| **Production/deploy images** | Self-host appliance and deployable app artifacts | Current: `frontend/Dockerfile`, `backend-rs/Dockerfile` (+ `compose.yaml`, `frontend/nginx.conf`) | Minimal, reproducible, no dev/test/perf tooling, no baked secrets/`DATABASE_URL`, digest-pinned bases, locked/offline build rules preserved |
| **Tooling/CI-parity images** | Reusable environments for checks, e2e support, perf/gungraun baselines, and maintenance tasks | Planned: `docker/tooling/*` (Dockerfiles, README, optional compose helpers, version manifest) | Pinned versions aligned to CI/manifests, optimized for cache mounts and repeated runs, separate from prod images |
| **Third-party service images** | Databases and external tools (MySQL/MariaDB, k6, nginx/node/rust bases as applicable) | Current: compose/workflow references | Prefer pinned/digested refs per existing repo policy; reuse pulled images; treat data persistence according to use case |

## 2) Production image catalog (keep and reuse)

| Image | Build context | Key characteristics | Reuse notes |
|---|---|---|---|
| **Frontend (appliance SPA/nginx)** | `frontend/` (`frontend/Dockerfile`) | Multi-stage: `node:26-alpine@digest` builder (`npm ci`, Vite build, `ARG/ENV VITE_API_URL` build-time), `nginx:alpine@digest` runtime serving `dist` + `nginx.conf`; healthcheck on `/`. | Rebuild on source/config/base digest changes; leverage Docker layer cache and BuildKit; do not leak `.env` via context (dockerignored). |
| **Backend API (Rust)** | `backend-rs/` (`backend-rs/Dockerfile`) | Multi-stage musl-static build: `rust:1.99-alpine@digest` builder with BuildKit cache mounts for cargo registry/target, `SQLX_OFFLINE=true`, copies `.sqlx/`/migrations/sources, `cargo build --locked --release`; runtime `gcr.io/distroless/static-debian12:debug-nonroot@digest` with binary only, non-root, `PORT=3001`, `/health` check, `serve` default. No `DATABASE_URL` baked. | Primary reuse levers: BuildKit cache mounts + external build cache (`--cache-from/--cache-to` locally/CI) and stable dependency layers. Keep `.dockerignore` excluding `target/`. |

**Build contract to preserve (prod and CI image builds):**
- `SQLX_OFFLINE=true` build path with committed `.sqlx/` query cache (build does not require DB). Distinct from cargo `--offline`.
- `cargo build --locked` for reproducible dependency resolution.
- No secrets or runtime `DATABASE_URL` embedded in images/layers.
- Base images digest-pinned; bump tag and digest together.
- Migrations are applied at runtime where designed (`compose.yaml` `migrate run && serve`) and/or as explicit CI/e2e steps—not as part of immutable image contents beyond packaged migration files.

## 3) Tooling image catalog (planned, reusable)

> All tooling images are separate from prod images, versioned by purpose+toolchain, and
> intended for `docker build` once + repeated `docker run`/`docker exec` with mounted
> caches and repo. Exact Dockerfile content is implementation work.

| Planned image/tag (example) | Purpose | Base/contents (pin to CI/manifests) | Cache/runtime mounts (typical) | Priority |
|---|---|---|---|---|
| `kanpad/tool-rust-perf:<os>-rust<ver>-valgrind<ver>-gungraun<ver>` | **Gungraun/callgrind baseline save & compare**, valgrind-dependent perf checks | CI-matching OS for baseline target (e.g. ubuntu 26.04 when comparing against ubuntu-26.04 runner baselines), pinned `valgrind` (e.g. `3.26.0`), Rust toolchain (e.g. `1.99.1`), `gungraun-runner` (e.g. `0.20.0`, aligned with `backend-rs/Cargo.toml` gungraun major/minor), minimal build deps (`build-essential`, `pkg-config`, `ca-certificates`, `curl` only if needed at build time) | Repo at `/work` (`-v $PWD:/work -w /work/backend-rs`), cargo registry/git, persistent `CARGO_TARGET_DIR`, `GUNGRAUN_HOME=/work/backend-rs/benches/baselines` | **P0** (current blocker class) |
| `kanpad/tool-rust-ci:<os>-rust<ver>-sqlx<ver>` (or consolidated with perf) | Local CI-parity for Rust gates: fmt, clippy, tests/coverage, sqlx prepare/check flows, deny/cargo tools as needed | Rust `1.99.1` (or CI pin), `sqlx-cli` pinned to workflow (e.g. `0.9.0` as in CI), optional `cargo-llvm-cov` (`0.9.1`), `cargo-deny` (`0.20.2`), `gungraun-runner` if consolidated | Same cargo/repo/target mounts; DB access via host/networked test services as required | P1 |
| `kanpad/tool-node-ci:<node-ver>` (optional) | Frontend/Storybook/Playwright CI parity or sandboxed runs | Node `26` (repo/CI-aligned), npm tooling; Playwright system deps and browsers (prefer browser cache volume to avoid re-download) | Repo mount, npm cache, Playwright browser cache (`~/.cache/ms-playwright` or configured path) | P2 (only if native flow insufficient) |
| `kanpad/tool-k6:<ver>` (optional, usually upstream) | Load test execution | Official/pinned `grafana/k6` or equivalent | Mount `scripts/load/`; network access to target; no persistent state required | P2 |

**Consolidation guidance:** prefer fewer images where versions/size tradeoff is reasonable (e.g. a single Rust CI+perf image) to reduce disk footprint and maintenance, but keep tags explicit and rebuild when any pinned component changes.

## 4) Proposed `docker/tooling/` layout (future implementation)

```text
docker/tooling/
  README.md                  # purpose, build/run examples, mounts, flags, cleanup
  versions.env               # or versions.md: authoritative pinned versions + source refs
  rust-perf.Dockerfile       # (or rust-ci.Dockerfile / combined)
  node-ci.Dockerfile         # optional
  compose.tooling.yaml       # optional helper services/runs (no coupling to prod compose)
```

- `README.md` must document: image tags, build commands, required mounts/env, minimal runtime flags (e.g. seccomp for valgrind if needed), file ownership handling, and safe cleanup.
- `versions.env`/manifest must record each pin and its source of truth (workflow step, `Cargo.toml`, `Dockerfile` base, etc.).
- Optional compose helpers must not alter `compose.yaml` or `scripts/load/compose.load.yaml` semantics.

## 5) Version pinning and tagging policy

- **Source of truth:** `.github/workflows/backend-rs.yml` and `.github/workflows/frontend-storybook.yml` for CI tool versions/actions; `backend-rs/Cargo.toml`/`Cargo.lock`/`rust-toolchain` (if present) and prod Dockerfiles for Rust ecosystem/runtime; `frontend/package.json`/lock and frontend Dockerfile for Node; `backend-rs/benches/*` and committed baselines metadata for gungraun/valgrind expectations.
- **Tag format:** include purpose and key versions, e.g. `kanpad/tool-rust-perf:ubuntu26.04-rust1.99.1-valgrind3.26.0-gungraun0.20.0`. Avoid mutable `latest` for reproducible tasks.
- **Immutability in practice:** treat a tag as representing an exact toolset; publish a new tag on any component bump. Retire old tags only after consumers/baselines no longer require them.
- **Env-sensitive artifacts:** when valgrind/toolchain/arch affecting gungraun changes, bump the tool image tag, update CI pin/assertion and `docs/perf-runbook.md` as needed, and regenerate/commit baselines in the same PR/task.

## 6) Build and caching strategy

### BuildKit / buildx (all image builds)
- Enable BuildKit (`DOCKER_BUILDKIT=1`) and prefer `docker buildx build` for local/CI parity.
- Use `--cache-from` / `--cache-to` (local directory cache or registry cache, as appropriate) for repeatable CI and fast local rebuilds.
- Keep layer order stable: base OS/runtime deps → language toolchains → dependency manifests/lockfiles → source/artifacts that change frequently.

### Dockerfile cache mounts
- **Rust:** `--mount=type=cache,target=/usr/local/cargo/registry` and `--mount=type=cache,target=/usr/local/cargo/git` (and/or target dir cache) in build stages; copy compiled outputs out of cache-mounted target paths within the same `RUN` when needed (existing backend pattern).
- **Node:** cache npm downloads where beneficial (`~/.npm`) and rely on manifest-first `npm ci` layering.
- **Tool images:** prefer installing toolchains once in layers (image build time) rather than at container runtime.

### Runtime-mounted caches (for repeated command execution)
- Mount persistent cargo registry/git and a scoped `CARGO_TARGET_DIR` into tool/prod-build containers to avoid recompiling dependencies across runs.
- Mount npm cache and Playwright browser cache for frontend/e2e parity runs.
- Scope target/cache dirs by toolchain/target triple/profile/OS when mixing environments to prevent incompatible artifacts.

### Context and transfer efficiency
- Keep `.dockerignore` strict to exclude `target/`, VCS noise, local envs/secrets, and large non-input artifacts; only broaden context when required for correctness (and document why).
- Avoid copying host caches into image layers; mount them at runtime instead.

## 7) Reproducibility and security constraints

- No secrets, tokens, `.env` contents, or `DATABASE_URL` in Dockerfiles, build args persisted into layers, or committed tooling configs.
- Prefer non-root execution in runtime/CI containers where compatible; tooling images may require specific users/ownership handling for mounted workspaces—document safe defaults.
- For gungraun/valgrind in containers, allow only minimal additional permissions/capabilities required for correct execution (see `30-lifecycle-operations-and-cleanup.md`); never default to broad privileged mode.
- Continue digest-pinned upstream bases for prod images and apply equivalent pinning discipline to tooling bases used for reproducible outputs.