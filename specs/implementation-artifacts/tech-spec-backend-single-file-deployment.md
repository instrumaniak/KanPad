---
title: 'Backend Single-File Deployment Build'
slug: 'backend-single-file-deployment-build'
created: '2026-05-13T00:00:00.000Z'
status: 'implementation-complete'
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8, 9]
tech_stack: ['NestJS 11', 'TypeScript', 'TypeORM 0.3', 'MySQL 8', 'webpack', 'bcrypt']
files_to_modify:
  [
    'backend/package.json',
    'backend/nest-cli.json',
    'backend/src/app.module.ts',
    'backend/src/data-source.ts',
    'backend/src/database/typeorm-registry.ts',
    'backend/src/database/data-source-options.ts',
    'backend/src/scripts/migrate.ts',
    'backend/src/scripts/create-admin.ts',
    'backend/src/scripts/prepare-release.js',
    'backend/src/scripts/verify-release.js',
    'backend/webpack.config.js',
  ]
code_patterns:
  [
    'NestJS webpack bundling via nest build --webpack',
    'shared explicit entity registry used by both app and data-source',
    'shared DataSourceOptions builder to eliminate config triplication',
    'explicit migration class imports bundled into a single migration runner file',
    'webpack splitChunks to deduplicate shared code across app, migrate, and create-admin bundles',
    'native addon kept external with minimal runtime node_modules',
    'Node.js release helper scripts instead of shell echo for cross-platform safety',
  ]
test_patterns:
  [
    'build smoke test for all bundled artifacts (app, migrate, create-admin)',
    'release artifact verification script (file presence, non-empty, native deps, shared chunks)',
    'runtime smoke test against local test database',
    'verify migration side effects in database, not only CLI output',
    'idempotency test: re-running migration bundle on already-migrated database exits cleanly',
    'create-admin bundle smoke test (help flag and argument parsing)',
  ]
---

# Tech-Spec: Backend Single-File Deployment Build

**Created:** 2026-05-13

## Overview

### Problem Statement

The current NestJS backend ships as a standard compiled `dist/` tree and relies on TypeORM filesystem globs to discover entities and migrations. That works for local development, but it is not the right shape for a compact deployment artifact or a lightweight Docker image. We also need separate, manually runnable CLI artifacts for SSH-based production operations: migration runner and admin bootstrapping.

### Solution

Produce a deployment-focused backend release layout that contains:

1. A bundled JavaScript file for the API server (`app.js`).
2. A bundled JavaScript file for running TypeORM migrations manually (`migrate.js`).
3. A bundled JavaScript file for creating the initial admin user in production (`create-admin.js`).
4. Shared chunks extracted by webpack `splitChunks` so common code (TypeORM, mysql2, entities, utilities) is not duplicated across bundles.
5. A minimal runtime `node_modules` set only for native or unavoidable runtime dependencies.

The app bundle follows NestJS's webpack bundling path. TypeORM metadata moved away from `__dirname` glob discovery to a shared explicit entity registry so all bundles use the same source of truth. The migration runner and admin creator bundle all their dependencies via explicit imports into single self-contained files that initialize the `DataSource`, perform their operation, and exit with a non-zero status on failure so they can be run manually over SSH with no source tree present.

### Scope

**In Scope:**
- Add a webpack-based backend release build.
- Bundle the API server into one executable JavaScript file.
- Add a second bundle for manual migration execution.
- Add a third bundle for production admin bootstrapping.
- Deduplicate shared code across all bundles using webpack `splitChunks`.
- Keep native runtime dependencies available in a minimal distribution folder.
- Replace filesystem-glob entity discovery with explicit entity registration.
- Preserve the existing TypeORM migration workflow and migration table behavior.
- Keep `migrationsRun: false` in the API runtime.

**Out of Scope:**
- Frontend build changes.
- Dockerfile authoring unless needed later to consume the release output.
- Changing the database engine or ORM.
- Auto-running migrations at application startup.
- Rewriting the admin bootstrap script's business logic.

## Context for Development

### Codebase Patterns

- The backend is a NestJS 11 app with TypeScript strict mode and TypeORM 0.3.
- Domain modules already register their entities with `TypeOrmModule.forFeature(...)`.
- The app module currently wires TypeORM through `TypeOrmModule.forRootAsync(...)`.
- `src/data-source.ts` is already the shared TypeORM CLI data source.
- Migrations live in `backend/src/migrations/` and are compiled into `dist/migrations/`.
- `bcrypt` is the only clearly native runtime dependency in the backend; `mysql2` is pure JS.
- Current deployment guidance in the project context already expects migrations to run during deployment.

### Files to Reference

| File | Purpose |
| ---- | ------- |
| `backend/src/app.module.ts` | Current TypeORM app bootstrap and session store wiring |
| `backend/src/data-source.ts` | Current TypeORM CLI data source for migration commands |
| `backend/src/migrations/*` | Existing migration classes that must still run in production |
| `backend/src/scripts/create-admin.ts` | CLI entrypoint for bootstrapping the first admin user |
| `backend/package.json` | Current build and migration scripts to extend or replace |
| `backend/src/main.ts` | API server bootstrap that will become the bundled entrypoint |

### Technical Decisions

1. Use NestJS webpack bundling for the production API artifact, not a separate packager.
2. Keep one release artifact per runtime concern: `app.js` for the API, `migrate.js` for manual migration runs, and `create-admin.js` for production admin bootstrapping.
3. Move from filesystem entity discovery to a shared explicit entity registry so all runtime paths use the same source of truth.
4. Bundle migration classes into `migrate.js` via explicit imports in the migration entrypoint so the migration runner is a single self-contained file with no runtime dependency on a migration directory.
5. Use webpack `optimization.splitChunks` with `chunks: 'all'` to extract shared code (TypeORM, mysql2, entities, utilities) into a `common.js` chunk. This reduces total release size and prevents code duplication across the three bundles.
6. Treat `bcrypt` as the native runtime dependency that must be present in the minimal distribution folder.
7. Leave `migrationsRun: false` in the API runtime so DB changes remain an explicit deployment step.
8. Build and package on the same platform family as production so native module binaries match the target runtime.

## Implementation Plan

### Tasks

1. Add a backend release build path in `backend/package.json`.
   - Add scripts for an app bundle build, a migration bundle build, and an admin bundle build.
   - Keep the existing dev-oriented TypeORM commands intact unless they conflict with the release path.
2. Add a webpack configuration file at `backend/webpack.config.js`.
   - Configure a Node target.
   - Emit JS files for each entrypoint: `release/app.js`, `release/migrate.js`, and `release/create-admin.js`.
   - Configure `optimization.splitChunks` to extract shared modules into `release/common.js`.
   - Bundle regular JS dependencies.
   - Externalize `bcrypt` so the minimal runtime folder can carry the native addon explicitly.
3. Refactor `backend/src/app.module.ts`.
   - Replace glob-based entity loading with explicit entity registration.
   - Include the session entity explicitly because it is not covered by a `forFeature()` module.
   - Keep migration execution disabled in the app runtime.
4. Refactor `backend/src/data-source.ts`.
   - Import the explicit entity list and migration array from the shared registry (`backend/src/database/typeorm-registry.ts`).
   - Remove any glob-based migration or entity discovery.
   - Ensure the data source can be initialized independently for both CLI and bundled contexts.
5. Add a dedicated migration CLI entrypoint, `backend/src/scripts/migrate.ts`.
   - Import all migration classes explicitly from the shared registry.
   - Initialize the `DataSource` with the explicit entity and migration arrays.
   - Run pending migrations.
   - Exit non-zero on failure so SSH/manual execution is reliable.
6. Refactor `backend/src/scripts/create-admin.ts` for bundling.
   - Replace the inline `dataSourceOptions` object with `buildDataSourceOptions()` from the shared config.
   - Remove the `__dirname` glob migration pattern (not needed for admin creation).
   - Fix unsafe `process.exit()` calls to use `process.exitCode` with proper `finally` blocks.
7. Define the release folder assembly process.
   - Copy the bundled JS files (`app.js`, `migrate.js`, `create-admin.js`, and shared chunks) into `release/`.
   - Copy only the minimal runtime `node_modules` needed for native or externalized packages (`bcrypt` and its transitive deps).
   - No migration files need to be copied because they are bundled into `migrate.js`.
8. Validate the release artifacts against a local database.
   - Confirm the bundled API starts successfully.
   - Confirm the migration runner can apply pending migrations.
   - Confirm the create-admin bundle can parse arguments and show help.
   - Confirm migrated schema changes are visible in the database.

### Acceptance Criteria

1. **Given** the backend release build completes, **When** the deployment artifact is inspected, **Then** it contains `release/app.js`, `release/migrate.js`, `release/create-admin.js`, and shared chunks (`release/common*.js`) plus only the minimal runtime dependencies required at execution time.
2. **Given** the bundled API file is copied to a Node.js-compatible server with its minimal runtime dependencies, **When** the process starts with the required environment variables, **Then** the API boots successfully and serves requests.
3. **Given** the bundled migration file is copied to the deployment server, **When** it is executed manually over SSH with no source tree present, **Then** it applies all pending TypeORM migrations and exits successfully.
4. **Given** the bundled create-admin file is copied to the deployment server, **When** it is executed with `--help`, **Then** it displays usage information and exits successfully.
5. **Given** the API bundle starts in production mode, **When** the app initializes, **Then** it does not auto-run migrations.
6. **Given** a migration is applied through the migration runner, **When** the database is queried afterward, **Then** the schema changes are actually present, not just reported in CLI output.
7. **Given** the release build is prepared for a lightweight Docker image, **When** the image is assembled, **Then** it can omit the full source tree and keep only the release files plus the minimal native runtime dependency set.
8. **Given** the three bundles share a large amount of code, **When** the release build completes, **Then** the total size of all bundles plus shared chunks is smaller than the sum of three independent bundles.

## Additional Context

### Dependencies

- NestJS webpack bundling support.
- TypeORM compiled-JS migration execution.
- Native module handling for `bcrypt`.
- `inquirer` for interactive admin creation prompts.

### Testing Strategy

- Build the release artifacts locally and confirm all three bundles plus shared chunks are emitted.
- Start the bundled API against a test database and hit a basic endpoint.
- Run the migration bundle against a clean test database with no `migrations/` directory present and verify the migration table plus schema changes.
- Run the create-admin bundle with `--help` and verify output.
- Confirm the release output does not depend on the source tree at runtime for entity or migration discovery.
- Verify shared chunks are loaded correctly by all bundles.

### Resolved Findings

Post-implementation adversarial review produced 12 findings. The following were fixed automatically:

| Finding | Severity | Resolution |
|---|---|---|
| F1: Triplicated DataSource config | Critical | Extracted `buildDataSourceOptions()` into `database/data-source-options.ts`; used by app, CLI data source, and migration runner. |
| F2: Unsafe `process.exit()` in migrate.ts | High | Rewrote with `finally` block, protected `destroy()` with its own try/catch, and sets `process.exitCode` instead of calling `process.exit()`. |
| F4: Hardcoded bcrypt version | Medium | Replaced shell `echo` with `scripts/prepare-release.js` which reads the actual `bcrypt` version from `package.json`. |
| F5: Misleading filename | Low | Renamed `database/entities.ts` to `database/typeorm-registry.ts` to reflect both entities and migrations exports. |
| F6: `migrations: []` in app module | Low | App module now includes the full migrations array via `buildDataSourceOptions()`; `migrationsRun: false` still prevents auto-run. |
| F7: No npm script for migration bundle | Low | Added `migrate:release` script to `package.json`. |
| F9: Stale artifacts in `release/` | Low | `build:release` now runs `rm -rf release/` before webpack. |
| F10: `.gitignore` exclusion | Low | Added `release/` and `backend/release/` to `.gitignore`. |
| F11: Shell-fragile `release:install` | Medium | Replaced shell command with cross-platform `scripts/prepare-release.js` Node script. |
| F12: No automated tests for build artifacts | High | Added `scripts/verify-release.js` and `test:release` npm script that checks file presence, size, and native dependency availability. |

Skipped by design:
- **F3:** Only `bcrypt` externalized — intentional; `mysql2` is pure JS and bundles reliably.
- **F8:** Manual migration registration — required because webpack cannot bundle dynamic glob patterns.

### Notes

- The existing backend already has a deployment-oriented migration story in the architecture docs; this spec makes it release-artifact friendly.
- The biggest implementation risk is breaking TypeORM discovery by leaving any runtime path scan in place. Bundling migrations into `migrate.js` via explicit imports eliminates the migration-directory runtime dependency entirely.
- The minimal runtime `node_modules` should stay intentionally small; only native or runtime-externalized packages should remain there.
- If the native addon strategy becomes unstable across deployment platforms, the fallback is to rebuild `bcrypt` in the same target environment as the release artifact.
- Deduplication via `splitChunks` requires all bundles to be present in the same directory at runtime so they can `require()` the shared chunks. This is guaranteed by the release build process.

---

## Update: Deployment-Ready Enhancements (2026-05-15)

### Overview

Following the initial single-file deployment build, three additional requirements were identified to make the artifact truly deployment-ready for a shared cPanel hosting workflow:

1. **Eliminate native runtime dependencies** so the release folder contains zero `node_modules`.
2. **Serve the frontend SPA from the backend bundle** so a single artifact handles both API and UI.
3. **Enforce strict `/api` routing separation** so frontend routes and API routes never collide.

### Changes Summary

#### 1. Replace `bcrypt` with `bcryptjs` (Pure JavaScript)

**Motivation:** `bcrypt` contains a native C++ addon that must be compiled for the target platform. This forces the release artifact to carry a `node_modules/` folder with platform-specific binaries. `bcryptjs` is a pure-JavaScript implementation with identical API surface, eliminating the native dependency entirely.

**Files modified:**
| File | Change |
|---|---|
| `backend/package.json` | Removed `bcrypt` and `@types/bcrypt`. Added `bcryptjs` `^3.0.2` and `@types/bcryptjs` `^2.4.6`. |
| `backend/src/auth/auth.service.ts` | `import * as bcrypt from 'bcrypt'` → `import * as bcrypt from 'bcryptjs'` |
| `backend/src/auth/auth.service.spec.ts` | Same import change; `jest.mock('bcrypt')` → `jest.mock('bcryptjs')` |
| `backend/src/scripts/create-admin.ts` | `import bcrypt from 'bcrypt'` → `import bcrypt from 'bcryptjs'` |
| `backend/webpack.config.js` | Removed `bcrypt` externalization block (`externals` now empty array). `bcryptjs` bundles inline. |
| `backend/src/scripts/prepare-release.js` | Removed all `bcrypt`-specific logic and `npm install --production`. No `package.json` generation needed. |
| `backend/src/scripts/verify-release.js` | Removed `node_modules/bcrypt` from `requiredDirs`. |

**Result:** The `backend/release/` folder no longer contains any `node_modules/` directory. The release artifact is purely JavaScript bundles + frontend static assets.

#### 2. Enforce `/api` Prefix for All Backend Routes

**Motivation:** To create deterministic, collision-free routing, every API endpoint must live under `/api/*`. This allows the static file serving and SPA fallback to cleanly own every non-API path without complex exclusion lists.

**Files modified:**
| File | Change |
|---|---|
| `backend/src/users/users.controller.ts` | `@Controller('users')` → `@Controller('api/users')` |
| `backend/src/app.controller.ts` | Removed `@Get()` root route (`Hello World!`). Controller class retained for module wiring. |

**Result:** All functional API routes are now strictly under `/api/*`. No backend route competes with frontend paths.

#### 3. Add Health Check Endpoint

**Motivation:** After removing the root `GET /` endpoint, there was no lightweight endpoint to verify the API is running. A dedicated health check is useful for monitoring and quick validation.

**Files modified:**
| File | Change |
|---|---|
| `backend/src/app.controller.ts` | Added `@Get('api/health')` returning `{ status: 'ok' }`. |
| `backend/src/app.controller.spec.ts` | Updated test to assert `getHealth()` returns `{ status: 'ok' }`. |
| `backend/test/app.e2e-spec.ts` | Updated e2e test to hit `/api/health` instead of `/`. |

#### 4. Serve Frontend Static Build from Backend

**Motivation:** The deployment workflow is: build backend → build frontend → copy frontend `dist/` into backend release folder → zip the release folder → upload to cPanel. The backend bundle must serve the frontend SPA so a single Node.js process handles both API and UI.

**How it works:**
1. `backend/src/main.ts` checks for a `public/` folder relative to the bundle at startup.
2. If `public/` is missing, the app logs a fatal error and exits immediately. This prevents deploying an API-only bundle when a full-stack deployment was intended.
3. If `public/` exists, `express.static()` serves all static files (JS, CSS, images) directly.
4. An `app.use()` middleware registered after `express.static()` catches all non-API routes and serves `public/index.html`. This handles React Router's `BrowserRouter` client-side routes (`/login`, `/board/123`, etc.).
5. API routes (`/api/*`) are excluded from the fallback via a path prefix check.

**Files modified:**
| File | Change |
|---|---|
| `backend/src/main.ts` | Added `express.static()` serving + SPA fallback middleware. Fatal error if `public/` missing. |
| `backend/src/scripts/prepare-release.js` | **Always builds the frontend first** (`npm run build` in `../../frontend/`), then copies `frontend/dist/` contents into `release/public/`. If frontend build fails, exits with code 1. |
| `backend/src/scripts/verify-release.js` | Added `public/` to `requiredDirs`. Added hard requirement for `public/index.html` — verification fails if missing or empty. |

**Request routing determinism after changes:**

| Path Pattern | Handler |
|---|---|
| `/api/*` | NestJS API controllers |
| `/api/docs` | SwaggerModule |
| `/api/health` | Health check (AppController) |
| Any file in `public/` matching the path | `express.static()` |
| Any other path | SPA fallback → `public/index.html` |

#### 5. Frontend Build Fixes (Blocking Release Build)

During the implementation, two pre-existing TypeScript errors in the frontend blocked the release build because `prepare-release.js` now always builds the frontend. These were fixed:

| File | Issue | Fix |
|---|---|---|
| `frontend/src/features/cards/card.tsx` | `duration` property not defined in toast type | Removed `duration: 30000` from toast call |
| `frontend/src/features/columns/column-header.tsx` | `boardId` property not defined in move mutation type | Removed `boardId: column.board_id` from mutation call |

#### 6. E2E Test Fixes

| File | Change |
|---|---|
| `backend/test/create-admin.e2e-spec.ts` | Updated expected help text from `"Usage: npm run create-admin"` to `"Usage: node create-admin.js"` to match actual CLI output. |

### Updated Release Folder Structure

```
backend/release/
  app.js                 # API server bundle
  migrate.js             # Migration runner bundle
  create-admin.js        # Admin bootstrap bundle
  common.js              # Shared chunks (TypeORM, entities, etc.)
  4.js, 5.js             # Additional webpack split chunks
  public/                # Frontend build output
    index.html
    assets/
      index-xxx.js
      index-xxx.css
    favicon.svg
    icons.svg
```

**No `node_modules/` directory.**

### Updated Acceptance Criteria

9. **Given** the backend release build completes, **When** the deployment artifact is inspected, **Then** it contains JS bundles, shared chunks, and a `public/` folder with `index.html`, and **no** `node_modules/` directory.
10. **Given** the bundled API starts with its `public/` folder present, **When** a request is made to a non-API path (e.g., `/login` or `/board/123`), **Then** the backend serves `public/index.html` so React Router can handle client-side routing.
11. **Given** the bundled API starts without a `public/` folder, **When** the process initializes, **Then** it logs a fatal error and exits with code 1.
12. **Given** any backend controller, **When** its route prefix is inspected, **Then** it is under `/api/*` (with the exception of Swagger docs, also under `/api/docs`).
13. **Given** the health endpoint is called, **When** `GET /api/health` is requested, **Then** it returns `{ status: 'ok' }` with HTTP 200.

### Testing Results

All tests pass after the enhancements:
- **Backend unit tests:** 172 passed / 172 total
- **Backend e2e tests:** 49 passed / 49 total
- **Frontend unit tests:** 223 passed / 223 total
- **Release build verification:** `npm run test:release` passes with all required files and `public/index.html` present.

---

## Update: Fastify Platform Migration (2026-05-15)

### Overview

The user requested switching the backend HTTP platform from Express to Fastify for maximum performance, while keeping the codebase as platform-independent as possible (no Express-specific APIs in NestJS controllers). Additionally, all runtime dependencies must be pure JavaScript — no native modules.

### Changes Summary

#### 1. Replace Express with Fastify Platform

**Motivation:** Fastify is a high-performance, low-overhead web framework. It is pure JavaScript with no native dependencies, aligning with the deployment goal of zero native modules.

**Dependencies changed:**
| Action | Package |
|---|---|
| Removed | `@nestjs/platform-express`, `express-session`, `cookie-parser`, `@types/cookie-parser`, `@types/express-session`, `@types/express` |
| Added | `@nestjs/platform-fastify`, `@fastify/static`, `@fastify/cookie`, `@fastify/session`, `@fastify/view` |

**Files modified:**
| File | Change |
|---|---|
| `backend/package.json` | Updated dependency list (see above). |
| `backend/src/main.ts` | Complete rewrite: uses `FastifyAdapter`, registers `@fastify/cookie` + `@fastify/session` with `TypeormStore`, registers `@fastify/static` for `public/`, adds `setNotFoundHandler` for SPA fallback. Session plugin registered **before** `app.init()` so all controller routes have session support. |
| `backend/src/app.module.ts` | Removed `implements NestModule`, `configure()` method, and all Express-specific middleware imports (`cookie-parser`, `express-session`). |
| `backend/src/auth/auth.controller.ts` | Replaced `import type { Response } from 'express'` with platform-agnostic `@Res({ passthrough: true }) res: any`. |

**Result:** The backend now runs on Fastify. Controllers remain platform-agnostic (using NestJS decorators). The only platform-specific code is in `main.ts` (plugin registration) and the cookie clearing in `auth.controller.ts`.

#### 2. Fastify Session Store Configuration

**Key configuration details:**
- `@fastify/session` requires `secret` to be at least 32 characters. Updated default dev secret to `'kanbanflow-dev-secret-key-must-be-32-chars'`.
- `cookieName: 'connect.sid'` is explicitly set to match the existing cookie name used by the frontend and Swagger docs.
- Session store (`TypeormStore` from `connect-typeorm`) is compatible with `@fastify/session` because the plugin supports Express session store interfaces.

#### 3. SPA Fallback with Fastify

**Approach:**
1. Register `@fastify/static` with `wildcard: false` (only serves explicit files, no automatic directory index).
2. Use `fastify.setNotFoundHandler()` to catch all unmatched routes.
3. In the handler: if path starts with `/api/`, return 404 JSON. Otherwise, serve `public/index.html` via `res.sendFile()`.

**Code pattern (platform-specific, isolated to `main.ts`):**
```typescript
const fastify = app.getHttpAdapter().getInstance();
fastify.setNotFoundHandler((req: FastifyRequest, res: FastifyReply) => {
  if (req.url?.startsWith('/api/')) {
    res.code(404).send({ message: 'Not Found' });
    return;
  }
  res.sendFile('index.html', publicPath);
});
```

#### 4. E2E Test Updates for Fastify

**Challenge:** `supertest` works with Express apps directly but requires an actual listening server URL with Fastify.

**Solution:**
- All e2e tests now create the app with `new FastifyAdapter()`.
- `await app.listen(0)` is called after `await app.init()`.
- `request(app.getUrl())` is used instead of `request(app.getHttpServer())`.
- A shared test utility `backend/test/test-utils.ts` provides `setupFastifySession(app)` to register cookie/session plugins before `app.init()`.
- `app.getUrl()` returns a `Promise<string>` with Fastify, so tests `await` it and store in a `url` variable.

**Files modified:**
| File | Change |
|---|---|
| `backend/test/test-utils.ts` | New file: `setupFastifySession()` helper for e2e tests. |
| `backend/test/app.e2e-spec.ts` | FastifyAdapter, `app.listen(0)`, `await app.getUrl()`, `request(url)`. |
| `backend/test/auth.e2e-spec.ts` | Same pattern + `setupFastifySession()` for session-backed auth tests. |
| `backend/test/boards.e2e-spec.ts` | Same pattern. |
| `backend/test/cards.e2e-spec.ts` | Same pattern + agent-based tests use `request.agent(url)`. |
| `backend/test/columns.e2e-spec.ts` | Same pattern + agent-based tests. |
| `backend/test/projects.e2e-spec.ts` | Same pattern. |

#### 5. Webpack Build Compatibility

**Issue:** `@nestjs/platform-fastify` has an optional peer dependency on `@fastify/view` which webpack tries to resolve during bundling.

**Fix:** Installed `@fastify/view` as a runtime dependency. It is pure JavaScript and bundles cleanly.

**Issue:** TypeScript type errors with `app.register(fastifyCookie)` and `app.register(fastifySession)`.

**Fix:** Used dynamic `await import(...)` for Fastify plugins and cast to `any` for `app.register()` calls. This keeps the code functional while bypassing strict type mismatches between Fastify plugin versions and NestJS adapter types.

### Updated Routing Determinism (Fastify)

| Path Pattern | Handler |
|---|---|
| `/api/*` | NestJS API controllers (Fastify platform) |
| `/api/docs` | SwaggerModule |
| `/api/health` | Health check (AppController) |
| Any file in `public/` matching the path | `@fastify/static` explicit route |
| Any other path | `setNotFoundHandler` → `public/index.html` |

### Updated Acceptance Criteria

14. **Given** the backend starts, **When** the HTTP platform is inspected, **Then** it is Fastify (not Express), with no Express-specific middleware or imports in controllers.
15. **Given** any backend controller, **When** it uses platform-specific APIs, **Then** none exist — all platform-specific code is isolated to `main.ts`.
16. **Given** the session secret is not configured in production, **When** the app initializes, **Then** it throws an error (secret must be >= 32 chars for `@fastify/session`).

### Testing Results

All tests pass after Fastify migration:
- **Backend unit tests:** 172 passed / 172 total
- **Backend e2e tests:** 49 passed / 49 total
- **Frontend unit tests:** 223 passed / 223 total
- **Release build:** Webpack bundles successfully; `npm run test:release` passes.

### Notes

- `@fastify/session` stores sessions on `request.session`, which NestJS's `@Session()` decorator reads correctly once the plugin is registered before route initialization.
- The `cookieName: 'connect.sid'` setting ensures existing frontend code and browser cookies continue to work without changes.
- Fastify's `setNotFoundHandler` is global and runs after all explicit routes (including `@fastify/static` explicit file routes). This makes the SPA fallback reliable without interfering with static assets or API endpoints.

---

## Update: Compression and Security Middleware (2026-05-15)

### Overview

The user requested adding response compression and security middleware to the Fastify backend. For Express, `helmet` and `compression` are the standard choices. For Fastify, the equivalents are `@fastify/helmet` and `@fastify/compress` — both pure JavaScript, no native dependencies.

### Changes Summary

#### 1. Response Compression (`@fastify/compress`)

**Motivation:** Compress API responses and other text-based content to reduce bandwidth and improve perceived performance, especially on slower networks (relevant for cPanel shared hosting).

**Package:** `@fastify/compress`
- Pure JavaScript, no native dependencies.
- Supports gzip and brotli automatically (negotiated via `Accept-Encoding` header).
- Applied globally to all responses.

**Registration:**
```typescript
const fastifyCompress = (await import('@fastify/compress')).default;
await app.register(fastifyCompress as any, {
  global: true,
});
```

**Placement in `main.ts`:** After session setup, before SwaggerModule and static file serving.

#### 2. Security Headers (`@fastify/helmet`)

**Motivation:** Protect against common web vulnerabilities by setting security-related HTTP headers (XSS, clickjacking, content sniffing, etc.).

**Package:** `@fastify/helmet`
- Fastify port of the well-known `helmet` middleware.
- Pure JavaScript, no native dependencies.
- Sets headers like `X-Content-Type-Options`, `X-Frame-Options`, `Strict-Transport-Security`, etc.

**Registration:**
```typescript
const fastifyHelmet = (await import('@fastify/helmet')).default;
await app.register(fastifyHelmet as any, {
  contentSecurityPolicy: false,
});
```

**Why `contentSecurityPolicy: false`:** The frontend is a React SPA served as static files. CSP would block inline scripts/styles generated by the build process. Disabling CSP is the pragmatic choice for a SPA deployment; other security headers (XSS, framing, MIME sniffing) are still enforced.

**Placement in `main.ts`:** After session setup, before compression, SwaggerModule, and static file serving.

**Files modified:**
| File | Change |
|---|---|
| `backend/package.json` | Added `@fastify/compress` and `@fastify/helmet` to dependencies. |
| `backend/src/main.ts` | Added `fastifyHelmet` and `fastifyCompress` registration after session setup. |
| `backend/test/test-utils.ts` | Added `fastifyHelmet` and `fastifyCompress` registration in `setupFastifySession()` so e2e tests match production behavior. |

### Updated Acceptance Criteria

17. **Given** an API request returns text-based content, **When** the response headers are inspected, **Then** it includes `Content-Encoding: gzip` (or `br` for brotli), indicating compression is active.
18. **Given** any HTTP response from the backend, **When** the security headers are inspected, **Then** it includes `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, and other standard helmet headers.

### Testing Results

All tests pass after adding compression and helmet:
- **Backend unit tests:** 172 passed / 172 total
- **Backend e2e tests:** 49 passed / 49 total
- **Release build:** Webpack bundles successfully; `npm run test:release` passes.

### Notes

- `@fastify/compress` with `global: true` automatically compresses all text-based responses. Binary files (images, already-compressed assets) are skipped.
- `@fastify/helmet` runs before routes, so all responses (API, static, SPA fallback) get security headers.
- Both plugins are pure JavaScript and bundle cleanly into the webpack output.
