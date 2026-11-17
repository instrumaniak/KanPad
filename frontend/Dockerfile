# KanPad frontend image (T012) — the self-host appliance's web server.
#
# The appliance is same-origin: nginx below proxies /api/* to the rust
# service and the SPA is built with VITE_API_URL="" (the client.ts BASE
# default), so browsers never do a cross-origin call. It IS production
# (docs/self-hosting.md); the org's two-origin deployment is docs/deployment.md.
#
# T62: both bases are DIGEST-pinned (`name:tag@sha256:…`, the manifest-list
# digest, so amd64 and arm64 resolve). A tag is a pointer upstream can move;
# a digest cannot. Bump the tag AND the digest together.

# Builder base moves WITH the repo's Node pin (CI pins Node 26 LTS —
# .github/workflows/frontend-storybook.yml, backend-rs.yml).
FROM node:26-alpine@sha256:6bd9ab43c0ec0190ca7b2d193906cb8b7edf6e7f4afa43ddeccd6452710bffb5 AS builder

WORKDIR /app

# Manifests first (layer cache); the lockfile is the dependency contract.
COPY package.json package-lock.json ./
RUN npm ci

COPY . .

# VITE_API_URL is a BUILD-time value — Vite statically bakes
# import.meta.env.VITE_API_URL into the bundle (see src/lib/api/client.ts).
# Default "" = same-origin via the compose nginx /api proxy. frontend/.env is
# dockerignored, so the build below cannot see a developer's local override.
ARG VITE_API_URL=""
ENV VITE_API_URL=$VITE_API_URL
RUN npm run build

# T62: digest-pinned like the builder (manifest-list digest).
FROM nginx:alpine@sha256:fed7e14be83cbd2661a75cdc5b872d936cf155d46c327084d30b8df19ae88d72 AS runtime

# SPA fallback + /assets caching + /api proxy (appliance — see header).
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80

# nginx:alpine ships busybox wget, NOT curl.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1/ || exit 1
