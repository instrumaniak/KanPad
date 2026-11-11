---
project_name: 'KanPad'
user_name: 'Raziur'
date: '2026-11-11'
sections_completed: ['technology_stack', 'language_rules', 'framework_rules', 'testing_rules', 'code_quality_rules', 'workflow_rules', 'critical_rules']
status: 'complete'
rule_count: 132
optimized_for_llm: true
---

# Project Context for AI Agents

_Cross-cutting rules and patterns for AI agents implementing code in **KanPad**.
This file is a summary, not the source of truth. When a rule here contradicts a
canonical document, the canonical document wins:_

- `AGENTS.md` — repo-wide agent instructions (task workflow, DB safety, commit rules)
- `frontend/CONVENTIONS.md` — frontend API layer / React Query / E2E / component rules
- `backend-rs/tests/README.md` — Rust test harness rules (canonical)
- `specs/api-rewrite-rs-specs/10-architecture.md` + `21-api-standard-shapes.md` — API contract & architecture
- `docs/deployment.md` — production topology; `docs/mariadb-compat.md` — SQL portability

---

## Technology Stack & Versions

### Core Runtime
- **Backend: Rust** (edition 2024, MSRV `1.99`, pinned by `backend-rs/rust-toolchain.toml` → `1.99.1`)
- **Frontend: TypeScript** (strict mode) — React 19 + Vite
- **Database: MySQL 8.4 (dev/CI) and MariaDB 10.6 (production)** — both must work
- The **NestJS/Node backend was deleted** (`4051be68`). There is no `backend/` directory, no TypeORM, no Jest.

### Frontend (`frontend/`)
- React 19 + **React Compiler** (`babel-plugin-react-compiler` via `@rolldown/plugin-babel`) — components are compiler-compiled; do not defeat the compiler with refs/effects that fight it
- Vite 8, Tailwind CSS v4 via `@tailwindcss/vite` (no `tailwind.config.js`)
- shadcn/ui on the `radix-ui` meta package (Radix primitives) — `src/components/ui/`
- TanStack React Query v5 for server state; React Router v7 (`BrowserRouter` + `Routes`)
- `@dnd-kit/*` for drag-drop; `react-markdown` + `remark-gfm` + `rehype-highlight` + `rehype-raw` + `rehype-sanitize`
- `mermaid` v11 — **lazy `import('mermaid')`, client-side only**, rendered by `features/notes/mermaid-diagram.tsx`
- `dompurify` for HTML sanitization, `date-fns` for dates, `react-day-picker` for the due-date picker
- `lucide-react` for icons

### Backend (`backend-rs/` — single crate, no workspace)
- **Axum 0.8** + `tokio` (multi-thread runtime) + `tower` 0.5 / `tower-http` 0.7
- **`sqlx` 0.9** (`mysql`, `mysql-rsa`, `runtime-tokio`, `tls-rustls-ring-native-roots`, `chrono`, `migrate`, `macros`) — offline query cache `.sqlx/` is committed
- `tower-sessions` 0.15 + `tower-sessions-core` 0.15 (`deletion-task`) — **the session store is in-tree** (`src/common/session_store.rs`), NOT `tower-sessions-sqlx-store`
- `tower_governor` 0.8 (rate limiting), `tower-http` `csrf` feature (Go 1.25 `CrossOriginProtection`)
- `argon2` 0.6 (Argon2id) + `bcrypt` 0.19 (**legacy verify only**), `garde` 0.23 (validation), `secrecy` 0.10 (secret handling)
- `utoipa` 6 + `utoipa-swagger-ui` 10 (OpenAPI), `sha2` 0.11 (Sha512 → session cookie key)
- `clap` 4 + `dialoguer`/`rpassword` (CLI), `tracing`/`tracing-subscriber` (logs), `thiserror` 2 + `anyhow` (errors)
- Layout: `src/features/<domain>/{mod.rs,routes.rs,store.rs,types.rs}` + shared kernel `src/common/`

### Testing
- **Frontend unit**: Vitest 4 + `@testing-library/react` + jsdom — `npm run test` (co-located `*.test.ts(x)`)
- **Frontend E2E**: Playwright (`frontend/e2e/*.spec.ts`)
- **Backend**: `cargo test` — in-process suites (`axum::Router` + `oneshot`) and black-box suites (raw TCP), both against a live MySQL/MariaDB
- **Backend coverage floor**: 85% lines (`cargo llvm-cov --fail-under-lines 85`)

### Build & Tooling
- Frontend: Vite build (`npm run build` = `tsc -b && vite build`); ESLint 9 flat config (`frontend/eslint.config.js`)
- Backend: `cargo build --release` (thin LTO, 1 codegen unit, symbols stripped); Docker image `rust:1.99-bookworm` builder
- Root `.prettierrc` (`semi`, `singleQuote`, `trailingComma: all`, `printWidth: 100`, `tabWidth: 2`, `arrowParens: always`) — **Prettier is not installed as a dependency and no pre-commit hook exists**; formatting is convention, ESLint + `tsc` are the enforced gates
- CI: `.github/workflows/backend-rs.yml` — Rust gates, MariaDB leg, web (`gen:api --check` → `tsc --noEmit` → `vite build`), docker + vitest + playwright

---

## Critical Implementation Rules

### Language-Specific Rules

**Rust (`backend-rs/`)**
- Crate-wide lint table in `Cargo.toml`: `missing_docs = "deny"`, `unreachable_pub = "warn"`, `unsafe_code = "forbid"`, `clippy::all = "deny"`, `clippy::pedantic = "warn"` (advisory — CI exempts pedantic with `-A clippy::pedantic` until the measured debt is drained)
- **Every public item needs a doc comment** — `missing_docs` is a build gate, not a convention
- No `unwrap()` / `expect()` / `panic!` / `todo!` / `unimplemented!` in `src/` **outside `main.rs` and `#[cfg(test)]`** — enforced by CI grep gates. Propagate errors with `?` and `AppError` instead
- No `#[tracing::instrument(skip(state))]` — `skip` takes only what it names, everything else is `Debug`-recorded (that put `auth.email` and request bodies in spans). Skip the body and session explicitly
- No `format!`-built SQL — bind with `?` placeholders. `format!` may build a **bound value** (e.g. a LIKE pattern), never SQL text
- Files/modules: `snake_case` (`store.rs`, `routes.rs`, `types.rs`); types `PascalCase`; functions `snake_case`; constants `UPPER_SNAKE_CASE`
- Errors: `thiserror` types in `src/error.rs` (`AppError` → RFC 9457 `application/problem+json`); `anyhow` only at the `main.rs` boundary

**TypeScript / React (`frontend/`)**
- `strict: true` — no `any`; type the API boundary with the generated `src/lib/api/schema.d.ts`
- Feature-based imports from `features/`, shared utilities from `lib/`, shared UI from `components/ui/`
- User-facing messages: friendly and actionable, never raw error codes or raw server payloads — map them through `src/lib/error-messages.ts`
- Frontend components `PascalCase` (`BoardView`), files `kebab-case` (`board-view.tsx`); functions `camelCase`; constants `UPPER_SNAKE_CASE`
- No `I`-prefix interfaces (there is no NestJS layer any more); no `fetch()` outside `src/lib/api/client.ts`

### Framework-Specific Rules

**React frontend**
- All server state goes through React Query (`useQuery`/`useMutation`) — no raw `fetch` in components, no data fetching in `layouts/`
- Use `apiFetch<T>()` / `apiFetchVoid()` from `@/lib/api/client`; shared response types (`ApiResponse<T>`, `Page<T>`, `ApiError`) live in `@/lib/api` and are never duplicated per feature
- Targeted cache invalidation (`['columns', boardId]`) — not global (`['columns']`) — unless all instances must be invalidated
- When two handlers read the same cache, capture the original state at interaction start in React state; do not assume the cache is unchanged between handlers
- In drag-drop, `handleDragEnd` must use the `dragSource` captured by `handleDragStart` — `getColumns()` may be stale
- Use Radix primitives (`DropdownMenu`, `Dialog`, `Select`, …) for menus/modals; never hand-roll with `useRef` + absolute positioning
- No `document.querySelector` for focus management — use React refs or callback props
- Radix `SelectItem` values must be non-empty strings; use a sentinel (e.g. `NO_PROJECT_VALUE = 'none'`) and map it back to `null`
- Lazy-load route components (`React.lazy`); the Mermaid bundle is dynamically imported
- Skeleton screens for initial loads, spinner on submit buttons, optimistic updates for drag-drop (no spinner)

**Axum backend**
- Vertical slices: one `src/features/<domain>/` per domain, each with `routes.rs` (HTTP), `store.rs` (SQL), `types.rs` (DTOs), `mod.rs`. Feature stores are free `async fn`s generic over `MySqlExecutor` — **no repository traits, no DI container**
- `AppState { pool, settings, sessions }` is the shared context, wired with `FromRef`; **no Nest-style module graph**
- Shared kernel in `src/common/` — ownership SQL (`ownership.rs`), pagination envelope, time formatting, session store, validation helpers. Features must not import each other's stores
- Request validation: `garde` `Validate` on DTOs, surfaced through the `ValidJson<T>` / `ValidQuery<T>` / `ValidPath<T>` extractors; `PaginationParams` handles `?page`/`?limit` ranges
- Auth: `AuthSession` extractor resolves the session and re-reads the user row (deleted → 401, demoted → 403). Foreign and missing IDs both yield **404, never a 403 oracle**
- Errors: every error is RFC 9457 `application/problem+json` with a stable `type` URI (`https://kanpad.dev/problems/<slug>`) and an `instance` of `"{METHOD} {path}#{x-request-id}"`. Never return axum's default plain-text rejections
- Middleware order in `src/lib.rs` is **load-bearing** (documented inline as layers 1–8). Inside-out: session manager → timeout/handle-error → request-context capture → compression → CSRF → CORS → request-id propagation → trace → request-id assignment → sensitive headers → secure response headers → metrics. Reordering changes security semantics
- CORS and CSRF must be fed from **one canonical origin list** (`settings.canonical_cors_origins()`); `CsrfLayer` sits inside CORS so preflights short-circuit at CORS
- `TRUSTED_PROXIES` gates whether `X-Forwarded-For` is honoured for the rate-limiter key. A proxy that rewrites `X-Forwarded-For` collapses per-IP throttling into one global bucket; a proxy that rewrites `Host`/`Origin` breaks the CSRF same-authority fallback — **forward `Host` unchanged**
- `APP_ENV` (`development`/`test`/`production`) is the runtime mode — **not `NODE_ENV`**. Boot **fails** when `NODE_ENV` is present so a stale deployment cannot silently lose `Secure` cookies, the `__Host-` prefix, HSTS and the deny-all CORS default
- Config is validated fail-fast at boot: `SESSION_SECRET` ≥ 32 bytes, `HASH_COST` ≥ 2 in production, `CORS_ORIGINS` non-empty and canonical in production, `DATABASE_URL` must carry `charset=utf8mb4` **and** `collation=utf8mb4_unicode_ci`
- The session cookie is `kanpad.sid` in dev/test and `__Host-kanpad.sid` in production — `HttpOnly`, `SameSite=Lax`, `Secure` in production, no `Domain`, HMAC-signed, 24 h sliding inactivity expiry
- Passwords: Argon2id for new hashes (`m=19 MiB`, `t=HASH_COST`, `p=1`); bcrypt cost-10 hashes are **verified only** and rehashed to Argon2id on successful login — never mint bcrypt. Password byte cap ≤ 72
- Rate limiting is per-IP via `tower-governor` `route_layer` (never a global `.layer`): a global per-IP ceiling on `/api/*` plus a tighter bucket on login/register only. Logout and `/auth/me` must never 429
- Swagger/OpenAPI is `utoipa` — `src/docs.rs` holds the spec; the served `openapi.json` is **byte-identical to the committed `backend-rs/openapi.json`** (CI diffs them)

### Testing Rules

**Organization**
- Frontend unit tests: co-located `*.test.ts(x)` next to the implementation (e.g. `board-view.test.tsx`)
- Frontend E2E: `frontend/e2e/*.spec.ts`
- Backend: `backend-rs/tests/api/*.rs` — in-process suites use `tests/api/helpers.rs`, black-box suites use `tests/api/wire.rs`. Fixtures in `backend-rs/tests/fixtures/*.sql`, golden response bodies in `backend-rs/tests/golden/*.json`
- **Single owner per transport**: never re-declare a helper locally; import explicitly (never glob) from `helpers.rs`/`wire.rs`. Every `assert_problem` call takes a `what` label
- A test that binds a port binds an **ephemeral** one (`wire::bind_blackbox()`) — never a fixed port
- Session rows are per-database: scope session SQL by `WHERE id = ?`, never blanket `DELETE`/`UPDATE`/`DROP`
- `#[sqlx::test]` gives each test its own migrated database; fixtures resolve relative to the **test file** directory (`../fixtures/<n>.sql`)

**What to test**
- Unit: pure helpers and DTO/validation logic in isolation; frontend components — rendering, interactions, hooks
- Integration (backend): real DB, real request/response cycle, per resource (`boards_create`, `cards_crud`, `cors`, `throttle`, `errors`, `validation`, `collation`, `unicode_roundtrip`, `sql_portability`, `proxy_headers`, `openapi_snapshot`, `golden`)
- E2E (frontend): full user journeys — login → create project → create board → add column → add card → drag → logout
- **DB-backed assertions**: for every data-modifying endpoint, assert the persisted state via a second API call or a DB query — not only the response body
- The backend must pass on **MySQL and MariaDB**: no row-alias upserts (`VALUES (…) AS new`), no `utf8mb4_0900_*` collations, no `INSERT … RETURNING`, no MySQL-only JSON operators (`->`, `->>`)

**Frontend E2E patterns**
- Test setup (creating boards, columns, cards) uses **API calls**, not UI interactions; the UI is only the behaviour under test
- Never `waitForTimeout()` — use `waitForLoadState('networkidle')`, element assertions, or `page.waitForSelector()`
- **Target state**: the E2E API is `backend-rs` on `:3001` (and `E2E_BASE_URL` pins it). ⚠️ *Not yet done*: `frontend/playwright.config.ts` still starts the deleted Nest backend (`cd ../backend && npm run start:dev`, `:3000`) and the specs still call `http://localhost:3000/api/...`. The retarget is task **T017** — CI therefore installs Chromium but does not run the suite

**Coverage & CI gates**
- Backend line coverage floor **85%** (`cargo llvm-cov --workspace --all-targets --fail-under-lines 85`)
- Rust CI gates (all must pass): `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, the four grep gates (no `unwrap`/`expect`; no `panic!`/`todo!`/`unimplemented!`; no `instrument(skip(state))`; no `format!`-built SQL), `cargo sqlx prepare --check --workspace --all -- --locked --all-targets`, `cargo deny check` (committed `deny.toml`), openapi snapshot diff, `migrate info` pending = 0, and the MariaDB 10.6 leg
- Web CI gates: `npm run gen:api -- --check` (generated types match `openapi.json`), `tsc --noEmit`, `npm run build`, `npm run test`

### Code Quality & Style Rules

**Formatting**
- Root `.prettierrc` is the shared style (`semi`, `singleQuote`, `trailingComma: all`, `printWidth: 100`, `tabWidth: 2`, `arrowParens: always`) — follow it in new code. There is **no** project-level override and **no** prettier dependency or pre-commit hook; ESLint + `tsc` are the enforced gates
- Rust: `cargo fmt` — never hand-format against rustfmt

**ESLint**
- ESLint (flat config, `frontend/eslint.config.js`) handles linting; TypeScript strict mode catches type mismatches. No `eslint-disable` without a comment explaining why

**Code organization**
- Backend: feature slices in `src/features/`, shared kernel in `src/common/`; a feature never imports another feature's `store.rs`
- Frontend: feature-based under `src/features/<domain>/` (components + hooks + api co-located), shared UI in `src/components/ui/`, layouts in `src/layouts/`, shared hooks in `src/hooks/`
- Never put business logic in `lib/` — utilities only

**Naming**
- DB tables `snake_case` plural (`boards`, `note_tags`); columns `snake_case` (`created_at`, `board_id`, `project_id`)
- API paths plural, `snake_case`, versioned: `/api/v1/boards`, `/api/v1/columns/{id}/move-cards`. `/api/*` is a **deprecated dual-mount alias** (same handlers, not redirects) — never add a new unversioned route
- **JSON keys, bodies and query params are `snake_case`** everywhere (`project_id`, `moved_count`) — the Nest-era camelCase contract is gone
- API responses: create → `201` + `Location`, read/update → `200 {data}` or `200 Page<T>`, delete → `204` with no body; no `message` strings
- `Page<T>` = `{data, page, limit, total}`; `total` comes from a real count query, never `data.length`
- Error semantics: `400` + `errors[]` for validation, `401` unauthenticated, `403` role denial only, `404` for missing **and** foreign IDs, `409` for uniqueness and wrong-state conflicts, `429` + `Retry-After` when throttled
- `?page`/`?limit` are strict: `page ≥ 1`, `1 ≤ limit ≤ 100`, otherwise `400` with `errors[]`. `?sort`/`?order` come from a per-resource whitelist

**Documentation**
- Rust: doc comments on every public item (build gate); `# Panics` sections where a panic is possible
- API: `utoipa` schemas and route docs in `src/docs.rs`; the committed `backend-rs/openapi.json` is the contract
- `docs/` holds operator-facing docs; `docs/lessons/*.md` are append-only, dated, tagged

### Development Workflow Rules

**Branches & commits**
- Feature branches `feature/{issue}`, fixes `fix/{issue}`, chores `chore/{issue}` (task branches in this repo are `rs-tNN-…` / `wt-tNN`)
- Conventional commits: `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `refactor:` — e.g. `feat(cards): add drag-drop card reordering`
- **Never commit without asking and confirming first**

**Local development**
- Backend: `cargo run -- serve` → **`:3001`** (default `PORT=3001`); `cargo run -- migrate run|info|revert`; `cargo run -- create-admin`; `cargo run -- export-openapi`
- Frontend: `npm run dev` → `:5173`; the Vite proxy sends `/api` to `VITE_API_URL`, default `http://localhost:3001` (the Nest fallback on `:3000` is dead — the backend was deleted)
- Test DB: the shared MySQL/MariaDB instance used by the Rust suites is documented in `BACKEND-RS-ORCHESTRATION.md` (export `DATABASE_URL` and the shared `CARGO_TARGET_DIR` before any cargo command)
- `.env` files are git-ignored; production values are set in CI or on the build machine

**Database workflow**
- Schema changes are **sqlx migrations** in `backend-rs/migrations/*.sql` — never hand-written DDL, never an auto-sync
- `cargo run -- migrate run` applies them; `migrate info` must report `pending: 0`
- **Never modify an applied migration** — sqlx stores a SHA-384 of each file's bytes in `_sqlx_migrations` and refuses to re-run a changed file. Drop and re-migrate the database instead; never hand-edit `_sqlx_migrations`
- Migrations never auto-run on app start: `serve` accepts `--auto-migrate` as an explicit opt-in, and it **refuses to run in production**
- A schema change ships in the same PR as: the migration, the refreshed `.sqlx/` cache, the regenerated `backend-rs/openapi.json`, and the regenerated `frontend/src/lib/api/schema.d.ts`

**API contract changes**
- `openapi.json` and the frontend's generated `schema.d.ts` are **regenerated in the same PR** as any DTO/route change (`npm run gen:api`); CI fails if they drift
- The generated type is the contract — when a generated type and hand-written code disagree, fix the hand-written code

**Deployment**
- **Two-origin topology** (see `docs/deployment.md`): the SPA is a static build (`npm run build` → `frontend/dist/`) served from its own origin; the API is the `backend-rs` container on its own origin. The SPA calls the API cross-origin via `VITE_API_URL`, baked at build time
- The API serves **no static files** and no SPA rewrite — every non-API path returns a `problem+json` 404
- Production env (`APP_ENV=production`, `CORS_ORIGINS`, `TRUSTED_PROXIES`, `SESSION_SECRET`, `HASH_COST`, `DATABASE_URL`, `DB_STARTUP_CHECK=require`) is injected at runtime, never baked into the image
- Production is MySQL **or** MariaDB — both are supported and both are exercised in CI

**PR requirements**
- All CI gates green: Rust fmt/clippy/grep/coverage/deny/openapi, the MariaDB leg, `gen:api --check`, `tsc`, `vite build`, vitest
- Schema changes include the migration; no data loss in `down`/revert paths
- New lessons go to `docs/lessons/{data-migration,rust-backend,frontend,backend-node,ci-quality}.md` (append-only, dated, tagged) or the task spec's `## Lessons` section

### Critical Don't-Miss Rules

**Anti-patterns**
- ❌ Adding a `fetch()` call outside `src/lib/api/client.ts`
- ❌ Duplicating `ApiResponse<T>` / `Page<T>` / `ApiError` in a feature API file
- ❌ Global React Query invalidation when a targeted key exists
- ❌ Using `getColumns()` inside `handleDragEnd` instead of the captured `dragSource`
- ❌ `unwrap()` / `expect()` / `panic!` in `src/` outside `main.rs` and `#[cfg(test)]`
- ❌ `format!`-built SQL, or SQL string interpolation of user input
- ❌ `#[tracing::instrument(skip(state))]` — it records request bodies
- ❌ Importing another feature's `store.rs`; cross-feature SQL helpers belong in `src/common/`
- ❌ Hand-rolled menus/modals when a Radix primitive exists
- ❌ `document.querySelector` for focus management
- ❌ `waitForTimeout()` in an E2E test
- ❌ Returning raw database errors to the client
- ❌ Editing an applied sqlx migration or `_sqlx_migrations`
- ❌ Adding an unversioned `/api/...` route or a camelCase JSON key
- ❌ Business logic in `lib/`; data fetching in `layouts/`
- ❌ Modifying `components/ui/` shadcn files without also updating `frontend/CONVENTIONS.md` lessons when the change is non-obvious

**Security**
- Never expose raw database or internal errors — everything becomes a `problem+json` response
- Markdown is sanitized **twice**: `DOMPurify.sanitize()` plus `rehype-sanitize` (notes), `rehype-sanitize` with an extended schema (card previews)
- Passwords are hashed with Argon2id; bcrypt is a legacy verification path only
- Sessions are server-side, signed, `HttpOnly` cookies; CSRF protection comes from `tower-http`'s `CsrfLayer` fed by the same canonical origin list as CORS
- Rate limiting is per-IP through `tower-governor`, keyed on the client IP that only honours `X-Forwarded-For` from `TRUSTED_PROXIES`
- Secrets live in `secrecy::SecretString`; `Settings` implements a **redacting** `Debug`; secrets are never logged
- `SESSION_SECRET` ≥ 32 bytes; `HASH_COST` ≥ 2 in production; production `CORS_ORIGINS` must be explicit, canonical, non-loopback and never `*` with credentials
- The API sets no `Content-Security-Policy` (it serves JSON only); the SPA host owns that

**Performance gotchas**
- Commit and refresh the `.sqlx/` offline cache with every query change — CI's `prepare --check` is the gate
- Use optimistic updates for drag-drop (no loading spinner on reordering)
- Memoize expensive computations; lazy-load routes and the Mermaid bundle
- Compression is `gzip` + `br` only — do not add a codec no browser accepts (it drags in a C toolchain)

**Edge cases to handle**
- **Delete cascades**: `users → projects/boards/labels/tags/notes` cascade; `boards → columns → cards → checklists/items + card_labels` cascade; `notes ↔ note_tags` cascade; **`projects → boards` is `ON DELETE SET NULL`** (deleting a project keeps its boards, unlinked)
- Empty states: no projects, no boards, no cards, no notes, no search results
- Session expiry → the API answers `401`; `src/lib/error-messages.ts` maps `401` to "Session Expired" with `redirect: '/login'`. ⚠️ *Not yet done*: no global handler performs that redirect (task **T014**), and the query client is configured with `retry: false`
- Concurrent edits: last-write-wins for a single user (MVP)
- Archiving a board is an `is_archived` flip (idempotent); deleting an **active** board is `409`
- Ownership: a foreign ID is `404`, never `403`, and never an existence oracle

**Notes system rules**
- Notes use nullable FKs (`board_id`, `project_id`, `card_id`) — all NULL = top-level note
- Tags are cross-cutting: `tags` + `note_tags` join table; `card_tags`/`project_tags`/`board_tags` can be added later
- Markdown rendering MUST be sanitized — `DOMPurify` + `rehype-sanitize` on all rendered HTML
- Mermaid diagrams render client-side only — no server-side mermaid processing
- Note auto-save: debounced 2 s after typing stops, with a "Saved" indicator
- Board view toggle preserves scroll position and filters when switching back

---

## Usage Guidelines

**For AI Agents:**
- Read this file plus `AGENTS.md` before implementing any code
- Follow ALL rules exactly as documented; when in doubt, prefer the more restrictive option
- Check the tree before trusting a spec: `specs/api-rewrite-rs-specs/10-architecture.md` mixes target state with what has actually landed — verify against `backend-rs/src/` and `frontend/src/`
- Update this file when new patterns emerge (a rule that changes behaviour belongs here as well as in `AGENTS.md`)

**For Humans:**
- Keep it lean and agent-focused; update when the technology stack changes
- Review quarterly for outdated rules; remove rules that become obvious over time

Last Updated: 2026-11-11
