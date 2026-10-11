# 40 — Typegen pipeline (Nest → openapi-typescript → frontend)

## Decision: `openapi-typescript` types-only (not orval/hey-api)

Frontend today: native `fetch`, 9 hand-rolled `*.api.ts` files each copy-pasting
`FETCH_OPTIONS{credentials:include}` + `handleResponse` (reads Nest
`{message:string|string[]}`, throws plain `Error` discarding `status`), hand
`interface`s preserving snake_case (`column_id,created_at,has_description`) with
duplications: `Label.color` drift (`LabelColor` union in `cards.api.ts:1` vs plain
`string` in `columns.api.ts:1-7`), `Tag` ×2 (`tags.api.ts` full shape with
`user_id,created_at` vs `notes.api.ts` minimal `id/name/color`), `Card` ×2
(`cards.api.ts` vs `columns.api.ts`), and 6 local `ApiResponse<T>` defs
(auth, cards, columns, boards, projects, labels — tags/notes/checklists import
from cards). `tsconfig.app.json` requires
`strict + erasableSyntaxOnly (no enums/namespaces) + verbatimModuleSyntax (import type) + bundler`
with `skipLibCheck: true`, and excludes `**/*.test.*` from `tsc -b`.
No `zod/orval/hey-api` installed; `vite.config.ts` proxies `/api → :3000`;
all 9 modules hardcode relative `/api/...` URLs, no `VITE_API_URL` env.

`orval`/`hey-api` not adopted: both would add a generator runtime plus either
validators or generated fetch client / React Query hooks, which would force a
rewrite of all 9 modules and their tuned `use-*` optimistic/drag-drop caches.
This is a footprint + cache-preservation call, not a hard rejection — if the
repo later wants validators or hook generation, re-evaluate. `openapi-typescript`
wins today: zero runtime, pure `type`s, single `schema.d.ts`, incremental
adoption (alias existing names to `components["schemas"]`), preserves
`credentials:include` + proxy + envelope unwrapping.

## Pipeline (backend-rs utoipa export — rewrite; supersedes the old Nest SwaggerModule pipeline)

```
backend-rs: #[utoipa::path] handlers + ToSchema DTOs, one ApiDoc in src/docs.rs
  → cargo run -p backend-rs -- export-openapi > backend-rs/openapi.json (committed, CI-diffed;
    also served live at GET /api-docs/openapi.json; Swagger UI at /swagger-ui)
  → frontend: npx openapi-typescript ../backend-rs/openapi.json -o src/lib/api/schema.d.ts (default -o output)
  → hand frontend/src/lib/api/client.ts (apiFetch) consumed per-feature
```

Backend rules: utoipa 5 + utoipa-axum (no Nest `DocumentBuilder`/`SwaggerModule`/`@ApiResponse`
decorators — annotations live in `#[utoipa::path]` + `ToSchema` derives; wiring/gating is T40a).
Envelopes are part of the spec: `Page<T>` (paginated lists) and the RFC9457 problem+json error
schemas generate as real components. DTO nullability follows the `20-api-contract.md` Nullability table (absent-vs-null,
e.g. `checklist_progress` omitted when empty vs `description: null` emitted) — do not re-decide per feature. Paginated endpoints exist, so the frontend DOES take on
page-in-queryKey work where lists paginate (T41b+).

Frontend rules: `npm i -D openapi-typescript@<exact>` (exact version pinned in `package.json`, no floating `^`; T40b records the chosen version here in the same PR); add `gen:api` script
(`openapi-typescript ../backend-rs/openapi.json -o src/lib/api/schema.d.ts`,
no `--enum` flag — `erasableSyntaxOnly` forbids generated enums);
`schema.d.ts` never hand-edited (drift gate: `npm run gen:api -- --check`
regenerates to temp + `diff`, fails on stale output).

## `client.ts` spec (`src/lib/api/`)

- `ApiError extends Error { status: number; title?: string; detail?: string; errors?: unknown }`.
- `apiFetch<T>(path, init?)`: shared `FETCH_OPTIONS{credentials:include}` +
  JSON envelope handling; `204 → undefined as T`.
- `apiFetchVoid(path, init?)`: void-returning sibling for DELETE/204 endpoints.
- `BASE = import.meta.env.VITE_API_URL ?? ''` — default stays relative so the
  dev proxy keeps working; the `VITE_API_URL` switch points the
  frontend at `:3000` (Nest) vs `:3001` (Rust) during cutover — that switch is
  the only cutover concept the client needs. Owned by T40b (not T01: T01 only
  wires compose + Vite proxy; `client.ts` does not exist until T40b).
- Dual error parser: legacy Nest `{message: string|string[]}` OR
  `{title, detail, errors}`, always preserving `status` on the thrown `ApiError`.
- Update `getErrorMessage` (`src/lib/error-messages.ts:44`): its
  `status`-routing branch is currently dead code because every api function
  throws plain `Error(message)` discarding `status`; rewire it to read
  `ApiError.status`.
- Must cover all 3 existing `handleResponse` variants when unifying:
  1. standard (boards/projects/cards/columns/tags/notes/labels): `!ok → parse
     error.message → throw Error`; success → `response.json()` in try/catch
     throwing `Unexpected response format` on empty bodies;
  2. `auth.api.ts` inline-fetch: no shared helper — each function inlines its
     own error block; `loginApi` additionally try/catches `fetch` into
     `Network error`; `logoutApi` returns `Promise<void>` (bare, no envelope);
  3. checklists divergent: `!ok → errorBody.message || 'Request failed with
     status …'`; explicit `response.status === 204 → undefined as T`;
     `deleteChecklist`/`deleteChecklistItem` return bare `Promise<void>`.
- Explicit 204 path required: only labels + checklists handle 204 today; every
  other module throws `Unexpected response format` on an empty body (tags/notes
  tests already name 204 cases their handlers cannot pass).

## Type replacements + naming decisions

- 6 local `ApiResponse` defs collapse to one `client.ts` re-export; post-rewrite the two real envelopes are `ApiResponse<T>` (single values) and `Page<T>{data,page,limit,total}` (paginated top-level lists; hooks take on page-in-queryKey work where lists paginate). The Nest-only `ListResponse<T>{data,total}` is retired with the Nest envelope — do not carry it into `schema.d.ts`. Deletes are `204` — use `apiFetchVoid` (every module needs the explicit-204 path, not just labels/checklists).
- `CardSummary` vs `CardDetail` split is real and must survive typegen:
  columns list requires `has_description` (no TEXT) while cards detail carries
  optional `description?` + `has_description?`; map to two schemas, not one.
- `Label.color` drift (`LabelColor` union vs `string`) and `Tag` ×2 resolve to
  the generated schemas; whichever shape loses, its consumers update.
- Wire keys are `snake_case` byte-for-byte (`board_count`, `moved_count`,
  `tag_id`/`tag_ids`, request bodies `{label_id}`-style ids, query
  `?project_id=`) — recorded as naming decisions in `CONTRACT-CHANGES.md`,
  not refactors. The pre-cleanup verbatim mixed-case rule (`boardCount`,
  `movedCount`, `tagId`, `{labelId}`, `?projectId=`) is RETIRED; frontend
  keeps the new snake_case names verbatim from `schema.d.ts`.

## Migration order (hub-first)

> **Newly-strict input audit (rides every feature step):** the Rust rewrite validates inputs Nest
> silently ignored (bare `@IsNumber()` DTOs, manual `parseInt` → `NaN` filters, out-of-range
> `page/limit`, auth register/login inputs (no pipes in Nest), blank board-create names — all now **400 problem+json with `errors[]`**, see `20-api-contract.md`).
> When migrating each feature's fetchers/hooks, grep for query params it sends
> (pre-migration camelCase `?projectId=`, `?tagId=`, `?boardId=` alongside
> `?page=`, `?limit=`, `?type=`) and confirm none can
> be undefined/garbage at runtime (e.g. NaN from uninitialized state) — pre-sanitize in the
> hook or accept the 400 as a surfaced bug. Post-migration names are snake_case
> (`?project_id=`, `?tag_id=`, `?board_id=`); audit both during the transition.

1. `src/lib/api/`: `schema.d.ts` + `client.ts` + `ApiResponse`/`Page`/`ApiError` re-exports.
2. `cards` + `columns` together (shared `Card`/`Label` shapes; `columns.api`
   re-exports `ApiResponse` from its own def today).
3. Dependents: `labels`/`tags`/`notes`/`checklists` + `board-view` components.
4. `boards`/`projects`/`auth` last (auth's inline-fetch shape differs most).

Hook signatures WILL change where the contract changed — this is expected, not
a break to avoid: paginated lists gain page-in-queryKey, deletes become void
(`apiFetchVoid`), and renamed snake_case keys flow through. What must NOT
change is hook *behavior* (optimistic updates, drag-drop caches, unwrapping
`.then(r => r.data)` stays).

Per feature: replace local `interface Project/Board/Card/…` with
`import type { components } …; export type Project = components["schemas"]["ProjectResponse"]`
re-exports (imports keep working), then swap `handleResponse` bodies to
`apiFetch` one file at a time. Hooks keep `.then(r => r.data)` unwrapping.
`*.api.test.ts` rewrites happen at the `apiFetch` boundary (mock `apiFetch`,
assert full URL incl. `BASE`) with fixtures drawn from
`components["schemas"]`. CI must run `vitest` plus `tsc --noEmit` over tests
separately — `tsconfig.app.json` excludes `*.test.*` from `tsc -b`, so the
build alone never type-checks tests.

## CI gates

* Backend: export script `--check` mode (regenerate + `diff`) fails on
  uncommitted `openapi.json` drift.
* Frontend: `npm run gen:api -- --check` (regenerate to temp + `diff`) fails on
  stale `schema.d.ts`; `tsc --noEmit` (app) + `vitest` + separate
  `tsc --noEmit` over test files.
