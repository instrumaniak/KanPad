# KanPad

Self-hosted _project management & note taking_ tool for solo developers & small teams.

![KanPad screenshot](https://raziur.com/public_assets/projects/kanpad/KanPad-09-12-2026_opt.png)

### Why?

- Free & source-available (customize and extend as you want)
- No restrictions (create as many boards as you want)
- Self-hosted (deploy to your own server or even cPanel shared hosting!)
- Lightweight & fast

### Tech stack:

- _Frontend_: TypeScript, React 19, Vite, Tailwind CSS, shadcn/ui, TanStack Query
- _Backend_: Rust (Axum, sqlx, tower-sessions)
- _Database_: MySQL / MariaDB

## Features

_Core_:

- [x] User authentication (registration, login, logout)
- [x] Board management (create, archive, restore)
- [x] Column management (create, sort, bulk move cards)
- [x] Card management (create, edit, delete, drag & drop)
- [x] Card detail panel with markdown descriptions
- [x] Color-coded labels
- [x] Card Due dates with calendar picker
- [x] Checklists with progress tracking
- [x] Dark mode support
- [x] Notes with markdown & Mermaid diagrams
- [x] Projects (Organize boards & notes into projects)
- [x] Board view toggle (list/kanban)
- [x] Card search & filtering
- [ ] Admin panel (user management, registration toggle, activity log)
- [ ] Team collaboration (shared projects, boards, notes)
- [x] Responsive layout
- [ ] Progressive web app (PWA) support
- [ ] Finalize source code license

_Deployment_:

- [x] cPanel hosting
- [x] Docker image for the API
- [ ] Templates for common self-hosting setup (portainer, dokploy)

## Backend setup

The backend is a Rust service (`backend-rs/`) built with Axum + sqlx. It needs a
Rust toolchain — the exact version is pinned by `backend-rs/rust-toolchain.toml`
(1.99.1), so `rustup` picks it up automatically.

```bash
$ cd backend-rs
```

First, make sure a MySQL (8.0.19+) or MariaDB (10.6+) database is running and
create a database for the application. Copy `.env.example` to `.env` and update
the database connection details, plus `SESSION_SECRET` (at least 32 bytes —
`openssl rand -hex 32`).

```bash
# apply database migrations
$ cargo run -- migrate run

# create the first admin user
$ cargo run -- create-admin

# development server (http://localhost:3001)
$ cargo run -- serve
```

The API serves JSON only — it does not serve the frontend. The frontend is a
separate static build, see [Frontend setup](#frontend-setup).

Run tests:

```bash
# unit + integration tests (needs a reachable database)
$ cargo test

# coverage (85% line floor, same gate as CI)
$ cargo llvm-cov --workspace --all-targets --fail-under-lines 85
```

The test suites create their own per-test databases, so point `DATABASE_URL` at
a throwaway server — the query macros also read it at compile time:

```bash
$ export DATABASE_URL='mysql://root:dev_root_pw@127.0.0.1:3337/kanpad?charset=utf8mb4&collation=utf8mb4_unicode_ci'
```

`backend-rs/docker-compose.test.yml` starts a MySQL and a MariaDB instance for
local testing.

## Frontend setup

```bash
$ cd frontend
```

Install the packages and run the frontend:

```bash
$ npm install

# development
$ npm run dev

```

The dev server runs on `http://localhost:5173` and proxies `/api` to the
backend. Set `VITE_API_URL` to point it somewhere else — it defaults to
`http://localhost:3001`, the Rust backend's default port.

Run tests:

```bash
# unit tests
$ npm run test

# e2e tests
$ npm run test:e2e

```

> The e2e suite is being retargeted from the old Node backend to the Rust one
> on `:3001` — until that lands, `npm run test:e2e` still expects the retired
> backend on `:3000`.

## Deployment

KanPad runs as two independent origins:

- **SPA** — the static frontend build (`frontend/dist/`), served by any static
  host.
- **API** — the Rust `backend-rs` binary (or its Docker image), serving JSON
  only. It serves no frontend assets and no SPA rewrite; every non-API path
  returns a `problem+json` 404.

Build both:

```bash
# frontend
$ cd frontend && npm run build

# backend
$ cd backend-rs && cargo build --release
```

Runtime configuration is passed as environment variables — never baked into the
build. `backend-rs/.env.example` lists the full key set.

_Detailed deployment instructions (Docker, cPanel/shared hosting, self-hosting
templates) will be filled in here. In the meantime see `docs/deployment.md` for
the production topology, the reverse-proxy contract and the post-deploy smoke
list._

## Acknowledgement

### AI usage
AI assisted development tools were *heavily* used for planning & development of this project.
