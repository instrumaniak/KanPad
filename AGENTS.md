## Project Specification References

- `specs/project-context.md`
- `specs/planning-artifacts/prd.md`
- `specs/planning-artifacts/architecture.md`
- `specs/planning-artifacts/ux-design-specification.md`
- `specs/planning-artifacts/epics.md`
- `specs/implementation-artifacts/sprint-status.yaml`

## General Agent Instructions

- Plan the implementation including edge cases, verify your plan by web search or documentation.
- TDD - do Test driven development. check coverage.
- Task Breakdown: Always break tasks into smaller, trackable todo items using `todowrite`.
- Parallel Independent Task Execution with context: Use sub-agents for tasks that can be independently done in a background process.
- after implementation check for errors, run tests, fix linting or type check error
- Follow project coding conventions & community best practices.
- add the lessons you learned while implementing tasks & fixing issues in this file (AGENTS.md) so that next time you are better prepared.

## DB data & migration safety first

- development, testing, production - all must follow the exact db migration patterns for safe, reproducible data persistence behavior.

## Backend E2E Test Pattern: Must Always Verify DB Persists

- For ALL data modification endpoints, tests MUST verify actual database state, not just API response.
- For every PATCH/POST/DELETE test, add ONE additional assertion that verifies persistence via a different API call or DB query.

## Frontend API Layer Conventions

- API response types (`ApiResponse<T>`, `ListResponse<T>`, `ApiError`) must be defined in `@/lib/api`, not duplicated across feature API files.
- Use `apiFetch<T>()` helper instead of raw `fetch()` + `handleResponse()` in API files.
- Don't copy-paste `FETCH_OPTIONS` or `handleResponse` into each API file.

## React Query Conventions

- Use targeted cache invalidation (`['columns', boardId]`) not global (`['columns']`) unless you need to invalidate all instances.
- When multiple handlers read from the same cache, capture original state at interaction start in React state. Don't assume cache is unchanged between handlers.
- In drag-drop code, `handleDragEnd` must use `dragSource` state from `handleDragStart`, not `getColumns()` which may be stale.

## Frontend E2E Test Patterns

- Test setup (creating boards, columns, cards) must use API calls, not UI interactions. UI is only for the behavior under test.
- Never use `waitForTimeout()` in E2E tests. Use `waitForLoadState('networkidle')`, element assertions, or `page.waitForSelector()` instead.

## Frontend Code Quality

- Use Radix UI components (`DropdownMenu`, `Dialog`) for menus and modals instead of manual implementations with `useRef` and absolute positioning.
- Don't use `document.querySelector` for focus management - use React refs or callback props instead.

## Rust Backend Test Harness

- **Single owner per transport.** In-process suites (`axum::Router` + `tower::ServiceExt::oneshot`) get all plumbing from `backend-rs/tests/api/helpers.rs`. Black-box suites (raw TCP) get the transport-independent pieces from `backend-rs/tests/api/wire.rs`. Never re-declare a helper locally — the ownership header in `helpers.rs` lists the exact forbidden set AND the sanctioned exceptions.
- **A suite MUST NOT reuse a name from the forbidden set.** The forbidden set is scoped to the IN-PROCESS suites; a local helper is only allowed when it is genuinely suite-specific, and then it must (a) take a name outside the forbidden set and (b) be listed in the header's "Sanctioned exceptions". A suite that shadows a shared name (`json_body`, `send`, `register`, `json_of`, `cookie_jar`, …) is a silent drift even when it compiles — that is how `auth_register.rs`, `throttle.rs` and `e2e_rest.rs` ended up with near-duplicate plumbing.
- **Import EXPLICITLY, never glob.** `use super::helpers::{app_on_pool, send, …}` — a glob import lets a leftover local definition silently shadow the shared one, while an explicit import makes it a hard `E0255` compile error. That compile error IS the regression guard; a glob import removes it.
- **Every `assert_problem` call takes a `what` label** naming the failing step, so a failure identifies the scenario instead of a line number. Keep the label short and specific.
- **Wire vs in-process problem assertions share one core.** `helpers::assert_problem_fields` is the single assertion core; `helpers::assert_problem` and `wire::assert_problem` are thin adapters that only differ in how they read the `Content-Type`. Add new problem assertions to the core, never to one adapter. Same pattern for the cookie jar: `helpers::cookie_jar_from` is the core, `helpers::cookie_jar` / `wire::cookie_jar` are the per-transport adapters.
- **Pick the shared variant, don't re-implement it.** `helpers::send` (owned router), `helpers::send_ref` (borrowed router — for a suite driving ONE app repeatedly, e.g. the throttle flood probes), `helpers::send_json` (pre-parsed body). The wire-flavored body parser is `WireResponse::json(what)`. If a suite needs a fourth shape, add it to `helpers.rs`/`wire.rs` — do not write a local copy.
- **Session rows are per-database, not server-global.** The store follows the pool's database (`AppState::with_pool`), so under `#[sqlx::test]` each test has its own `sessions` table. Never blanket `DELETE`/`UPDATE`/`DROP` it — scope session SQL by `WHERE id = ?`.
- **Never hardcode the session table's schema/table names in a test.** Use the unqualified `sessions` (the app database) — the store's own defaults (`tower_sessions`.`session`) are NOT what the app uses.
- **A test that binds a port binds an EPHEMERAL one.** Use `wire::bind_blackbox()` (port 0 → OS-assigned) and drive `listener.local_addr()`; never a fixed `:3001`. Cargo runs the suites in one binary in parallel, so a shared fixed port collides, and it also breaks when a dev server already holds that port.
- **Editing an applied sqlx migration is a checksum break.** sqlx stores a SHA-384 of each migration's bytes in `_sqlx_migrations` and refuses to re-run a migration whose bytes changed (`VersionMismatch`). A DB migrated with the old baseline must be DROPPED and re-migrated — never hand-edit `_sqlx_migrations`. The baseline is a squash (cutover always builds a fresh DB), so an empty target is unaffected.

## Lessons Learned

- Backend shared list-enrichment (`has_description`, `checklist_progress`, card summary select) lives in `backend/src/common/card-summary.helpers.ts` as pure functions taking `Repository<Card>`. Don't reintroduce per-service copies in `cards.service.ts` / `columns.service.ts`.
- Don't place shared helpers inside `cards/` or `columns/` feature folders — cross-feature imports risk circular deps (`arch-avoid-circular-deps`). Use neutral `src/common/`.
- Backend unit-spec mocks: avoid `as any` (triggers `@typescript-eslint/no-unsafe-*`). Use `as unknown as Repository<T>` + `jest.Mock` typed helpers, and assert on the mock variable directly instead of `expect(repo.method)` (avoids `@typescript-eslint/unbound-method`).
- Frontend board create/edit shares one `BoardFormModal(mode)` + top-level `BoardFormFields` in `features/boards/board-form-modal.tsx`. Don't reintroduce `InlineEditForm` in `board-card.tsx` or per-mode form copies. Reset edit state via `key={edit-${id}}` remount, not `useEffect` prop-to-state sync. `BoardCard.onEdit` passes full `Board` (incl. `project_id`), modal self-fetches projects via `useProjects`.
- Backend `Board` maps `project_id` twice (`@Column` + `@ManyToOne/@JoinColumn` on the same column). `save()` persists the FK from the **relation side**: when updating `project_id`, always set `board.project` too (entity or `null`), never the FK column alone. Ownership checks must use an injected `Repository<Project>` (`TypeOrmModule.forFeature` + `@InjectRepository(Project)`) — never `manager.findOne('project', ...)` string lookup (throws 500).
- `npx shadcn@latest add <component>` can misresolve the `@` alias (check `info --json` `resolvedPaths`): after adding, verify placement with `git status`. Move stray `@/components/ui/*.tsx` into `src/components/ui/`, fix the generated `from "cn"` import to `@/lib/utils`, and remove the bogus `cn` npm dep it adds (`radix-ui` meta package already covers Select/Dialog primitives).
- Radix `SelectItem` values must be non-empty strings — never `value=""`. For a "No project" null-option use a sentinel (e.g. `NO_PROJECT_VALUE = 'none'`) mapped back to `null` in `onValueChange`.
- Radix `Select` in vitest/jsdom needs `Element.prototype.hasPointerCapture` / `setPointerCapture` / `releasePointerCapture` / `scrollIntoView` mocks in `src/test-setup.ts`; drive it with `user-event` click on the `combobox` trigger + `findByRole('option')`, not `fireEvent.change`. `<label htmlFor>` associates with `SelectTrigger`'s `<button>` (button is labelable), so `getByLabelText` / `getByRole('combobox', { name })` keep working after migrating from native `<select>`.
- Chained create-then-use mutations (e.g. create project → create board in `BoardFormModal`): cache the first result in a `useRef` keyed by input so a second-step failure + retry reuses it instead of duplicating/orphaning. Guard the submit handler with a synchronous `submittingRef` (React Query `isPending` flips only after re-render, so rapid double submits slip through). While any mutation is pending, block Dialog dismissal: guard `Dialog onOpenChange` in the modal shell (pending state lifted via `onPendingChange`) and `preventDefault()` `onInteractOutside`/`onEscapeKeyDown` on `DialogContent`.
- Assert navigation in modal tests by partially mocking `react-router-dom` (`vi.hoisted` for the `mockNavigate` + `importOriginal` spread keeps `MemoryRouter` real). Deterministic double-submit race test: two synchronous `fireEvent.click`s back-to-back (no `await` between) — the second handler must hit the ref guard before microtasks flush.
- Split modal close paths: user dismissal (`Dialog onOpenChange`, Cancel) goes through the pending guard; programmatic submit closes (success, pristine) use a raw `onClose` prop — the guard would see the just-set `submittingRef` and refuse to close. Cache chained-creation results in a `Map<key, id>` (not a single entry) so A→B→A rename sequences don't duplicate.

- T41 frontend migration: when a feature's hook (`use-cards`) newly depends on a helper exported from its API module (`sanitizeCardListParams`), the module-level `vi.mock('./<feature>.api')` in the hook tests must also mock that helper (or use `importOriginal`), otherwise Vite fails at runtime with "No ... export is defined on the mock".
- Frontend node_modules repair: if Vitest bus-errors after an interrupted copy/reinstall, remove `node_modules` (and stray `node_modules.bak`) entirely and `npm install` from a clean state; do not `git add -A` until verifying no ignored dependency trees are staged.
- Cutover: default backend target now lives in two places (`VITE_API_URL` in `compose.yaml`, Vite proxy fallback in `frontend/vite.config.ts`); keep both flipped together and retain the `VITE_API_URL=http://localhost:3000` override for one-line app rollback.

### Rust backend (`backend-rs`)

- The session store follows the POOL's database: `AppState::with_pool(settings, pool)` binds `MySqlStore` to `pool.connect_options().get_database()` (`.with_table_name("sessions")`). `#[sqlx::test]` builds its pool with `.database(&db_name)`, so the per-test DB name is already in the connect options — no `SELECT DATABASE()` round-trip and no hand-wired store in tests. Production boots through `AppState::from_settings`.
- `MySqlStore::with_schema_name` validates identifiers STRICTER than MySQL (rejects a leading digit, `-`, `.`, space). `Settings::validate` screens the DB name via `db::is_valid_db_name` so a bad `DB_NAME` is a clean `ConfigError` — never a boot panic when the store is constructed. `DATABASE_URL` and `DB_NAME` are two spellings of the same value; validate whichever is in play.
- `dotenvy::dotenv()` never OVERRIDES an existing env var — it only ADDS missing ones. So a developer's local `.env` silently re-supplies a key a test deliberately unset, and a "this must fail to boot" assertion passes for the wrong reason. `Settings::load_from_env()` (no dotenv) exists for exactly this; the config unit tests use it, boot uses `Settings::load()`.
- Rust test helpers: in-process suites own their plumbing in `backend-rs/tests/api/helpers.rs`; black-box (raw TCP) suites share `backend-rs/tests/api/wire.rs`. Import EXPLICITLY (`use super::helpers::{…}`) — a glob import lets a leftover local copy silently win, while an explicit import is a hard `E0255`. That compile error is the regression guard for "no per-suite helper copies".
- The two black-box suites and the in-process suites assert problem+json through ONE core (`helpers::assert_problem_fields`); the transport-specific `assert_problem` functions are thin adapters. Keep new problem assertions in the core.
- `backend-rs/tests/api.rs` needs an explicit `#[path = "api/<name>.rs"] mod <name>;` for every suite — a bare `mod health;` resolves to `tests/health.rs`, which does not exist.
- `cargo fmt --check | tail` MASKS the exit code (the pipeline's status is `tail`'s). Capture `$?` without a pipe (`> file; echo $?`).
- A test that binds a FIXED port (e.g. `TcpListener::bind(":3001")`) breaks under cargo's parallel test scheduler when two suites in the same binary pick the same port — and also breaks if a dev server already holds that port. Bind port **0** (OS-assigned ephemeral) and drive `listener.local_addr()`; the OS may hand out a different port than requested, so never re-derive the address from the requested port. `wire::bind_blackbox()` in `backend-rs/tests/api/wire.rs` does this for both black-box suites; `E2E_BASE_URL` pins a fixed port when an operator needs one.
- A normative ownership header that the CODE VIOLATES is worse than no header — a future contributor trusts it and copies the pattern. The E0255 guard only fires for names a suite actually IMPORTS, so a suite can shadow a shared name forever just by not importing it (`auth_register.rs` aliased `json_of as parse_json` to dodge the collision; `throttle.rs` imported nothing from `super::helpers` at all and kept its own `send` + a duplicate `PROBLEMS` const). Audit the forbidden set against the actual `fn` definitions, not against the import statements — and when a suite legitimately needs its own helper, it must take a name OUTSIDE the forbidden set AND be listed in the header's "Sanctioned exceptions".
- When auditing a forbidden set, grep for the DEFINITIONS, not the imports — and make sure the pattern covers EVERY name in the set. A first audit of the session-store change missed `health.rs`/`errors.rs` shadowing `test_settings`/`test_app` and three suites duplicating the `PROBLEM_BASE` literal, purely because the grep pattern had omitted those names. Grep the whole forbidden set, then re-grep for the constant's VALUE (`https://kanpad.dev/problems`) to catch inline duplicates of a "constant" that were never imported.
