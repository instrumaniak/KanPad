# KanPad backend-rs image (T06 docker contract).
#
# Build context is backend-rs/ (see compose.yaml `rust` service), so only
# backend-rs/ files are addressable here.
#
# Offline/SQL contract:
#   * SQLX_OFFLINE=true (builder ENV) + committed .sqlx/ query cache
#     (COPY .sqlx) — the build needs NO database.
#   * --locked --offline with the committed Cargo.lock — reproducible,
#     network-free Rust build.
#   * DATABASE_URL is NEVER baked in (no ARG/ENV for it anywhere below);
#     it is a runtime-only env (compose.yaml / CI `docker run -e`).
#
# SPA contract (10-architecture.md SPA fallback; serving code lands in T05):
#   * `public/` is NOT in git. CI/vite build copies frontend/dist/ to
#     backend-rs/public/ BEFORE `docker build`; the runtime stage copies it
#     to /app/public (ServeDir::new("public/") resolves against WORKDIR).
#   * Serves :3001 by default (PORT env, default 3001 in Settings).

# The builder base moves WITH the toolchain pin (rust-toolchain.toml =
# 1.99.1) in the same commit — never one without the other.
#
# T62: both bases are DIGEST-pinned (`name:tag@sha256:…`, the manifest-list
# digest, so both amd64 and arm64 resolve). A tag is a pointer upstream can
# move; a digest cannot. Bump the tag AND the digest together — and when the
# toolchain pin moves, this line moves with it (see the comment above).
FROM rust:1.99-bookworm@sha256:48b7f435b8fc6378ce43b44f15e3a47fd8919b2825f835c39bd4bd3da50af5b8 AS builder

ENV SQLX_OFFLINE=true

WORKDIR /app

# Dependency manifests + offline SQL cache first (layer cache).
# T62: `sqlx.toml` is GONE from this COPY — T66 deleted the file (sqlx 0.9
# hard-errors when it exists and the `sqlx-toml` feature is off; the deleted
# file's only setting, `database-url-var = "DATABASE_URL"`, is 0.9's default).
# Keeping the COPY was the next CI red waiting behind the fmt gate.
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
# Compile-time inputs: migrations (future sqlx::migrate! embed), the
# openapi.json served/printed via include_str!, then sources.
COPY migrations ./migrations
COPY openapi.json ./openapi.json
COPY src ./src

RUN cargo build --locked --offline --release

# T62: digest-pinned like the builder (manifest-list digest).
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8 AS runtime

# `curl` is here ONLY for the HEALTHCHECK below (debian:bookworm-slim ships
# neither curl nor wget). It is not app code and not a shell escape hatch —
# the image still runs as the non-root `app` user with nologin.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin app

WORKDIR /app

COPY --from=builder --chown=app:app /app/target/release/backend-rs /usr/local/bin/backend-rs
# Frontend dist/ -> public/ (populated by CI before `docker build`).
COPY --chown=app:app public/ ./public/

USER app

ENV PORT=3001 \
    SQLX_OFFLINE=true

EXPOSE 3001

# T62: container health = `GET /health` (liveness, DB-FREE by contract —
# `state.rs` documents that /health stays 200 through a DB outage while
# /ready reports 503). Probing /ready here would make a DB blip restart the
# container, which is the opposite of what a healthcheck is for.
#
# The port comes from $PORT (Settings default 3001, compose/CI override it),
# so the probe must not hardcode 3001. `--fail` + `-sS` = non-2xx/3xx exits
# non-zero and errors are not silenced. start-period covers the argon2id +
# pool warmup; 30s interval / 5s timeout / 3 retries is deliberately lax so a
# transient blip does not flap the container.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD sh -c 'exec curl -fsS "http://127.0.0.1:${PORT:-3001}/health" >/dev/null'

CMD ["backend-rs", "serve"]
