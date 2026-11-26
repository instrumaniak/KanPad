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

- `specs/tasks/T050-frontend-board-infinite-scroll-and-server-driven-filters.md` (amend/verify DnD-under-filter AC/notes)
- `specs/tasks/T051-e2e-board-pagination-and-server-side-filter-coverage.md` (amend/verify filtered DnD guard coverage)
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
- Filtered `CardSummary.position` in column card pages/embed is **returned-set relative** (`offset + index` over returned filtered page in `fetch_card_page`; 0..n in embed `assemble_summaries`), while backend move API interprets `position` as full-column 0-based index over all cards in target column (`clamp_position(requested, others_len)`, `key_for_index`). Sending filtered visual index is ambiguous/wrong when hidden cards exist.
- `BoardCardListItem.position` is different: it is computed per-column in stable sort-key order by the board-cards query (`COUNT(*) ... p2.sort_key < k.sort_key`), not returned-set relative. The DnD ambiguity still applies to board-view column/embed filtered visual indices.
- Client-side filtering previously worked because visual index equaled full-column index.
- `useLoadMoreColumnCards` is already filter-aware and uses `matching_cards` when filtered. End-drop guard already blocks drops when unloaded tail exists.
- React 19 + dnd-kit 6.3.1 emits `useEffect` dep-array-size warning when sensors array toggles `[] <-> [3 sensors]`; T053 should remove that toggle by keeping sensors stable.

## Product Decisions (web-research validated)

| Decision | Recommendation | Rationale / source notes | T053 implication |
|---|---|---|---|
| Hidden-gap end anchor | **Accept/document** `after last visible loaded anchor` even when hidden cards exist between anchor and real end | Matches "one global order, filters are views; move relative to visible items; hidden items keep stable rank" (Jira/LexoRank-style filtered reorder). Deterministic and preserves drop location; appending to real end would jump card away. | Allow safe end/container drops only when resolved to visible loaded anchor; document that clearing filters may reveal hidden cards around moved card. |
| Over-card targeting | **v1 deterministic `before_card_id = overCard.id`** for over-card targets; container/end => `after_card_id = lastVisibleLoadedCard.id` when safe; top/start only via over-first-card | Deterministic, matches current simple behavior, easy to test; backend supports both anchors so pointer-midpoint before/after can be added later. Atlassian DnD guidelines emphasize clear drop result/indicator; midpoint is common refinement. | Implement deterministic resolver now; container-over never maps to top/start. Note follow-up for pointer-midpoint before/after + drop indicator if UX testing needs it. |
| Blocked feedback channel | **Visual neutral toast + persistently mounted polite `aria-live`/status announcement; no focus steal; no assertive** | Blocked drop is non-urgent status feedback. MDN/Chrome a11y guidance favor polite live regions for non-urgent updates; toasts alone can be missed by SR users. `frontend/src/components/ui/toast-provider/index.tsx` maps `error`/`destructive` to `alert`/`assertive`. | Add AC: blocked drop gives neutral/default non-assertive toast and polite SR announcement, no focus steal, no mutation. Repeated identical blocked drops must be re-announced. |

## Repo Reality / Delta Checklist (2026-11-26)

T050/T051 docs and the `frontend/e2e/board-search-filter.spec.ts` header checklist were partially amended in doc commit `eafe24b7`, but runtime code/tests still implement the old blanket guard. T053 implementation must reconcile **both** doc drift and runtime drift.

| Area | Current state | Required T053 delta | Done when |
|---|---|---|---|
| `specs/tasks/T050-...md` | Already contains T053 anchor-safe DnD language in key AC/steps. | Verify wording matches final locked rules below (payload semantics, mutation/cache, blocked a11y, copy). Update only where stale. | No blanket "DnD disabled while filtered" remains except sort/move-all/list-view no-DnD. |
| `specs/tasks/T051-...md` | Already contains T053 pre-dev checklist/AC89 language. | Verify e2e delta is precise (test names/cases/selectors/copy/request predicates). Update only where stale. | T051 clearly requires rewriting old guard test and adding allowed/blocked cases. |
| `frontend/e2e/board-search-filter.spec.ts` header | Header checklist amended to T053. | Keep header aligned with actual rewritten tests. | Header and test body agree. |
| `frontend/e2e/board-search-filter.spec.ts` body | Still asserts blanket guard (`filtered DnD guard disables/blocks guarded affordances...`, banner says DnD disabled, drag/keyboard expect no mutation because disabled). | Rewrite to T053 behavior: sort/move-all still disabled/no mutation; allowed visible-anchor card DnD persists; ambiguous blocked drop gives toast + polite `dnd-blocked-status` and no mutation. | Old blanket assertions removed/replaced. |
| `frontend/src/features/boards/board-view/board-view.tsx` | Passes `disabled={filterActive}` to `DragDropContext`; banner says DnD disabled. | Remove blanket filter disable; pass filters/active key context as needed; update banner copy. | Cards draggable under filter; sort/move-all still disabled. |
| `frontend/src/features/cards/drag-drop-context.tsx` | Hardcoded `['columns', boardId]`, `useMoveCard/useReorderCard`, no anchor resolver/blocked status, sensor toggle. | Active-key/filter-aware anchor DnD, stable sensors, blocked feedback, board-scoped invalidation. | No hardcoded canonical key for filtered optimistic path. |
| `backend-rs/src/features/cards/{types,routes,store}` | No anchor fields/logic. | Add additive anchor contract and no-oracle validation. | OpenAPI/schema/typegen updated. |
| `specs/tasks/status.yaml` | `T053.depends: ~` | Align with task deps (T048-T051) or record reason if intentionally different. | Metadata consistent. |

## Anchor Contract (additive backend API)

### Request fields (`UpdateCardInput`)

| Field | Type | Meaning |
|---|---|---|
| `column_id` | `i32 >= 1`, optional | Target column. Existing behavior. Required for cross-column anchor/append moves. |
| `position` | `i32`, optional | Existing full-column 0-based index (clamped). Legacy/unfiltered-compatible path. |
| `before_card_id` | `i32 >= 1`, optional | Place moving card immediately before anchor card in resolved target column stable order. |
| `after_card_id` | `i32 >= 1`, optional | Place moving card immediately after anchor card in resolved target column stable order. |

### Payload rules (locked)

| Scenario | Required payload | Notes |
|---|---|---|
| Cross-column drop over visible card | `{ column_id: <target>, before_card_id: <overCardId> }` | `column_id` is required because backend target resolves as `target_column.unwrap_or(source)`. |
| Cross-column drop at safe end/container | `{ column_id: <target>, after_card_id: <lastVisibleLoadedCardId> }` | Use only when `visibleCount > 0` and no unloaded matching tail. |
| Cross-column drop into truly empty column | `{ column_id: <target> }` | Append is unambiguous only when `total_cards === 0`. |
| Same-column reorder over visible card | `{ before_card_id: <overCardId> }` | `column_id` may be omitted; if present it must resolve to same column. |
| Same-column safe top/start | `{ before_card_id: <firstVisibleLoadedCardId> }` | Top/start only by over-first-card; container-over does not map to top. |
| Same-column safe end/container | `{ after_card_id: <lastVisibleLoadedCardId> }` | Only when no unloaded matching tail. |

### Validation / semantics

- **Move-path trigger**: enter move/reorder path when any of `column_id`, `position`, `before_card_id`, `after_card_id` is present.
- **Mutual exclusivity**: at most one of `position`, `before_card_id`, `after_card_id` may be present. Both anchors present, or anchor+position, => 400 `validation-error` with `errors[]` naming every conflicting field present.
- **Null/missing semantics**: `position: null` is treated as absent (no mutual-exclusivity failure by itself). Anchor fields follow normal JSON optional-field semantics; `0`/negative fail id/range validation as 400.
- **Target resolution**: `target = column_id.unwrap_or(source_column)`. Cross-column anchor moves must send `column_id`; backend must not infer cross-column target from anchor id.
- **Anchor scope (no-oracle, snapshot-only)**: resolve anchor inside the already-owned resolved target column's `others = fetch_card_keys_in_column(target, Some(moving_card_id))` snapshot in the same tx. Do **not** add a pre-snapshot `fetch_card_owned`/`fetch_card_column_owned` lookup that can distinguish existence/ownership. Missing/foreign/other-board/wrong-column/deleted/self/not-in-target/missing-after-concurrent-move => **same generic 400 `validation-error`** on the anchor field (`before_card_id` or `after_card_id`), same static message (e.g. `must be a card in the target column`), no id echo, no 404/403 oracle.
- **Resolution rule**: `before_card_id=X` => `target_index = idx(X)`; `after_card_id=X` => `target_index = idx(X)+1`. Reuse `key_for_index`/rebalance/retry; keep D3 semantics (moving row locked, neighbors unlocked, unique-index conflict retried). `after_card_id` at tail resolves to `others.len()` (allowed). Empty target with an anchor => same generic 400 anchor-scope error. Rebalance scope remains target column only.
- **Scalars + move**: scalar patch fields remain allowed with anchor moves, same as existing move path.
- **Append behavior**: `column_id` with no position/anchor for cross-column keeps existing append-to-real-end behavior. Frontend uses it only for truly empty target columns (`total_cards == 0`); otherwise it uses anchors.
- **No filter awareness in backend**: anchor validity is target-column membership/ownership, not filter visibility.

### Error shape (locked)

Mutual exclusivity:

```json
{
  "type": "validation-error",
  "title": "Validation failed",
  "status": 400,
  "errors": [
    { "field": "before_card_id", "message": "..." },
    { "field": "position", "message": "..." }
  ]
}
```

Anchor scope (all no-oracle cases identical except field name):

```json
{
  "type": "validation-error",
  "title": "Validation failed",
  "status": 400,
  "errors": [
    { "field": "before_card_id", "message": "must be a card in the target column" }
  ]
}
```

## Proposed Behavior

### Behavior matrix (filter/search active)

| Action | Behavior | Rationale |
|---|---|---|
| Cross-column drop onto a visible card | **Allow** | Send `column_id` + `before_card_id = overCard.id`. |
| Same-column reorder onto a visible card | **Allow** | Send `before_card_id = overCard.id`. |
| Drop at column bottom/end/container area with visible loaded cards and no unloaded matching tail | **Allow** | Send `after_card_id = lastVisibleLoadedCard.id` (`column_id` for cross-column). Container-over never maps to top/start. |
| Drop at column top/start area with visible loaded cards | **Allow** | Send `before_card_id = firstVisibleLoadedCard.id` only when over first card/top-start target is explicitly resolved. |
| Drop into truly empty column (`total_cards == 0`) | **Allow** | Cross-column append `{ column_id }` is unambiguous. |
| Drop into column with no visible loaded anchor cards while hidden/unloaded matching cards exist | **Block** with neutral toast + polite status, no mutation | No safe visual anchor. |
| Drop at visual end/container when matching tail not fully loaded (`visibleCount < effectiveTotal`) | **Block** with neutral toast + polite status, no mutation | Ambiguous unloaded matching tail. |
| Move All Cards | **Keep disabled** | Operates on hidden cards; surprising/destructive. |
| Sort by Date | **Keep disabled** | Global reorder of all cards in column. |
| Keyboard DnD | **Allow** with same anchor/ambiguity rules | Same semantics as pointer; covered by tests or explicitly deferred with justification. |
| Add/edit/open/delete card | **Allow** | Not position-ambiguous. |

`effectiveTotal = filterActive ? (matching_cards ?? visibleCount) : (total_cards ?? visibleCount)`. If `filterActive` is true and `matching_cards` is undefined/unknown, treat end/container drop as **ambiguous/fail closed** unless the target column is truly empty (`total_cards === 0`). Resolver must use the same totals source as `ColumnCardList`/`useLoadMoreColumnCards`.

## Frontend Design

### Locked implementation decisions (amendment 2026-11-26, clarified 2026-11-27)

- **Mutation strategy (final):** DnD uses a **dedicated board-scoped anchor-move mutation** (new hook or a board-scoped wrapper around `updateCard`; do **not** reuse `useUpdateCard` as-is). The DnD mutation must: send anchor payloads, perform **no `['cards', columnId]` optimistic writes**, optimistically update only the active columns query key captured at drag start, and on settle/error/cancel invalidate `['columns', boardId]` prefix + `['board-cards', boardId]` only. It must not globally invalidate `['columns']`. Preserve session-boundary safety (currentSessionEpoch guard or equivalent) if the implementation moves away from `useMoveCard/useReorderCard`.
- **Legacy hooks:** `useMoveCard`/`useReorderCard` are **not used by DnD** after T053. Audit remaining callers; keep them only if still needed outside board DnD, with their scope documented. Remove or isolate them if DnD was their only real consumer. Do not leave DnD depending on their `['cards']` optimism.
- **Active cache key:** `DragDropContext` gains `filters?: BoardColumnFilters`. Derive `activeKey = columnQueryKey(boardId, filters)` at drag start and use that captured key for optimistic reads/writes during that drag lifecycle. `BoardView` passes `serverFilters`. Do not write canonical unfiltered `['columns', boardId]` from a filtered drag.
- **Filter change during in-flight DnD mutation:** capture `activeKey` at drag start. If filters change before settle, stale optimistic UI must not corrupt the new filter cache; rely on board-scoped prefix invalidation/refetch. Tests should cover drop-under-filter then clear/change filter before/after settle.
- **Drop resolver module:** `frontend/src/features/cards/drop-resolver.ts` exporting a pure `resolveCardDrop(...)` returning `{ kind: 'anchor', payload } | { kind: 'append', payload } | { kind: 'blocked', reason }`. Inputs: active/over event, active columns snapshot, filterActive, counts (`matching_cards`/`total_cards`/visible), loaded counts. Container-over maps to "after last visible loaded anchor"; top/start only via over-first-card/top-start target explicitly resolved. v1 over-card rule: `before over card`. No pointer midpoint in v1.
- **Cross-column payload rule:** any cross-column anchor or append move must include `column_id: <targetColumnId>`. Same-column anchor moves may omit `column_id`.
- **Blocked feedback (locked):** persistently mounted polite region inside `DragDropContext` with `data-testid="dnd-blocked-status"`, `role="status"`, `aria-live="polite"`; update its text on every blocked drop so repeated identical blocks are re-announced (nonce/clear-set pattern acceptable). Also show a **neutral/default non-assertive** visual toast with title `"Can't drop here while filters are active."` Do **not** use `error`/`destructive` toast types (toast provider maps them to assertive alert). No focus steal, no mutation.
- **Count deltas (filtered):** cross-column moves adjust `matching_cards` +/-1 and `total_cards` +/-1 in the active filtered cache; same-column reorders leave counts unchanged. Empty column detection under filter uses `total_cards === 0` (unfiltered empty).
- **Blocked drops:** no optimistic write, no mutation; invalidate board-scoped columns prefix to roll back any live rearrangement.
- **Stable sensors:** always pass `sensors={sensors}` to `DndContext` (no conditional `[]`). Enforce disabled/blocked behavior in handlers/resolver; keep `disabled` prop only for true hard-disable cases.
- **Mutual exclusivity validation order (backend):** evaluate in `update_card` before any DB work; report all conflicting present fields in `errors[]`.
- **Anchor scope validation order (backend):** after `fetch_card_keys_in_column(target, Some(moving_id))` snapshot in same tx, before `key_for_index`; missing anchor in snapshot (including concurrent move/delete) => same generic 400 on the anchor field. Self-anchor (`anchor == moving id`) => same generic 400. Do not use ownership-lookup helpers that can create oracles.
- **No-oracle rule (backend):** no existence oracle. Missing/foreign/other-board/wrong-column/deleted/self/not-in-target => identical 400 `validation-error` with static message `must be a card in the target column` on the anchor field; message must not echo id. Only field name differs (`before_card_id` or `after_card_id`).
- **Edge cases:** empty target with anchor => same generic 400 anchor-scope error; `position: null` treated as absent; cross-column anchor in source while `column_id` points elsewhere => wrong-column 400; `after_card_id` at tail => `idx+1 == others.len()` (allowed by `key_for_index`); same-column reorder with no position/anchor keeps existing behavior only where unambiguous.
- **Error shape:** mutual-exclusivity and anchor-scope failures return `AppError::Validation` (400 `validation-error`) with `errors[]`; anchor-scope message byte-identical across no-oracle cases.

### Cache ownership

- **Active cache only for optimistic UI**: when filter active, read/write the captured active filtered columns query key `columnQueryKey(boardId, normalizedFilters)`. When unfiltered, use `['columns', boardId]`.
- **No dual-cache writes**: never write filtered-only arrays into canonical unfiltered key. Never fabricate a second DnD cache.
- **Invalidation**: on mutation settle/error/cancel and on blocked drop rollback, invalidate prefix `['columns', boardId]` (covers filtered variants) and `['board-cards', boardId]`. Do not use global `['columns']` invalidation for board-scoped DnD moves.
- **Position fields during filtered optimism**: do not reindex/fabricate full-column positions from filtered visible indices. Preserve existing card positions except where unavoidable object movement is needed for visual order; server refetch is authoritative for positions after settle.
- **DragDisabledContext**: remove blanket `disabled={filterActive}`. Keep `disabled` prop for true hard-disable cases only. Always pass stable `sensors={sensors}` to DndContext.

### Drop resolver (pure, testable)

Return one of:

- `{ kind: 'anchor', payload: { before_card_id } | { after_card_id } | { column_id, before_card_id } | { column_id, after_card_id } }`
- `{ kind: 'append', payload: { column_id } }` (truly empty target column only)
- `{ kind: 'blocked', reason: 'no-visible-anchor' | 'unloaded-tail' | 'hidden-only-target' }`

Rules: over-card => `before_card_id = overCard.id`; container/end with safe visible anchor => `after_card_id = lastVisibleLoadedCard.id`; top/start only via over-first-card => `before_card_id = firstVisibleLoadedCard.id`; container-over never maps to top; empty target (`total_cards === 0`) => append with `column_id`; filter active and `matching_cards` unknown => fail closed for ambiguous end/container drops.

### Filtered optimistic rules

- No fake full-column position reindexing. Visual order in active cache is sufficient; server is authoritative.
- Cross-column move in filtered active cache: remove from source, insert at visual target, adjust `matching_cards` +/-1 and `total_cards` +/-1. Card remains matching because filter fields unchanged.
- Same-column reorder in filtered active cache: reorder visual array only; counts unchanged. Invalidate prefix on settle to converge hidden-gap/server-normalized order.
- Blocked drops: no optimistic write, no mutation request; emit neutral toast + polite status; invalidate prefix to roll back live rearrangement.

### UX / a11y

- Board banner when filter active: sort/move-all disabled; card drag/drop allowed for visible cards. Hidden-gap consequence documented (clearing filters may reveal hidden cards around moved card).
- Column header: keep sort/move-all disabled with explicit affordance/copy.
- List view: remains no-DnD; keep copy accurate.
- Blocked drop feedback: persistently mounted polite `aria-live="polite"` status region, neutral/default non-assertive toast, no focus steal, clear reason/action, no mutation. Repeated identical blocked drops are announced.

## Files to Create/Modify

| File | Change |
|---|---|
| `backend-rs/src/features/cards/types.rs` | Add additive `before_card_id`, `after_card_id`; validation/mutual exclusivity; `ToSchema` docs. |
| `backend-rs/src/features/cards/routes.rs` | Trigger move path for anchor fields; accept anchor mode; preserve tx/rebalance/retry semantics; no-oracle anchor validation. |
| `backend-rs/src/features/cards/store.rs` | Resolve anchor against target-column `others` snapshot in tx; no oracle-producing ownership lookup for anchors. |
| `backend-rs/src/docs.rs`, `backend-rs/openapi.json`, `frontend/src/lib/api/schema.d.ts`, `CONTRACT-CHANGES.md` (if repo records contract deltas) | Regenerate additive contract; `npm run gen:api -- --check` (or repo equivalent). |
| `frontend/src/features/cards/drop-resolver.ts` (new) | Pure `resolveCardDrop(...)` returning anchor/append/blocked; cross-column payload includes `column_id`; container-over→after-last-visible; top/start only via over-first-card; v1 over-card→before. |
| `frontend/src/features/cards/drag-drop-context.tsx` | Query-key/filter-aware DnD (`filters` prop, captured `activeKey`); anchor resolver; persistently mounted polite blocked-status region + neutral non-assertive toast; stable sensors; no dual-cache writes; board-scoped DnD mutation. |
| `frontend/src/features/cards/drag-over-cache.ts` | Filter-safe optimistic move: no fake full-column positions; `matching_cards`/`total_cards` deltas for cross-column filtered moves. |
| `frontend/src/features/cards/cards.api.ts`, `frontend/src/features/cards/use-cards.ts` | Anchor payload types; add dedicated board-scoped anchor-move mutation/wrapper; do not use global `['columns']` invalidation or `['cards']` optimism for board DnD. |
| `frontend/src/features/columns/use-columns.ts`, `frontend/src/features/columns/column.tsx`, `frontend/src/features/columns/column-card-list.tsx`, `frontend/src/features/columns/column-header.tsx` | Filter-aware guards/counts; keep sort/move-all disabled; preserve total-source consistency. |
| `frontend/src/features/boards/board-view/board-view.tsx` | Remove blanket filter DnD disable; pass `serverFilters`; update banner copy/status. |
| `frontend/src/features/boards/board-view/board-list-view.tsx` | Keep no-DnD copy accurate. |
| `frontend/e2e/board-search-filter.spec.ts` | Rewrite filtered-DnD guard tests for T053 behavior; keep header checklist aligned with body. |
| `frontend/e2e/board-pagination.spec.ts` or focused e2e | Add filtered DnD persistence/anchor coverage if cleaner. |
| `frontend/src/features/cards/drag-drop-context.test.tsx`, `drag-over-cache.test.ts`, `use-cards.test.tsx`, `drop-resolver.test.ts` (new) | Resolver/mutation/cache/a11y/blocked behavior coverage; update old hook/position assumptions. |
| `specs/tasks/T050-*.md`, `specs/tasks/T051-*.md`, `specs/tasks/status.yaml` | Verify/amend spec drift and metadata consistency. |

## Implementation Steps

1. **Spec/contract preflight**: reconcile repo-reality delta checklist above; verify T050/T051 docs/comments; lock payload rules, anchor no-oracle contract, mutation/cache strategy, blocked a11y, and e2e case names.
2. **Backend TDD (red)**: anchor same/cross-column before/after; cross-column requires/uses `column_id`; scalar+anchor; append unchanged; mutual exclusivity; `position: null` absent; `position+anchor` rejected; invalid/self/wrong-column/foreign/other-board/deleted/not-in-target/concurrent-missing => same generic 400 no-oracle (byte-identical message, field differs); anchors trigger move path; MySQL 8.4 + MariaDB 11.4.
3. **Backend implementation**: extend DTO/route move-path trigger; resolve anchor from target-column `others` snapshot in tx; keep `key_for_index`/rebalance/retry/D3 semantics; no anchor-row FOR UPDATE unless justified.
4. **Contract regen**: update route/DTO docs, regenerate `backend-rs/openapi.json` and `frontend/src/lib/api/schema.d.ts`, run `gen:api -- --check`; update `CONTRACT-CHANGES.md` if used.
5. **Frontend TDD (red)**: pure `resolveCardDrop` tests (over card, container/end, top/start, empty, hidden-only, unloaded tail, unknown `matching_cards` fail closed, cross-column payload includes `column_id`); active-key cache writes; no dual-cache writes; no fake position reindex; `matching_cards`/`total_cards` deltas; blocked feedback no mutation and repeat-announced polite status.
6. **Frontend DnD refactor**: add board-scoped anchor-move mutation/wrapper; make context filter/key aware with captured `activeKey`; stable sensors; handler-level guards; remove blanket filter disable; remove DnD dependency on `useMoveCard/useReorderCard`.
7. **Optimistic/cache coherence**: active filtered cache only; no `['cards']` optimism; no global `['columns']` invalidation; prefix invalidation on settle/error/cancel/blocked rollback; define filter-change-during-flight behavior.
8. **UX/a11y**: banner/copy update, persistently mounted polite status, neutral/default non-assertive toast, no focus steal, sort/move-all still disabled.
9. **E2E amendments**: rewrite `board-search-filter.spec.ts` old guard test; add allowed filtered cross-column/same-column DnD persistence, blocked ambiguous case (visible toast + polite `dnd-blocked-status`, zero guarded mutations), and still-disabled sort/move-all. Follow API-seeded/no-waitForTimeout/scoped/virtualization-safe/bounded-scroll rules. Keyboard DnD covered under same rules or explicitly deferred with justification.
10. **Verification gates**: backend fmt/clippy/tests on MySQL 8.4 + MariaDB 11.4 tmpfs; frontend typecheck/lint/unit; targeted Playwright; full e2e confidence run as needed.

## Constraints

- **No ambiguous position writes against hidden cards.** Primary invariant.
- Additive backend contract only; same-PR OpenAPI/schema/typegen update.
- No existence oracle for anchor ids; same generic 400 `validation-error` shape/message across invalid anchor-scope cases (field differs only).
- Cross-column anchor/append moves must include `column_id`; backend must not infer target from anchor id.
- Keep `total_cards` unfiltered authoritative and `matching_cards` filtered authoritative (T049).
- `move-all` and `sort` remain disabled under active filters/search.
- React Query: no dual DnD cache; no `['cards']` optimistic writes for board DnD; no global `['columns']` invalidation for board-scoped DnD moves; invalidate `['columns', boardId]` prefix + `['board-cards', boardId]`.
- No fake full-column positions in filtered optimistic cache.
- No `waitForTimeout` in e2e; API-seeded setup; virtualization-safe assertions; scoped assertions in shared `kanpad_e2e`.
- MySQL 8.4 and MariaDB 11.4 compatibility.

## Acceptance Criteria

- [ ] T050/T051 (and affected e2e pre-dev checklist comments) are reconciled against the repo-reality delta checklist; runtime/tests/docs no longer assert blanket filtered card DnD disable except sort/move-all and list-view no-DnD.
- [ ] Backend `PATCH /api/v1/cards/{id}` supports additive `before_card_id`/`after_card_id`, with move-path trigger, mutual exclusivity, `position: null` treated as absent, cross-column target resolution requiring `column_id`, and same generic no-oracle 400 `validation-error` for invalid anchor-scope/self/concurrent-missing cases.
- [ ] Anchor-based moves are correct on MySQL 8.4 and MariaDB 11.4 for same/cross-column moves, including scalar+anchor; existing point-move/rebalance/retry/D3 semantics preserved.
- [ ] When search/filter is active, board-view users can drag/drop cards across columns and reorder within a column when drop resolves to a safe visible anchor (over-card, container/end, top/start, empty column); move persists after reload under the active filter and global order is correct after clearing filters.
- [ ] Ambiguous drops (no visible anchor, unloaded matching tail, hidden-only target, unknown `matching_cards` for ambiguous end/container) are blocked with neutral/default non-assertive toast (title `"Can't drop here while filters are active."`) + persistently mounted polite `aria-live` region (`data-testid="dnd-blocked-status"`, `role="status"`, `aria-live="polite"`), repeated announcements for identical blocked drops, no focus steal, and no mutation request.
- [ ] Sort-by-date and Move All Cards remain disabled under active search/filter with explicit affordance/copy.
- [ ] Filtered DnD uses captured active filtered query cache for optimistic UI; no dual-cache writes; no fake filtered-position fabrication; cross-column moves adjust `matching_cards` and `total_cards` by 1; same-column reorders leave counts unchanged; board-scoped prefix invalidation on settle/error/cancel/blocked rollback; no global `['columns']` invalidation for board DnD.
- [ ] E2E covers allowed filtered cross-column/same-column DnD (including empty-column and top/start anchor where feasible), blocked ambiguous case with visible+polite feedback and zero guarded mutations, and still-disabled sort/move-all, using API-seeded/no-waitForTimeout/scoped/virtualization-safe/bounded-scroll patterns. Keyboard DnD covered under same rules or explicitly deferred with justification.
- [ ] Frontend/backend tests and typegen checks pass; no regression in unfiltered DnD behavior.

## Risks / Open Questions

- **Hidden gap UX**: accepted by product decision; moved card may appear before hidden cards after filter clear. Keep copy/docs clear.
- **Pointer-midpoint before/after**: deferred for v1; deterministic before-over-card rule chosen. Add follow-up if UX testing shows ambiguity.
- **Keyboard/container-over mapping under virtualization**: deterministic resolver is required; keyboard moves across unmounted virtual items may be flaky in e2e. Cover resolver/unit behavior; add narrow Playwright keyboard case only if stable, otherwise defer with justification.
- **Legacy move/reorder hooks**: `useMoveCard`/`useReorderCard` have `['cards']` optimism and different invalidation semantics; DnD must not use them. Audit/remove/isolate remaining callers.
- **Same-column settle convergence**: invalidate board-scoped columns prefix on settle to avoid hidden-gap drift.
- **Concurrent anchor changes**: anchor deleted/moved out concurrently should yield same generic 400; treat as user-action retryable failure, not 409/503. Test where practical.

## Notes

- Investigation basis (2026-11-26): `drag-drop-context.tsx` hardcoded `['columns', boardId]` and filtered-index positions; `useColumns`/`columnQueryKey` create filtered keys; backend move clamps integer position to full-column `others_len`; CardSummary/embed filtered positions are returned-set relative in `card_summary.rs` and `cards/store.rs`; `BoardCardListItem.position` is per-column stable sort-key computed.
- Review basis (2026-11-26/27): 4 read-only subagent reviews (backend, frontend, spec/e2e, security/edge) + web research for product decisions.
- Web research references: LexoRank filtered reorder stability (appcraft.nl), Atlassian Pragmatic DnD design guidelines, MDN `aria-live`, Chrome drag-and-drop/toast accessibility guidance, Primer drag-and-drop a11y, WP Accessibility toasts.
- Small cleanup included: always pass stable `sensors={sensors}` to DndContext to avoid React 19/dnd-kit dep-array-size warning.
- This task is a behavior/spec amendment, not a silent relaxation of T050/T051 guardrails.