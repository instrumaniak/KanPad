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

FROM rust:1.96-bookworm AS builder

ENV SQLX_OFFLINE=true

WORKDIR /app

# Dependency manifests + offline SQL cache first (layer cache).
COPY Cargo.toml Cargo.lock sqlx.toml ./
COPY .sqlx ./.sqlx
# Compile-time inputs: migrations (future sqlx::migrate! embed), the
# openapi.json served/printed via include_str!, then sources.
COPY migrations ./migrations
COPY openapi.json ./openapi.json
COPY src ./src

RUN cargo build --locked --offline --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
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

CMD ["backend-rs", "serve"]
