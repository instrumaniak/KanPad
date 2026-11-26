# T053: Anchor-based card DnD under active board search/filters

## Goal

Allow safe card drag-and-drop in board view while server-side search/filters are active. T050/T051 currently disable all board DnD under filters to avoid ambiguous position writes against hidden cards. This task replaces that blanket guard with **anchor-based move semantics** so cross-column moves and same-column reorders remain unambiguous when some cards are hidden by filter or not yet loaded.

This is a post-T051 amendment/follow-up: it intentionally revises locked T050/T051 DnD-guard behavior and adds an additive backend move-anchor contract.

## Depends On

- T048 (board cards embed/list pagination contract)
- T049 (server-side search/filter contract; filtered totals semantics)
- T050 (frontend infinite scroll + server-driven filters)
- T051 (e2e coverage baseline and current filtered-DnD guard assertions)

## Phase

1

## Critical

No

## Spec References

- `specs/tasks/T050-frontend-board-infinite-scroll-and-server-driven-filters.md` (amend DnD-under-filter AC/notes)
- `specs/tasks/T051-e2e-board-pagination-and-server-side-filter-coverage.md` (amend filtered DnD guard coverage)
- `specs/tasks/T049-backend-server-side-board-card-search-and-filters.md` (filter semantics, no-oracle validation patterns)
- `specs/tasks/T048-backend-parameterized-embed-limit-board-cards-list-endpoint.md` (embed/list totals semantics)
- `specs/tasks/T022-pagination-u-x-page-ceilings.md` (pagination/filter amendment context)
- `backend-rs/src/features/cards/{routes.rs,store.rs,types.rs}` (card move/reorder contract)
- `backend-rs/src/common/order_key.rs` (`key_for_index`, `clamp_position`)
- `backend-rs/src/common/card_summary.rs`, `backend-rs/src/features/cards/store.rs` (filtered-page-relative position semantics)
- `frontend/src/features/cards/drag-drop-context.tsx`, `frontend/src/features/cards/drag-over-cache.ts`
- `frontend/src/features/cards/use-cards.ts`, `frontend/src/features/cards/cards.api.ts`
- `frontend/src/features/columns/use-columns.ts`, `frontend/src/features/columns/column.tsx`, `frontend/src/features/columns/column-card-list.tsx`, `frontend/src/features/columns/column-header.tsx`
- `frontend/src/features/boards/board-view/board-view.tsx`, `frontend/src/features/boards/board-view/board-list-view.tsx`
- `frontend/e2e/board-search-filter.spec.ts`, `frontend/e2e/board-pagination.spec.ts`, `frontend/e2e/test-utils.ts`

## Problem / Investigation Summary

- Current board view disables DnD when `filterActive`: `board-view.tsx` passes `disabled={filterActive}` to `DragDropContext`; sort/move-all disabled in `column-header.tsx`; banners explain guard. T051 e2e asserts guarded affordances and no mutation requests.
- `DragDropContext` currently mutates canonical unfiltered cache key `['columns', boardId]` and computes `position` from visible array indices. When filtered, board renders filtered query key `['columns', boardId, normalizedFilters]` from `useColumns`.
- Filtered `CardSummary.position` / board list item position is **returned-set relative** (`offset + index` over returned filtered page in `fetch_card_page`; 0..n in embed `assemble_summaries`), while backend move API interprets `position` as full-column 0-based index over all cards in target column (`clamp_position(requested, others_len)`, `key_for_index`). Sending filtered visual index is ambiguous/wrong when hidden cards exist.
- Client-side filtering previously worked because visual index equaled full-column index.
- `useLoadMoreColumnCards` is already filter-aware and uses `matching_cards` when filtered. End-drop guard already blocks drops when unloaded tail exists.
- React 19 + dnd-kit 6.3.1 emits `useEffect` dep-array-size warning when sensors array toggles `[] <-> [3 sensors]`; T053 should remove that toggle by keeping sensors stable.

## Product Decisions (web-research validated)

| Decision | Recommendation | Rationale / source notes | T053 implication |
|---|---|---|---|
| Hidden-gap end anchor | **Accept/document** `after last visible loaded anchor` even when hidden cards exist between anchor and real end | Matches "one global order, filters are views; move relative to visible items; hidden items keep stable rank" (Jira/LexoRank-style filtered reorder). Deterministic and preserves drop location; appending to real end would jump card away. | Allow safe end/container drops only when resolved to visible loaded anchor; document that clearing filters may reveal hidden cards around moved card. |
| Over-card targeting | **v1 deterministic `before_card_id = overCard.id`** for over-card targets; container/end => `after_card_id = lastVisibleLoadedCard.id` when safe | Deterministic, matches current simple behavior, easy to test; backend supports both anchors so pointer-midpoint before/after can be added later. Atlassian DnD guidelines emphasize clear drop result/indicator; midpoint is common refinement. | Implement deterministic resolver now; note follow-up for pointer-midpoint before/after + drop indicator if UX testing needs it. |
| Blocked feedback channel | **Visual toast + polite `aria-live`/status announcement; no focus steal; no assertive** | Blocked drop is non-urgent status feedback. MDN/Chrome a11y guidance favor polite live regions for non-urgent updates; toasts alone can be missed by SR users (WP a11y). | Add AC: blocked drop gives visible feedback and polite SR announcement, no focus steal, no mutation. |

## Spec Amendments Required (same PR as implementation)

T053 intentionally relaxes locked T050/T051 blanket guard. Update specs in the same PR before/with implementation:

- `specs/tasks/T050-frontend-board-infinite-scroll-and-server-driven-filters.md`
  - Replace blanket "DnD disabled/guarded while filtered..." with anchor-safe filtered DnD rules; keep sort/move-all disabled.
  - Update AC/notes: allow visible-anchor card drag/reorder/cross-column move under filter; block ambiguous drops with explicit feedback; no ambiguous position writes.
  - Add UX copy decision: sort/move-all disabled while filter active; card DnD allowed for visible cards; hidden-gap behavior documented.
- `specs/tasks/T051-e2e-board-pagination-and-server-side-filter-coverage.md`
  - Update pre-dev contract checklist and AC89/filtered-guard language to reflect T053 behavior.
  - Replace "filtered DnD guard disables/blocks guarded affordances..." with: sort/move-all still disabled/no mutation; allowed visible-anchor filtered DnD persists after reload; ambiguous blocked drop gives feedback and no mutation.
- `frontend/e2e/board-search-filter.spec.ts`
  - Update header pre-dev contract checklist comment and rewrite filtered DnD guard test(s) accordingly.

## Anchor Contract (additive backend API)

### Request fields (`UpdateCardInput`)

| Field | Type | Meaning |
|---|---|---|
| `column_id` | `i32 >= 1`, optional | Target column. Existing behavior. |
| `position` | `i32`, optional | Existing full-column 0-based index (clamped). |
| `before_card_id` | `i32 >= 1`, optional | Place moving card immediately before anchor card in resolved target column stable order. |
| `after_card_id` | `i32 >= 1`, optional | Place moving card immediately after anchor card in resolved target column stable order. |

### Validation / semantics

- **Move-path trigger**: enter move/reorder path when any of `column_id`, `position`, `before_card_id`, `after_card_id` is present.
- **Mutual exclusivity**: at most one of `position`, `before_card_id`, `after_card_id` may be present. Both anchors present, or anchor+position, => 400 `validation-error` with `errors[]`.
- **Anchor scope (no-oracle)**: resolve anchor in resolved target column (source column if `column_id` absent; validated owned target if present). Anchor must be an owned card in that target column and not the moving card. Missing/foreign/other-board/wrong-column/deleted/self/not-in-target => **same generic 400 `validation-error`** on the anchor field (`before_card_id` or `after_card_id`), no existence oracle. Do not return 404 for anchor lookup, do not emit distinguishable messages.
- **Resolution rule**: compute anchor index from `others = fetch_card_keys_in_column(target, Some(moving_card_id))` in the same tx snapshot. `before_card_id=X` => `target_index = idx(X)`; `after_card_id=X` => `target_index = idx(X)+1`. Reuse `key_for_index`/rebalance/retry; keep D3 semantics (moving row locked, neighbors unlocked, unique-index conflict retried). Anchor scope is validated against the `others` snapshot before resolution; missing anchor (concurrent change) => same generic 400. Self-anchor excluded from `others` by construction but still explicitly checked. `after_card_id` at tail resolves to `others.len()` (allowed). Empty target with an anchor => 400. Rebalance scope remains target column only.
- **Scalars + move**: scalar patch fields remain allowed with anchor moves, same as existing move path.
- **Append behavior**: `column_id` with no position/anchor for cross-column keeps existing append-to-real-end behavior. Frontend should use it only for truly empty target columns under filter; otherwise use anchors.
- **No filter awareness in backend**: anchor validity is target-column membership/ownership, not filter visibility.

## Proposed Behavior

### Behavior matrix (filter/search active)

| Action | Behavior | Rationale |
|---|---|---|
| Cross-column drop onto a visible card | **Allow** | `before_card_id = overCard.id` |
| Same-column reorder onto a visible card | **Allow** | Same anchor semantics |
| Drop at column bottom/end/container area with visible loaded cards and no unloaded matching tail | **Allow** | `after_card_id = lastVisibleLoadedCard.id` |
| Drop at column top/start area with visible loaded cards | **Allow** | `before_card_id = firstVisibleLoadedCard.id` |
| Drop into truly empty column (`total_cards == 0`) | **Allow** | Append is unambiguous |
| Drop into column with no visible loaded anchor cards while hidden/unloaded matching cards exist | **Block** with toast + polite status, no mutation | No safe visual anchor |
| Drop at visual end/container when matching tail not fully loaded (`visibleCount < effectiveTotal`) | **Block** with toast + polite status, no mutation | Ambiguous unloaded matching tail |
| Move All Cards | **Keep disabled** | Operates on hidden cards; surprising/destructive |
| Sort by Date | **Keep disabled** | Global reorder of all cards in column |
| Keyboard DnD | **Allow** with same anchor/ambiguity rules | Same semantics as pointer |
| Add/edit/open/delete card | **Allow** | Not position-ambiguous |

`effectiveTotal = filterActive ? (matching_cards ?? visibleCount) : (total_cards ?? visibleCount)`.

## Frontend Design

### Locked implementation decisions (amendment 2026-11-26)

- **Mutation strategy:** DnD uses `useUpdateCard` (no `['cards']` optimistic writes; invalidates `['columns']` prefix + `['board-cards', boardId]` on success) for all anchor moves. Retire `useMoveCard`/`useReorderCard` from the DnD path to avoid conflicting optimism. Blocked drops issue no mutation.
- **Active cache key:** `DragDropContext` gains a `filters?: BoardColumnFilters` prop; derive `activeKey = columnQueryKey(boardId, filters)` and use it in all `getQueryData`/`setQueryData` reads/writes. `BoardView` passes `serverFilters`. Invalidations use prefix `['columns', boardId]` (covers filtered variants) plus `['board-cards', boardId]`.
- **Drop resolver module:** `frontend/src/features/cards/drop-resolver.ts` exporting a pure `resolveCardDrop(...)` returning `{ kind: 'anchor', payload } | { kind: 'append', payload } | { kind: 'blocked', reason }`. Inputs: active/over event, active columns, filterActive, counts (`matching_cards`/`total_cards`/visible), loaded counts. Container-over maps to "after last visible loaded anchor"; top/start only via over-first-card. v1 over-card rule: `before over card`. No pointer midpoint in v1.
- **Blocked feedback:** dedicated polite region `data-testid="dnd-blocked-status"` with `role="status"` and `aria-live="polite"` inside `DragDropContext`, plus a visual toast. Do not use `error` toast type (assertive). Locked toast title: "Can't drop here while filters are active." Blocked reasons: `no-visible-anchor` | `unloaded-tail` | `hidden-only-target`.
- **Count deltas (filtered):** cross-column moves adjust `matching_cards` +/-1 and `total_cards` +/-1; same-column reorders leave counts unchanged. Empty column detection under filter uses `total_cards === 0` (unfiltered empty).
- **Blocked drops:** no optimistic write, no mutation; invalidate prefix to roll back any live rearrangement.
- **Stable sensors:** always pass `sensors={sensors}` to `DndContext` (no conditional `[]`).
- **Mutual exclusivity validation order:** evaluate in `update_card` before any DB work; report all conflicting present fields in `errors[]`.
- **Anchor scope validation order:** after `fetch_card_keys_in_column(target, Some(moving_id))` snapshot, before `key_for_index`; missing anchor in snapshot (concurrent change) => same generic 400 on the anchor field. Self-anchor (`anchor == moving id`) => same generic 400.
- **No-oracle helper:** `fetch_card_column_owned(ex, user_id, card_id) -> Option<i32>` (JOIN cards→columns→boards, user-scoped, `fetch_optional`); never `fetch_card_owned` for anchors. Missing/foreign/other-board/wrong-column/deleted/self/not-in-target => identical 400 `validation-error` with a static message (e.g. "must be a card in the target column") on the anchor field; message must not echo the id.
- **Edge cases:** empty target with anchor => 400; `position: null` treated as absent (no mutual-exclusivity failure); cross-column anchor in source while `column_id` points elsewhere => wrong-column 400; `after_card_id` at tail => `idx+1 == others.len()` (allowed by `key_for_index`).
- **Error shape:** mutual-exclusivity and anchor-scope failures return `AppError::Validation` (400 `validation-error`) with `errors[]`; anchor-scope message byte-identical across cases (only field name differs).

### Cache ownership

- **Active cache only for optimistic UI**: when filter active, read/write the active filtered columns query key `columnQueryKey(boardId, normalizedFilters)`. When unfiltered, use `['columns', boardId]` as today.
- **No dual-cache writes**: never write filtered-only arrays into the canonical unfiltered key. Do not create a second DnD cache.
- **Invalidation**: on mutation settle/error/cancel, invalidate prefix `['columns', boardId]` (covers filtered variants) and `['board-cards', boardId]` as needed. Ensure no stale `['cards', columnId]` optimistic writes conflict; decide and document whether DnD uses `useMoveCard`/`useReorderCard` or a dedicated anchor-move mutation path.
- **DragDisabledContext**: remove blanket `disabled={filterActive}`. Keep `disabled` prop for true hard-disable cases only. Always pass stable `sensors={sensors}` to DndContext; guard in handlers for blocked ambiguous drops.

### Drop resolver (pure, testable)

Add a small resolver/helper that returns one of:

- `{ kind: 'anchor', payload: { before_card_id } | { after_card_id } | { column_id, before_card_id } ... }`
- `{ kind: 'append', payload: { column_id } }` (truly empty column only)
- `{ kind: 'blocked', reason: 'no-visible-anchor' | 'unloaded-tail' | 'hidden-only-target' }`

Inputs: `active/over` DnD event, active columns, filterActive, `matching_cards`/`total_cards`/visible counts, loaded counts. Handle: over card, over column container, same-column, cross-column, keyboard, end/container, empty column. Deterministic v1 over-card rule: before over card.

### Filtered optimistic rules

- Do **not** fabricate full-column positions by reindexing filtered array indices. Visual order in active cache is sufficient for UI; server is authoritative.
- Cross-column move in filtered active cache: remove from source, insert at visual target, adjust `matching_cards` +/-1 where present, adjust `total_cards` +/-1 where present (real unfiltered totals change). Card remains matching because filter fields unchanged.
- Same-column reorder in filtered active cache: reorder visual array only; counts unchanged. Invalidate prefix on settle to converge hidden-gap/server-normalized order.
- Blocked drops: no optimistic write, no mutation request; emit toast + polite status.

### UX / a11y

- Board banner when filter active: sort/move-all disabled; card drag/drop allowed for visible cards. Document hidden-gap consequence in task/spec notes (user-facing copy can remain concise).
- Column header: keep sort/move-all disabled with explicit affordance/copy.
- List view: remains no-DnD; keep copy accurate.
- Blocked drop feedback: visual toast + `aria-live="polite"` status (or equivalent announced status region), no focus steal, clear reason/action. No mutation.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/features/cards/types.rs` | Add additive `before_card_id`, `after_card_id`; validation/mutual exclusivity; ToSchema docs. |
| `backend-rs/src/features/cards/routes.rs` | Trigger move path for anchor fields; pass anchor mode; update docs/errors. |
| `backend-rs/src/features/cards/store.rs` (or helper) | No-oracle anchor resolution in tx from target-column `others`; self/wrong-scope generic 400. |
| `backend-rs/src/docs.rs`, `backend-rs/openapi.json`, `frontend/src/lib/api/schema.d.ts` | Regenerate additive contract; `gen:api -- --check`. |
| `frontend/src/features/cards/drop-resolver.ts` (new) | Pure `resolveCardDrop(...)` returning anchor/append/blocked; container-over→after-last-visible; top/start via over-first-card; v1 over-card→before. |
| `frontend/src/features/cards/drag-drop-context.tsx` | Query-key/filter-aware DnD (`filters` prop → active key); anchor resolver; blocked ambiguous drop feedback (polite region + toast); stable sensors; no dual-cache writes; use `useUpdateCard`. |
| `frontend/src/features/cards/drag-over-cache.ts` | Filter-safe optimistic move: no fake positions, `matching_cards`/`total_cards` deltas. |
| `frontend/src/features/cards/cards.api.ts`, `frontend/src/features/cards/use-cards.ts` | Anchor payload types; clarify/bypass conflicting optimistic paths. |
| `frontend/src/features/columns/use-columns.ts`, `frontend/src/features/columns/column.tsx`, `frontend/src/features/columns/column-card-list.tsx`, `frontend/src/features/columns/column-header.tsx` | Filter-aware guards/counts; keep sort/move-all disabled. |
| `frontend/src/features/boards/board-view/board-view.tsx` | Remove blanket filter DnD disable; pass active query key/filters; update banner copy/status. |
| `frontend/src/features/boards/board-view/board-list-view.tsx` | Keep no-DnD copy accurate. |
| `frontend/e2e/board-search-filter.spec.ts` | Rewrite filtered-DnD guard tests for T053 behavior. |
| `frontend/e2e/board-pagination.spec.ts` or focused e2e | Add filtered DnD persistence/anchor coverage if cleaner. |
| `frontend/src/features/cards/drag-drop-context.test.tsx` and related unit tests | Anchor resolution, blocked cases, cache ownership, counts. |
| `specs/tasks/T050-*.md`, `specs/tasks/T051-*.md` | Same-PR spec amendments. |

## Implementation Steps

1. **Spec/contract preflight**: amend T050/T051 sections/ACs listed above; lock anchor contract table, drop resolver rules, UX copy, and blocked feedback channel. Record decisions in PR/task notes.
2. **Backend TDD (red)**: anchor same/cross-column before/after; cross-column+column_id; scalar+anchor; append unchanged; mutual exclusivity; `position+anchor` rejected; invalid/self/wrong-column/foreign/deleted/not-in-target => same generic 400 no-oracle; anchors trigger move path; MySQL 8.4 + MariaDB 11.4.
3. **Backend implementation**: extend DTO/route move-path trigger; resolve anchor from target-column `others` snapshot in tx; `key_for_index`/rebalance/retry unchanged. No anchor-row FOR UPDATE unless justified.
4. **Contract regen**: docs/OpenAPI/schema/typegen; `gen:api -- --check`.
5. **Frontend TDD (red)**: drop resolver unit tests (over card, container/end, empty, hidden-only, unloaded tail, keyboard/container); filtered cache key writes; no fake position reindex; `matching_cards`/`total_cards` deltas; blocked feedback no mutation.
6. **Frontend DnD refactor**: query-key/filter-aware context, stable sensors, handler-level guards, remove blanket filter disable. Decide `useMoveCard`/`useReorderCard` strategy.
7. **Optimistic/cache coherence**: active filtered cache only; prefix invalidation on settle/error/cancel; converge hidden-gap/server order.
8. **UX/a11y**: banner/copy update, polite aria-live blocked feedback, no focus steal, sort/move-all still disabled.
9. **E2E amendments**: update `board-search-filter.spec.ts` pre-dev checklist and tests; add allowed filtered cross-column/same-column DnD persistence; blocked ambiguous case asserts visible+polite announced feedback and zero guarded mutations. Follow API-seeded, no-waitForTimeout, scoped, virtualization-safe, bounded-scroll rules.
10. **Verification gates**: backend fmt/clippy/tests on MySQL 8.4 + MariaDB 11.4 tmpfs; frontend typecheck/lint/unit; targeted Playwright; full e2e confidence run as needed.

## Constraints

- **No ambiguous position writes against hidden cards.** Primary invariant.
- Additive backend contract only; same-PR OpenAPI/schema/typegen update.
- No existence oracle for anchor ids; same generic 400 validation shape for invalid anchor scope cases.
- Keep `total_cards` unfiltered authoritative and `matching_cards` filtered authoritative (T049).
- `move-all` and `sort` remain disabled under active filters/search.
- React Query: no dual DnD cache; prefix invalidation `['columns', boardId]` covers filtered keys.
- No fake full-column positions in filtered optimistic cache.
- No `waitForTimeout` in e2e; API-seeded setup; virtualization-safe assertions; scoped assertions in shared `kanpad_e2e`.
- MySQL 8.4 and MariaDB 11.4 compatibility.

## Acceptance Criteria

- [ ] T050/T051 (and affected e2e pre-dev checklist comments) are amended in same PR to reflect T053 filtered-DnD rules.
- [ ] Backend `PATCH /api/v1/cards/{id}` supports additive `before_card_id`/`after_card_id`, with move-path trigger, mutual exclusivity, and same generic no-oracle 400 `validation-error` for invalid anchor scope/self cases.
- [ ] Anchor-based moves are correct on MySQL 8.4 and MariaDB 11.4 for same/cross-column moves, existing point-move/rebalance/retry semantics preserved.
- [ ] When search/filter is active, board-view users can drag/drop cards across columns and reorder within a column when drop resolves to a safe visible anchor (over-card, container/end, top/start, empty column); move persists after reload under the active filter and global order is correct after clearing filters.
- [ ] Ambiguous drops (no visible anchor, unloaded matching tail, hidden-only target) are blocked with visible toast (title "Can't drop here while filters are active.") + polite aria-live region (`data-testid="dnd-blocked-status"`, `role="status"`, `aria-live="polite"`), no focus steal, and no mutation request.
- [ ] Sort-by-date and Move All Cards remain disabled under active search/filter with explicit affordance/copy.
- [ ] Filtered DnD uses active filtered query cache for optimistic UI; no dual-cache writes; no fake filtered-position fabrication; cross-column moves adjust `matching_cards` and `total_cards` (+/-1), same-column reorders leave counts unchanged; prefix invalidation on settle/error/cancel.
- [ ] E2E covers allowed filtered cross-column/same-column DnD (including empty-column and top/start anchor where feasible), blocked ambiguous case (no-visible-anchor, unloaded-tail, hidden-only-target) with visible+polite feedback, and still-disabled sort/move-all, using API-seeded/no-waitForTimeout/scoped/virtualization-safe/bounded-scroll patterns. Keyboard DnD covered under same rules or explicitly deferred with justification.
- [ ] Frontend/backend tests and typegen checks pass; no regression in unfiltered DnD behavior.

## Risks / Open Questions

- **Hidden gap UX**: accepted by product decision; moved card may appear before hidden cards after filter clear. Keep copy/docs clear.
- **Pointer-midpoint before/after**: deferred for v1; deterministic before-over-card rule chosen. Add follow-up if UX testing shows ambiguity.
- **Keyboard/container-over mapping**: must be deterministic and covered by tests; container-over should resolve through same resolver.
- **Mutation hook overlap**: `useMoveCard`/`useReorderCard` `['cards']` optimism may conflict; decide bypass/refactor before implementation.
- **Same-column settle convergence**: invalidate columns prefix on settle to avoid hidden-gap drift.
- **Concurrent anchor changes**: anchor deleted/moved out concurrently should yield same generic 400; test where practical.

## Notes

- Investigation basis (2026-11-26): `drag-drop-context.tsx` hardcoded `['columns', boardId]` and filtered-index positions; `useColumns`/`columnQueryKey` create filtered keys; backend move clamps integer position to full-column `others_len`; CardSummary positions returned-set relative in `card_summary.rs` and `cards/store.rs`.
- Review basis (2026-11-26): 4 read-only subagent reviews (backend, frontend, spec/e2e, security/edge) + web research for product decisions.
- Web research references: LexoRank filtered reorder stability (appcraft.nl), Atlassian Pragmatic DnD design guidelines, MDN `aria-live`, Chrome drag-and-drop/toast accessibility guidance, Primer drag-and-drop a11y, WP Accessibility toasts.
- Small cleanup included: always pass stable `sensors={sensors}` to DndContext to avoid React 19/dnd-kit dep-array-size warning.
- This task is a behavior/spec amendment, not a silent relaxation of T050/T051 guardrails.