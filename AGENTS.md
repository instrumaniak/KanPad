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
