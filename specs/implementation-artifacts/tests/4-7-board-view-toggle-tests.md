# Test Automation Summary - Story 4.7 Board View Toggle

**Generated:** 2026-09-10
**Feature:** Board View Toggle Kanban <-> List (Story 4.7, status: review)
**Status:** Complete - all green including live E2E

---

## Generated Tests

### API Tests
- [x] `backend/src/boards/boards.service.spec.ts` - update view_mode board->list persists + follow-up findOne persistence check
- [x] `backend/src/boards/boards.service.spec.ts` - default view_mode board on create
- [x] `backend/src/boards/boards.service.spec.ts` - null/invalid view_mode ignored (allowlist guard)
- [x] E2E API verify: `GET /api/boards/:id` asserts `view_mode === 'list'` after UI toggle (DB persistence, not just UI)

### E2E Tests
- [x] `frontend/e2e/board-view-toggle.spec.ts` - login -> open board -> assert kanban -> toggle List -> assert rows + sort Created/Updated/Due/Title -> reload -> assert List persisted + aria-pressed -> API verify -> toggle Board -> assert kanban -> Add Card with column picker -> verify appears -> delete -> verify removed
- Fixed in this QA run: `describe.configure({timeout:90000})`, replaced `waitForTimeout` with `expect.polling`, fixed row locator `getByRole('row')` -> `getByTestId('board-list-row')` (header row collision)

### Frontend Unit Tests
- [x] `board-view-toggle.test.tsx` - renders Board/List, active indication, onChange payload, aria-pressed
- [x] `board-list-view.test.tsx` (15 tests) - flat rows, column badge, labels +N, due nulls-last asc/desc, created sort, panel open, inline edit Enter/blur commit-once + Escape cancel, delete, empty state, disabled-on-empty, create-with-column, no-columns hint
- [x] `board-view.test.tsx` - init from `board.view_mode`, fallback board, toggle PATCH payload, no-PATCH-on-active, rollback+toast, kanban preserved

## Coverage
- API endpoints: 1/1 covered (`PATCH /api/boards/:id` view_mode + all 7 mappers include view_mode)
- UI features: toggle + list + sort (4 keys) + panel + inline edit + add/delete covered
- Backend: 26 suites / 300 passed
- Frontend: 63 files / 538 passed
- E2E: 1/1 passed (13.4s, chromium)

## Verification
```
Backend tsc --noEmit: clean
Frontend tsc --noEmit: clean
Backend jest src/boards: 2 suites / 34 passed
Backend full: 26 / 300 passed
Frontend board-view: 3 files / 33 passed
Frontend full: 63 / 538 passed
E2E board-view-toggle: 1 passed
ESLint e2e spec: clean
Migrations: AddViewModeToBoards1779600000000 applied [X]
```

DB persistence rule (AGENTS.md): every PATCH/POST/DELETE asserts via follow-up `findOne` / `GET` - service spec lines 194-197 + E2E lines 91-95.

## Next Steps
- Run tests in CI (backend `npm test`, frontend `npm run test`, e2e `playwright test`)
- Consider replacing remaining `waitForTimeout` in other e2e specs with expect polling
- Large boards 100+ cards: monitor list render, add virtualization follow-up if slow

**Done!** Tests generated and verified. Checklist: happy path + critical errors, semantic locators, no hardcoded waits (fixed), independent tests.
