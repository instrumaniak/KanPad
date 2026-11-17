# KanPad backend-rs image (T012 appliance build).
#
# Build context is backend-rs/ (see compose.yaml `rust` service), so only
# backend-rs/ files are addressable here.
#
# MUSL-STATIC, not glibc: rustls + ring pull in no OpenSSL/aws-lc C tree
# (verified in Cargo.lock), so the release binary is built on Alpine's musl
# toolchain and ships as a FULLY STATIC binary on a distroless runtime —
# no glibc, no curl, no useradd (non-root is built into the base).
#
# SQL/network contract — two DIFFERENT axes, do not conflate them:
#   * SQLX_OFFLINE=true (builder ENV) + committed .sqlx/ query cache
#     (COPY .sqlx) — the build needs NO database. `query!` macros compile
#     against the committed cache instead of a live server. This one is
#     genuinely offline and must stay.
#   * `cargo build --locked` with the committed Cargo.lock — reproducible
#     dependency set, NOT a network-free build. Reproducibility comes from
#     the lockfile's checksums + the digest-pinned bases below.
#   * DATABASE_URL is NEVER baked in (no ARG/ENV for it anywhere below).
#
# T62: both bases are DIGEST-pinned (`name:tag@sha256:…`, manifest-list
# digest). A tag is a pointer upstream can move; a digest cannot. Bump the
# tag AND the digest together — and when the toolchain pin moves, the
# Alpine rust tag moves with it.
FROM rust:1.99-alpine@sha256:44819b27be4c97b0e4d7203bd2e02c4820f97700211b6a37fb4c246a2de59383 AS builder

ENV SQLX_OFFLINE=true

# ring's C/asm sources build with musl's gcc (pre-generated asm since 0.17).
RUN apk add --no-cache musl-dev

WORKDIR /app

# Dependency manifests + offline SQL cache first (layer cache).
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
# Compile-time inputs: migrations (embedded by `sqlx::migrate!`), the
# openapi.json served/printed via include_str!, then sources.
COPY migrations ./migrations
COPY openapi.json ./openapi.json
COPY src ./src

# BuildKit cache mounts: the registry and target/ survive rebuilds on this
# machine only (fresh CI builders start empty — the build is unaffected).
# T62 cache-mount lesson: the binary MUST be cp'd out of the cache-mounted
# target/ inside THIS RUN — a cache mount's contents are not in the layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --locked --release \
    && cp target/release/backend-rs /tmp/backend-rs

# T62: digest-pinned like the builder (manifest-list digest).
# debug-nonroot = busybox (HEALTHCHECK via wget; home-lab users can exec in)
# + built-in `nonroot` user (uid 65532). No shell in PATH for CMD chains is
# NOT a problem — compose.yaml uses /busybox/sh explicitly for migrate+serve.
FROM gcr.io/distroless/static-debian12:debug-nonroot@sha256:d5563cc7f2f44313f332e91138cc8c6a158899afeeeab2fce3b0f9ccdb3cf9ee AS runtime

# `sqlx` compile-time inputs are irrelevant at runtime; only the binary ships.
COPY --from=builder --chown=65532:65532 /tmp/backend-rs /usr/local/bin/backend-rs

USER nonroot

ENV PORT=3001 \
    SQLX_OFFLINE=true

EXPOSE 3001

# The :debug base ships Entrypoint=["/busybox/sh"] (interactive debug shell by
# design). REPLACE it — otherwise argv routes through sh and the binary is
# read as a shell script ("ELF: not found"). Compose/Swarm command overrides
# replace CMD wholesale and append to ENTRYPOINT; put the binary in CMD only.
ENTRYPOINT []
# T012/T62: container health = `GET /health` (DB-FREE by contract). Port
# honored from $PORT. distroless ships no curl — busybox wget instead.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD ["/busybox/sh", "-c", "exec /busybox/wget -q -O /dev/null http://127.0.0.1:${PORT:-3001}/health"]

CMD ["/usr/local/bin/backend-rs", "serve"]
