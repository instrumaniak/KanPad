# 30 — Lifecycle, operations, and cleanup

Operational strategy for creating, reusing, updating, and retiring containerized
environments with minimal disk/network churn. Examples below are **planned/reference
patterns** for future implementation and local use; adapt to the final `docker/tooling/`
assets and pinned tags.

## 1) Day-to-day reuse patterns

### Prefer persistent services and cached environments
- **Test DBs (tmpfs data, reusable services):**
  - Start when needed, keep healthy/reuse across runs:
    - `docker compose -f backend-rs/docker-compose.test.yml up -d --wait test_db mariadb`
  - Stop when idle (data is ephemeral by design): `docker compose -f backend-rs/docker-compose.test.yml stop` (or `down` when a clean tmpfs reset is desired).
  - Do not create parallel ad-hoc DB containers for the same ports/purpose.
- **Tooling runs (no reinstall per run):**
  - Build the pinned tool image once; run commands by mounting repo/caches/baselines.
  - For repeated interactive/debug work, prefer a named long-lived container (`docker create`/`docker start` + `docker exec`) over repeated `--rm` containers that cannot retain non-mounted state. (Do not store valuable artifacts only inside container writable layers.)
- **Prod-like appliance:**
  - Reuse pulled/built images and BuildKit caches; `docker compose up --build` only when inputs changed.

### Reference pattern: reusable gungraun baseline env (future `kanpad/tool-rust-perf`)

```bash
# build once (pinned tag from docker/tooling/versions.env)
docker build -f docker/tooling/rust-perf.Dockerfile -t kanpad/tool-rust-perf:<tag> docker/tooling

# run many (example mounts/caches; adjust paths and UID/GID handling)
docker run --rm \
  --security-opt seccomp=unconfined \
  -e CARGO_HOME=/cargo -e RUSTUP_HOME=/usr/local/rustup \
  -v "$PWD":/work -w /work/backend-rs \
  -v "$HOME/.cache/kanpad/cargo/registry":/cargo/registry \
  -v "$HOME/.cache/kanpad/cargo/git":/cargo/git \
  -v "$HOME/.cache/kanpad/target-rust-perf":/work/backend-rs/target \
  kanpad/tool-rust-perf:<tag> \
  bash -lc 'GUNGRAUN_HOME="$PWD/benches/baselines" cargo bench --bench order_key_callgrind -- --save-baseline=main'
```

Notes:
- Use the **same pinned env** for `--save-baseline=main` and `--baseline=main --callgrind-limits='ir=0%'`.
- `--security-opt seccomp=unconfined` is a common minimal allowance for valgrind/callgrind in containers; validate and document the least-permissive setting that works in the implementation task (avoid `--privileged` unless strictly necessary and justified).
- If the container writes as root, fix ownership only for affected paths (e.g. `backend-rs/benches/baselines`, mounted cache dirs) after the run.

## 2) Cache and storage strategy

| Cache / data | Recommended persistence | Scope/isolation | Notes |
|---|---|---|---|
| Cargo registry (`$CARGO_HOME/registry`) | Persistent host dir or named volume | Shareable across Rust projects/toolchains when compatible | Largest repeated-download win; mount into builds and tool runs. |
| Cargo git db (`$CARGO_HOME/git`) | Persistent | Same as above | Small but avoids refetches. |
| Cargo target (`CARGO_TARGET_DIR`) | Persistent, scoped | Separate by toolchain/profile/target/OS/arch as needed (e.g. host vs musl image vs perf image) | Avoid cross-contamination causing rebuilds or incompatibilities. |
| npm cache / frontend deps | Persistent | Per Node major/version where relevant | Avoid repeated `npm ci` downloads; browser cache separate below. |
| Playwright browsers | Persistent cache volume/dir | Per Playwright version as applicable | Prevents large repeated browser downloads. |
| Buildx/build caches | Persistent | Per builder/cache key | Use `--cache-to/--cache-from` for reproducible rebuild speedups (local dir or configured backend). |
| Test DB data (`test_db`, `mariadb`) | **Ephemeral (tmpfs)** | Per service instance/run | Do not persist; reset by recreating container/service. |
| E2e/load/drill datasets | **Ephemeral/disposable** | Per run/suite | Enforce existing safety guards (no prod targets, throwaway naming). |
| Prod/self-host DB volumes | **Persistent** | Per deployment | Backups/restore governed by ops docs, not tmpfs. |
| Committed artifacts (`.sqlx/`, baselines, locks, snapshots) | Version-controlled | Repo | Update only intentionally in pinned/reviewed flows. |

**Space guidance:** prefer a small number of well-scoped cache dirs/volumes under a known location (e.g. `$HOME/.cache/kanpad/...` or project-approved cache path) so they can be inspected and cleaned intentionally.

## 3) Update, pin bump, and rebaseline flow

When a pinned component changes (Rust, Node, valgrind, gungraun-runner, sqlx-cli, base image digest, etc.):
1. **Update pins together:** tool image Dockerfile/tag + `docker/tooling/versions.env`/README + CI workflow assertions/steps + relevant docs (`docs/perf-runbook.md`, this bundle) as applicable.
2. **Rebuild once:** build the new tagged tool image; keep old tag only if still needed for comparison/rollback.
3. **Regenerate env-sensitive outputs in the new pinned env:** e.g. gungraun baselines via `--save-baseline=main` using the new valgrind/toolchain image.
4. **Verify in the same env:** e.g. `--baseline=main --callgrind-limits='ir=0%'` and affected tests/gates (MySQL 8.4 + MariaDB 11.4 where DB behavior is in scope).
5. **Review diffs intentionally:** commit only expected baseline/artifact changes; investigate unexpected `.out`/log churn rather than blindly accepting it.
6. **Record:** update Amendment log (`00-overview.md`) and/or task notes/lessons as appropriate.

## 4) Cleanup and disk/network hygiene (targeted, safe)

**Do**
- Stop/remove **known stray one-off containers** (e.g. abandoned baseline/bootstrap containers) promptly: `docker ps -a`, `docker rm -f <name>` for specific containers.
- Remove **superseded project-tagged images** when no longer needed: `docker images`, `docker image rm kanpad/tool-rust-perf:<old-tag>`.
- Prune **specific** builder/cache artifacts when space is tight and impact is understood (e.g. `docker builder prune` with care), preferring project-scoped cache dirs for mounted caches that can be deleted selectively.
- Reuse existing base images (`ubuntu:26.04`, `mysql:8.4`, etc.) when they match pinned needs instead of re-pulling variants unnecessarily.
- Keep package installs minimal in tool images; avoid `apt-get upgrade` unless required for a pinned fix.

**Avoid (default policy on constrained/shared hosts)**
- `docker system prune -a` and global `docker volume prune` without enumerating impact (can delete unrelated images/volumes/caches used by other work).
- Recreating environments to "clean up" when a targeted cache/container removal suffices.
- Parallel duplicate stacks bound to the same fixed ports.

**Quick inventory commands (safe/read-only):**
- `docker ps -a --format 'table {{.Names}}\t{{.Status}}\t{{.Image}}'`
- `docker images --format 'table {{.Repository}}\t{{.Tag}}\t{{.Size}}'`
- `docker system df`

## 5) Troubleshooting notes

| Symptom | Likely cause | Guidance |
|---|---|---|
| Gungraun/callgrind fails in Docker (e.g. `setarch: failed to set personality...`, valgrind/callgrind exit errors) | Insufficient container permissions/seccomp/profile for valgrind tooling | Run in the pinned tool image with minimal extra allowances (commonly `--security-opt seccomp=unconfined`); document the working minimal flags in `docker/tooling/README.md`; avoid rebuilding toolchains as a workaround. |
| Baseline compare fails on CI but passes locally (or vice versa) | Valgrind/toolchain/arch/OS mismatch vs committed baselines or CI runner | Generate and compare baselines in the CI-matching pinned env; align workflow pin and baseline metadata; do not "fix" by loosening limits without evidence. |
| Generated baseline/log files owned by root | Container ran as root with repo bind mount | `chown` only the affected repo/cache paths; consider running container with matching UID/GID if compatible with tools. |
| Repeated dependency downloads despite caches | Caches not mounted, wrong `CARGO_HOME`/`CARGO_TARGET_DIR`, or ephemeral container layers used for installs | Verify mount points/env vars; install toolchains in the image layer (once), not at runtime. |
| Port/name conflicts for test DBs | Existing `kanpad-rs-testdb`/`kanpad-rs-mariadb` or other services using `3337/3338` | Reuse/stop the known service instead of starting duplicates; only remap ports with explicit, documented coordination. |
| Slow or huge Docker builds | Large build context or cold caches (e.g. `target/` accidentally included) | Verify `.dockerignore`, BuildKit cache mounts, and `--cache-from/--cache-to` configuration. |

## 6) Security and compliance notes

- Never bake `DATABASE_URL`, session secrets, API keys, or `.env` contents into images or committed tooling files; inject at runtime where required.
- Keep existing load/e2e safety guards: loopback defaults, explicit remote opt-in, no dotenv autoload for load seed, throwaway DB/schema naming and host checks.
- Prefer least privilege; elevate container capabilities only as needed for tooling correctness (e.g. valgrind) and document the rationale.
- Treat Playwright traces/reports and similar artifacts as potentially sensitive when generated against authenticated flows; follow existing retention/handling practices.

## 7) Immediate planned application (known need)

- **Gungraun baseline alignment:** regenerate `backend-rs/benches/baselines/**` using a reusable, pinned **Valgrind 3.26.0** (ubuntu-26.04 parity) + Rust `1.99.1` + gungraun-runner `0.20.0` environment with mounted caches/repo; verify compare in the same env; reconcile unintended `.out` churn; update CI pins/docs in lockstep as part of the implementation task(s). No throwaway apt/rustup/runner bootstrap loops.
- **Stray container hygiene:** stop/remove abandoned ad-hoc baseline containers/images created during investigation and keep only intentional, reusable assets.