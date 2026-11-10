# Frontend conventions

Source of truth for frontend conventions. Referenced from root `AGENTS.md`.

## API layer
### Rules

- API response types (`ApiResponse<T>`, `ListResponse<T>`, `ApiError`) must be defined in `@/lib/api`, not duplicated across feature API files.
- Use `apiFetch<T>()` helper instead of raw `fetch()` + `handleResponse()` in API files.
- Don't copy-paste `FETCH_OPTIONS` or `handleResponse` into each API file.


## React Query
### Rules

- Use targeted cache invalidation (`['columns', boardId]`) not global (`['columns']`) unless you need to invalidate all instances.
- When multiple handlers read from the same cache, capture original state at interaction start in React state. Don't assume cache is unchanged between handlers.
- In drag-drop code, `handleDragEnd` must use `dragSource` state from `handleDragStart`, not `getColumns()` which may be stale.


## E2E test patterns
### Rules

- Test setup (creating boards, columns, cards) must use API calls, not UI interactions. UI is only for the behavior under test.
- Never use `waitForTimeout()` in E2E tests. Use `waitForLoadState('networkidle')`, element assertions, or `page.waitForSelector()` instead.


## Code quality
### Rules

- Use Radix UI components (`DropdownMenu`, `Dialog`) for menus and modals instead of manual implementations with `useRef` and absolute positioning.
- Don't use `document.querySelector` for focus management - use React refs or callback props instead.


## Lessons learned
- Frontend board create/edit shares one `BoardFormModal(mode)` + top-level `BoardFormFields` in `features/boards/board-form-modal.tsx`. Don't reintroduce `InlineEditForm` in `board-card.tsx` or per-mode form copies. Reset edit state via `key={edit-${id}}` remount, not `useEffect` prop-to-state sync. `BoardCard.onEdit` passes full `Board` (incl. `project_id`), modal self-fetches projects via `useProjects`.
- `npx shadcn@latest add <component>` can misresolve the `@` alias (check `info --json` `resolvedPaths`): after adding, verify placement with `git status`. Move stray `@/components/ui/*.tsx` into `src/components/ui/`, fix the generated `from "cn"` import to `@/lib/utils`, and remove the bogus `cn` npm dep it adds (`radix-ui` meta package already covers Select/Dialog primitives).
- Radix `SelectItem` values must be non-empty strings — never `value=""`. For a "No project" null-option use a sentinel (e.g. `NO_PROJECT_VALUE = 'none'`) mapped back to `null` in `onValueChange`.
- Radix `Select` in vitest/jsdom needs `Element.prototype.hasPointerCapture` / `setPointerCapture` / `releasePointerCapture` / `scrollIntoView` mocks in `src/test-setup.ts`; drive it with `user-event` click on the `combobox` trigger + `findByRole('option')`, not `fireEvent.change`. `<label htmlFor>` associates with `SelectTrigger`'s `<button>` (button is labelable), so `getByLabelText` / `getByRole('combobox', { name })` keep working after migrating from native `<select>`.
- Chained create-then-use mutations (e.g. create project → create board in `BoardFormModal`): cache the first result in a `useRef` keyed by input so a second-step failure + retry reuses it instead of duplicating/orphaning. Guard the submit handler with a synchronous `submittingRef` (React Query `isPending` flips only after re-render, so rapid double submits slip through). While any mutation is pending, block Dialog dismissal: guard `Dialog onOpenChange` in the modal shell (pending state lifted via `onPendingChange`) and `preventDefault()` `onInteractOutside`/`onEscapeKeyDown` on `DialogContent`.
- Assert navigation in modal tests by partially mocking `react-router-dom` (`vi.hoisted` for the `mockNavigate` + `importOriginal` spread keeps `MemoryRouter` real). Deterministic double-submit race test: two synchronous `fireEvent.click`s back-to-back (no `await` between) — the second handler must hit the ref guard before microtasks flush.
- Split modal close paths: user dismissal (`Dialog onOpenChange`, Cancel) goes through the pending guard; programmatic submit closes (success, pristine) use a raw `onClose` prop — the guard would see the just-set `submittingRef` and refuse to close. Cache chained-creation results in a `Map<key, id>` (not a single entry) so A→B→A rename sequences don't duplicate.

- T41 frontend migration: when a feature's hook (`use-cards`) newly depends on a helper exported from its API module (`sanitizeCardListParams`), the module-level `vi.mock('./<feature>.api')` in the hook tests must also mock that helper (or use `importOriginal`), otherwise Vite fails at runtime with "No ... export is defined on the mock".
- Frontend node_modules repair: if Vitest bus-errors after an interrupted copy/reinstall, remove `node_modules` (and stray `node_modules.bak`) entirely and `npm install` from a clean state; do not `git add -A` until verifying no ignored dependency trees are staged.
- Cutover: default backend target now lives in two places (`VITE_API_URL` in `compose.yaml`, Vite proxy fallback in `frontend/vite.config.ts`); keep both flipped together and retain the `VITE_API_URL=http://localhost:3000` override for one-line app rollback.
