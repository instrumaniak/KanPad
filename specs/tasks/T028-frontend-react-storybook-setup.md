# T028: frontend react storybook setup

> **STATUS: pending.** Wave 0. Frontend component workshop (dev + docs + story tests).

## Goal

Set up **Storybook 10.6** (latest stable) in `frontend/` as an isolated workshop for
building, documenting, and testing UI components — integrated with the existing stack
(React 19 + Vite 8 + Tailwind v4 + Vitest 5 + Playwright), not bolted on:

1. **Scaffold** via the official installer (`npx storybook@latest init --yes --features
   docs test a11y`); audit the installer's own edits before hand-editing anything.
2. **Config alignment** — Tailwind v4 CSS + Noir tokens loaded; light/dark theme
   toolbar with full `use-theme.ts` parity (both classes + `localStorage`); provider
   decorators matching `App.tsx` nesting (QueryClient → MemoryRouter → ToastProvider
   → AuthContext); **MSW** (`msw` + `msw-storybook-addon`) as the only network
   boundary (`onUnhandledFrame` → unmatched `/api` fails loudly via
   `defaults.error()`);
   `tags: ['autodocs']` + axe a11y gating; tsconfig + ESLint coverage for
   `.storybook/` and `*.stories.tsx`.
3. **First stories** — exhaustive inventory (below) of UI-kit, presentational, and
   interactive feature components as **CSF 3** colocated `*.stories.tsx`, each with
   an autodocs page; data-driven components cover default/loading/empty/error states.
4. **Story tests** — `@storybook/addon-vitest`: restructure `vitest.config.ts` into
   `test.projects` (jsdom `unit` + browser-mode `storybook`), with **`npm test`
   scoped to `--project=unit`** so existing CI and dev machines are untouched;
   stories run via a separate `test-storybook` script, with axe a11y checks running
   inside those tests via `parameters.a11y.test`.
5. **CI** — new workflow gated on **lint + `build-storybook` + `test-storybook`**
   (tsc + unit tests already run in `backend-rs.yml`, which this task must not edit).

## Depends On

(None)

## Phase

0

## Critical

No — developer tooling, no production/runtime impact; blocks nothing.

## Spec References

- Storybook 10.6 docs (Context7 `/storybookjs/storybook` + storybook.js.org, Nov 2026):
  react-vite framework, Vite builder auto-merging `vite.config.ts` + `viteFinal`,
  CSF 3 scaffolding / CSF Next `definePreview` preview API (experimental in 10.6),
  `@storybook/addon-vitest` (`test.projects`, inline projects default
  `extends: false`, browser mode), toolbars-and-globals (`initialGlobals`, not
  `defaultValue`), migration guide (ESM-only, Node ≥ 20.19/22.12).
- Mocking/testing docs (storybook.js.org, Nov 2026): network mocking via **MSW**
  (`msw-storybook-addon` v3, `beforeEach({ msw })` handlers — sanctioned form; `parameters.msw` is deprecated,
  `delay('infinite')` for loading states, `onUnhandledFrame`); module mocking via
  **`sb.mock`** (Storybook 10.1+, registered only in `.storybook/preview.ts`,
  per-story control via `mocked()` in `beforeEach` — the sanctioned replacement for
  `vi.mock`); a11y gating via `parameters.a11y.test: 'error' | 'todo'` (the addon's
  `afterEach` throws in standalone vitest runs); autodocs via `tags: ['autodocs']`.
- Vite 8 support: storybookjs/storybook PR #33788 (merged 2026-03-10); known runtime
  issues #35332 (`__commonJS is not defined`, fixed ≥ vite 8.1.4) and rolldown
  `__STORYBOOK_MODULE_*__ is not defined` in built output (storybookjs/storybook
  #33797, vitejs/rolldown-vite #562) — hence the runtime-smoke AC.
- Vitest 5 projects docs (vitest.dev/guide/projects) — `vitest run` executes all
  projects; filtering needs `--project` (drives the `npm test` scoping decision).
- `specs/arch-hardening-specs/00-overview.md` — Global DoD (TDD, **red-proof any new
  gate**, gate set incl. `gen:api --check`); `docs/lessons/ci-quality.md` (red-proof
  gates, verify workflow location).
- `specs/security/08-supply-chain.md` + `.github/workflows/backend-rs.yml` — CI
  conventions: SHA-pinned `uses:` with `# vX` comment, self-file in `paths`,
  `concurrency`, `permissions`, `setup-node` node 22 + `cache-dependency-path:
  frontend/package-lock.json`, `working-directory: frontend`.
- `frontend/vite.config.ts` (React Compiler is the **separate**
  `@rolldown/plugin-babel` entry; plain `react()` plugin; `@` alias; `tailwindcss()`
  — builder auto-merges all of this for `storybook dev/build`, but NOT for the
  Vitest storybook project), `frontend/src/index.css` (Tailwind v4 + Noir tokens;
  `--background` = `#FAFAFA` light / `#0B0C0F` dark; Inter woff2 fetched from
  `fonts.gstatic.com`), `frontend/src/hooks/use-theme.ts` (`applyTheme` removes BOTH
  `light`+`dark` classes, persists `localStorage['theme']`, follows system
  preference), `frontend/src/App.tsx` (provider nesting), `frontend/src/lib/api/client.ts`
  (`apiFetch` → `fetch(BASE + path)`, no proxy in Storybook), `frontend/src/lib/api/schema.d.ts`
  (OpenAPI-generated types — fixture typing), `frontend/vitest.config.ts`,
  `frontend/tsconfig*.json` (solution-style root references), `frontend/eslint.config.js`,
  `frontend/CONVENTIONS.md`.
- `specs/tasks/status.yaml` — T028 registry entry (phase 0, critical false).

## Files to Create/Modify

| File | Change |
|---|---|
| `frontend/.storybook/main.ts` | **NEW** (via init) — framework `@storybook/react-vite`, stories glob `../src/**/*.stories.@(ts|tsx|mdx)`; `staticDirs: ['../public']` for the MSW worker (verify init didn't already set it); **contingency**: `viteFinal` filters out ONLY the `@rolldown/plugin-babel` entry (by plugin `name`) if standalone build fails — the existing `react()` plugin is already merged; never add a second React plugin |
| `frontend/.storybook/preview.ts` | **NEW** (via init, keep its **CSF 3** shape — do NOT convert to `definePreview`) — import `../src/index.css`; theme toolbar (`globalTypes.theme` + `initialGlobals: { theme: 'light' }`) with decorator setting/removing BOTH `light`+`dark` classes on `<html>` AND `localStorage['theme']`, restoring in cleanup; global decorators: fresh-per-story `QueryClientProvider` → `MemoryRouter` → `ToastProvider` → `AuthContext.Provider` (fake user, per-story override); **MSW registration (v3, all-latest / MSW 3)**: `mswLoader(async () => { const worker = setupWorker(); await worker.start({ onUnhandledFrame({ frame, defaults }) { if (frame.protocol === 'http') { const request = (frame as { data?: { request?: Request } }).data?.request; if (request && new URL(request.url).pathname.startsWith('/api')) defaults.error(); } } }); return worker })`, importing `mswLoader` from `msw-storybook-addon/csf3` + `setupWorker` from `msw/browser` (no `initialize` — removed in v3); note MSW 3 ignores `onUnhandledRequest` (legacy key) and uses `onUnhandledFrame`; the default worker already ignores common asset/Storybook-internal requests; the callback keeps unmatched `/api/*` failing loudly (red-proof, via `defaults.error()` which responds 500) without false-failing the Inter font fetch or HMR. Handlers live in `src/mocks/handlers.ts`, fixtures typed `satisfies` against `@/lib/api/schema.d.ts`; wire per-story via `beforeEach({ msw })` (v3 sanctioned form; `parameters.msw` is deprecated). For query-driven stories, additionally prefill via `client.setQueryData(queryKey, fixture)` using the real queryKey from the feature's `use-*.ts`; `tags: ['autodocs']`; a11y test mode (Step 2) |
| `frontend/public/mockServiceWorker.js` | **NEW** — MSW worker via `npx msw init public/ --save`; served by `staticDirs` (row above) |
| `frontend/src/mocks/handlers.ts` (+ fixtures) | **NEW** — shared MSW handlers; fixtures typed `satisfies` against `@/lib/api/schema.d.ts` so backend schema drift becomes a compile error |
| `frontend/.storybook/tsconfig.json` | **NEW** — mirrors `paths` (`@/*`), `types: ["vite/client", "msw-storybook-addon/csf3"]` (vite/client for the `index.css` side-effect import; msw-storybook-addon/csf3 for `context.msw` augmentation in CSF 3), `jsx` from `tsconfig.app.json` |
| `frontend/tsconfig.json` | Add `{ "path": "./.storybook/tsconfig.json" }` to `references` (root is solution-style — a new project file alone is invisible to `tsc -b`) |
| `frontend/tsconfig.app.json` | Stories under `src/` stay **included** (free type coverage; they gate `npm run build` and CI's `tsc --noEmit -p tsconfig.app.json`) — record this decision in Notes; **also add `msw-storybook-addon/csf3` to `compilerOptions.types`** so story files get `context.msw`/`beforeEach({ msw })` typing even though `.storybook/preview.ts` is not in the `include` |
| `frontend/.storybook/*` (other) | init-generated assets; remove boilerplate example stories except the one kept for the Step 4 build gate |
| `frontend/package.json` | init devDeps (`storybook`, `@storybook/react-vite`, `addon-docs`, `addon-a11y`, `addon-vitest`, `@chromatic-com/storybook`, `eslint-plugin-storybook`, `playwright`, `@vitest/browser-playwright` — **pin `^5`** to match repo's `vitest@^5.1.0`, addon-vitest supports Vitest 5 (PR #36671); also keep `@playwright/test@^1.65.0` single-version (no duplicate playwright); + **`msw@^3.0.5`** + **`msw-storybook-addon@^3.0.3`** (all-latest; addon's README still says `msw@2.x` but Storybook's own eval fixture + Svelte skill confirm msw 3 works with addon 3); scripts: `test`/`test:watch` → `vitest run --project=unit` / `vitest --project=unit`; NEW `test-storybook: vitest run --project=storybook` (one-shot — not watch, not `@storybook/test-runner`; no dev server or build output needed), NEW `test:all: vitest run` (both projects); `storybook` / `build-storybook` from init |
| `frontend/package-lock.json` | UPDATED by init — commit only after local `npm ci` green |
| `frontend/vitest.config.ts` | Convert to `test.projects`: (1) `unit` with `extends: true` owning `environment/jsdom`, `globals`, `setupFiles`, `exclude: ['e2e/**', 'node_modules/**']`, `@` alias, `react()` plugin; (2) `storybook` with `extends: true`, `plugins: [storybookTest({ configDir }), tailwindcss()]`, `@` alias, browser mode chromium via `@vitest/browser-playwright`. Do NOT `mergeConfig(vite.config.ts)` (bleeds React-Compiler babel + `/api` proxy into story tests) |
| `frontend/tsconfig.app.json` | Stories under `src/` stay **included** (free type coverage; they gate `npm run build` and CI's `tsc --noEmit -p tsconfig.app.json`) — record this decision in Notes |
| `frontend/eslint.config.js` | Definite override: `files: ['**/*.stories.@(ts|tsx)', '.storybook/**/*.{ts,tsx}']` → `react-refresh/only-export-components: off` (rule trips on `meta`/story exports; add BEFORE writing stories, capture before/after lint output) |
| Story inventory (see §Story Inventory) | **NEW** — colocated `*.stories.tsx` per component; directory-based components (`button/`, `badge/`, `toast-provider/`) colocate inside their directory — a flat `ui/*.stories.tsx` glob does not reach them (`use-toast/` is a hook, out of scope) |
| `.github/workflows/frontend-storybook.yml` | **NEW** — conventions per `08-supply-chain.md`: SHA-pinned `uses:` with `# vX` comments; `permissions: contents: read`; `concurrency` group; `defaults.run.working-directory: frontend`; `setup-node` node 22 + `cache: npm` + `cache-dependency-path: frontend/package-lock.json`; `paths: ['frontend/**', '.github/workflows/frontend-storybook.yml']` + `workflow_dispatch`; steps: checkout → setup-node → `npm ci` → `npm run lint` → `npx tsc -b --force` → `build-storybook` → `npx playwright install --with-deps chromium` → `test-storybook`; caches `~/.npm`, `node_modules/.cache/storybook`, `~/.cache/ms-playwright` (keyed on lockfile + storybook version; there is NO `--disk` flag) |
| `frontend/.gitignore` | + `storybook-static/` |
| `frontend/CONVENTIONS.md` | NEW `## Storybook` section (**`### Rules` pattern, inserted before `## Lessons learned`**): CSF 3, colocation, provider decorators, MSW-only network mocking (no fetch monkey-patching, no `passthrough()`), `sb.mock`-only module mocking (never `vi.mock` in stories), state-coverage rule (default/loading/empty/error), story vs vitest test |
| `docs/lessons/frontend.md` | Dated, tagged lesson (React Compiler contingency, tsconfig references, init audit); state whether this file or `CONVENTIONS.md § Lessons learned` is canonical (both currently carry mirrored bullets) |

**Must NOT appear in the diff:** changes to existing `*.test.tsx` files, app runtime
behavior, `playwright.config.ts`, **`.github/workflows/backend-rs.yml`**, backend
files.

## Story Inventory

One row per story file (play = write a red-first `play` function; data = mock source).

| Batch | Components (source path) | Data / mock source |
|---|---|---|
| UI kit (`src/components/ui/`) | `alert-dialog`, `badge/`, `button/`, `calendar`, `checkbox`, `context-menu`, `dialog`, `dropdown-menu`, `input`, `popover`, `scroll-area`, `select`, `separator`, `sheet`, `skeleton`, `tabs`, `textarea`, `toast-provider/`, `tooltip` | args only; toast stories render via `ToastProvider` decorator |
| Presentational (`src/components/`) | `spinner`, `error-state`, `empty-state`, `loading-skeleton`, `card-list-skeleton`, `board-list-skeleton`, `form-skeleton`, `mobile-bottom-sheet`, `route-error-boundary`, `lazy-load-boundary` | args only |
| Feature | `features/cards/`: `card`, `card-preview`, `add-card-input`, `markdown-preview`; `features/tags/`: `tag-badge`, `tag-picker`; `features/boards/`: `board-card`; `features/auth/`: `login-form`, `register-form` | MSW handlers via `beforeEach({ msw })` (v3 sanctioned form; `parameters.msw` is deprecated), `delay('infinite')` for loading) + `setQueryData` prefills; auth via `AuthContext.Provider` override |

Out of scope (explicit): full pages/routes (`board-view`, `note-*`, `project-list`,
`board-list`), `layouts/*`, hooks, anything requiring a live backend.

Deferred to follow-up tasks (see Notes): visual regression, a
component-without-story CI coverage gate, optional MDX foundations/tokens page,
tightening a11y `'todo'` baseline to `'error'`.

## Implementation Steps

0. **Decompose** the steps below into `todowrite` items; story batches (Step 6) are
   independently dispatchable to parallel sub-agents (AGENTS.md).

1. **Scaffold + audit:** `npx storybook@latest init --yes --features docs test a11y
   --disable-telemetry` in `frontend/` (~3 min; Playwright download dominates).
   Then: `git diff` on `vitest.config.ts` / `eslint.config.js` / `tsconfig*` /
   `package.json` — **record init's own edits in Notes before steps 3–5** (init may
   pre-convert configs; don't double-apply or silently revert). `git status` audit
   per `docs/lessons/frontend.md` (installers misplace files; never stage until
   verified). Expect `storybook@^10.6` + the package list above; `addon-mcp` only if
   `ai` feature requested (not requested — do not expect it). Keep **one** init
   example story until Step 4's build gate passes; strip the rest.

2. **Preview config (CSF 3) + mocking + docs/a11y tags:** keep init's
   `export default { … } satisfies Preview` shape — do NOT adopt `preview.meta()`
   (experimental in 10.6; CSF 4 scaffolding lands in v11). Add:
   - index.css import;
   - theme toolbar per Files row (parity with `applyTheme` — both classes +
     `localStorage`, `initialGlobals` for deterministic browser tests, restore in
     cleanup);
   - provider decorators (fresh `QueryClient` per story so caches don't leak;
     `MemoryRouter`; `ToastProvider`; `AuthContext.Provider` with
     `{ user, isLoading: false, login, register, logout }` — per-story override
     for logged-out variants);
   - **MSW (the network boundary, all-latest / MSW 3):** `npm i -D msw@^3.0.5
     msw-storybook-addon@^3.0.3`; generate the worker (`npx msw init public/
     --save`); ensure `main.ts` serves it (`staticDirs: ['../public']` — verify
     init didn't already); register in preview.ts with the **MSW 3** form (no
     `initialize` — removed in v3; MSW 3 ignores the legacy `onUnhandledRequest`
     key and uses `onUnhandledFrame`):
     `mswLoader(async () => { const worker = setupWorker(); await worker.start({ onUnhandledFrame({ frame, defaults }) { if (frame.protocol === 'http') { const request = (frame as { data?: { request?: Request } }).data?.request; if (request && new URL(request.url).pathname.startsWith('/api')) defaults.error(); } } }); return worker })`,
     importing `mswLoader` from `msw-storybook-addon/csf3` and `setupWorker` from
     `msw/browser`. The default worker already ignores common asset/Storybook-
     internal requests; the callback keeps unmatched `/api/*` failing loudly
     (red-proof via `defaults.error()`, which responds 500) without false-failing
     the Inter font fetch or HMR. **Red-proof caveat (MSW 3):** `defaults.error()`
     answers with a 500 instead of throwing, so a component that handles errors can
     still render — record unhandled `/api` requests in a `Set` inside the callback
     and assert it is empty in `afterEach`, or assert the component's error UI.
     Handlers live in `src/mocks/handlers.ts`, fixtures typed `satisfies` against
     `@/lib/api/schema.d.ts`; wire per-story via `beforeEach({ msw })` (v3
     sanctioned form — `parameters.msw` is deprecated). For query-driven stories,
     additionally prefill via `client.setQueryData(queryKey, fixture)` using the
     real queryKey from the feature's `use-*.ts`;
   - `tags: ['autodocs']` (verify init didn't already set it) + a11y mode:
     `parameters.a11y.test` = `'error'` for stories written in this task, `'todo'`
     for pre-existing-component stories until the baseline violation count is
     recorded in Notes (tighten in a follow-up).
   **Forbidden in `*.stories.tsx`:** `vi.mock` / vitest imports (stories must load
   in `storybook dev` without a vitest runtime — module-level mocking, if ever
   needed, = `sb.mock(import(...))` registered in preview.ts only, controlled per
   story via `mocked()` in `beforeEach`), custom `globalThis.fetch`
   monkey-patching, real network calls, `passthrough()` in committed stories.

3. **TypeScript + lint + sample story:** create `.storybook/tsconfig.json` + root
   reference (Files row); add the ESLint override **before** writing stories. Write
   ONE sample story (UI kit `button`), then run `npx tsc -b --force` **and**
   `npx tsc --noEmit -p tsconfig.app.json` (the exact commands CI runs) + `npm run
   lint` — paste outputs. Record in Notes that stories intentionally remain inside
   `tsconfig.app.json`.

4. **Build gate + React-Compiler contingency:** `npm run build-storybook` (runs with
   ≥1 story — the kept sample). If it fails on the compiler babel plugin: add
   `viteFinal` in `main.ts` filtering **only the `@rolldown/plugin-babel` entry by
   plugin `name`** (the plugin's name is `"@rolldown/plugin-babel"` — verified in
   `node_modules/@rolldown/plugin-babel/dist/index.mjs`; the plain `react()` plugin
   is already present and merged —
   adding another React plugin is a documented failure mode); verify build output
   shows exactly one React plugin, zero compiler-babel plugins, and that `@` alias +
   Tailwind still work. **Runtime smoke:** open the built preview (or run
   `test-storybook`) and assert zero `__STORYBOOK_MODULE_*__ is not defined` /
   `__commonJS is not defined` console errors; record installed `vite` version in
   Notes (must be ≥ 8.1.4 if 8.1.x).

5. **Vitest restructure:** convert `vitest.config.ts` per Files row (`extends: true`
   on both projects; inline projects do NOT inherit root `test.*` — move options
   explicitly). Update scripts per Files row. Gates: (a) `npm test` runs **0** story
   files and needs no browser — `npx vitest list | grep -c '\.stories\.'` = 0;
   (b) all pre-existing suites still green; (c) `npm ls playwright` shows a single
   version (if two, pin `@vitest/browser-playwright` to match `@playwright/test@1.65`);
   (d) after `npx playwright install chromium`, `npm run test-storybook` runs the
   sample story green — proving `@/` alias + Tailwind compile inside the storybook
   project (assert a computed token, see AC); (e) a story with an MSW handler
   renders its fixture under `test-storybook` — proving the service worker is
   active in vitest browser mode.
   Note: addon-vitest >= 10.3 auto-provisions `setProjectAnnotations` (no manual
   `.storybook/vitest.setup.ts` needed); keep `extends: true` on both projects
   (default in Vitest 5.0) and move unit-only `setupFiles`/`environment` into the
   `unit` project per the addon docs.

6. **Story inventory batches** (per §Story Inventory; CSF 3, colocated, controls +
   docs): (a) UI kit — mind directory-based components; (b) presentational;
   (c) feature components with realistic args, MSW handlers / `setQueryData` data,
   auth decorator, state coverage (default / loading via `delay('infinite')` /
   empty / error) for data-driven components, and red-first `play` functions for
   interactions (e.g. `login-form` mocked submit, `tag-picker` selection,
   `mobile-bottom-sheet` open/close).

7. **Red-proof the new gates** (Global DoD `00-overview.md:47-48`): temporarily
   break one story assertion → `npm run test-storybook` exits non-zero → revert;
   temporarily remove one MSW handler from a story that asserts its fixture →
   non-zero exit (proves the `onUnhandledFrame` callback calls `defaults.error()`
   for `/api` is live; MSW 3 answers with a 500, so assert the error UI or an
   empty unhandled-request `Set`) → restore;
   introduce a probe lint failure → workflow run goes red (verify workflow location
   per `docs/lessons/ci-quality.md`) → revert. Paste all failing outputs.

8. **CI workflow:** `.github/workflows/frontend-storybook.yml` per Files row.
   Add an explicit `npx tsc -b --force` step (before `build-storybook`) so a type
   error in `.storybook/` or story files fails this gate directly, not only via
   `npm run build` in `backend-rs.yml`. Gate ownership otherwise unchanged: this
   workflow = lint + tsc + storybook build/test only; `backend-rs.yml` (web/
   docker-e2e) still owns unit vitest and `npm run build` — **that file must not
   change**.

9. **Docs:** `## Storybook` rules → `frontend/CONVENTIONS.md` (before `## Lessons
   learned`); dated, tagged lesson → `docs/lessons/frontend.md` (state canonical vs
   mirrored). *Optional (nice-to-have, not blocking):* an MDX foundations page
   (Noir palette + typography via `<Color />`/`<Type />` doc blocks) — only if time
   permits, else defer per Notes.

10. **Verification pass:** all ACs below; `tsc -b`, `tsc --noEmit -p
    tsconfig.app.json`, `npm run lint`, `npm test` (0 stories), `npm run
    test-storybook`, `npm run build-storybook`, `npm run gen:api -- --check` all
    green.

## Constraints

- **Storybook 10 stable only** — no v11 beta; ESM config files in `.storybook/`.
- **CSF 3, not CSF 4** — no `definePreview()`/`preview.meta()` in 10.6; revisit at
  the v11 upgrade (Notes).
- **`npm test` stays jsdom-only and browser-free** — scoped to `--project=unit`;
  `backend-rs.yml` unchanged and its `docker-e2e` job (runs `npm run test` before
  any Playwright install) stays green.
- **React Compiler stays enabled for the app** — any `viteFinal` strip applies only
  to Storybook's build and removes only `@rolldown/plugin-babel`; never touch
  `vite.config.ts`; never duplicate the React plugin.
- **Theme behavior parity** — same classes + `localStorage` semantics as
  `use-theme.ts` (no forked token system).
- **No real network calls in stories** — MSW is the only network boundary, wired
  via the MSW 3 form (`mswLoader` + `worker.start({ onUnhandledFrame })`); unmatched
  `/api/*` calls `defaults.error()` (red-proof — responds 500) while asset/font
  requests bypass so they don't false-fail; no custom `fetch` monkey-patching; no
  `passthrough()` in committed stories. Note: MSW 3 ignores the legacy
  `onUnhandledRequest` key, and `defaults.error()` responds rather than throws.
- **No `vi.mock` in story files** — the sanctioned tool is `sb.mock(import(...))`
  registered only in `.storybook/preview.ts` (Storybook 10.1+), controlled per
  story via `mocked()` in `beforeEach`.
- **Callback mocking in stories uses `fn`/`spy` from `@storybook/test`** — never
  `vi.fn`/vitest imports in `*.stories.tsx` (stories must load in `storybook dev`
  without a vitest runtime).
- **a11y gates are real** — new stories run `parameters.a11y.test: 'error'`;
  pre-existing-component stories start at `'todo'` with the violation count
  recorded in Notes (tighten later, don't silently skip axe).
- Follow `frontend/CONVENTIONS.md` (apiFetch, Radix, targeted cache invalidation)
  in decorator/story code.
- Verify uncertain details against current Storybook/Vitest/MSW docs (AGENTS.md);
  add lessons for anything non-obvious.
- Global DoD (TDD, red-proof new gates, `gen:api --check`); **no commit without
  approval**.

## Acceptance Criteria

- [ ] `npm run storybook` starts; sidebar lists every component in the §Story
      Inventory (out-of-scope list explicitly absent).
- [ ] `tags: ['autodocs']` active — an autodocs Docs page renders for inventoried
      components (screenshot/list pasted).
- [ ] `npm run build-storybook` clean; built preview shows **zero**
      `__STORYBOOK_MODULE_*__` / `__commonJS is not defined` errors (vite version
      ≥ 8.1.4 if 8.1.x, recorded in Notes); contingency path taken or not, noted.
- [ ] Token check proves CSS pipeline: a story test asserts
      `getComputedStyle(document.documentElement).getPropertyValue('--background')`
      is `#FAFAFA` (toolbar light) and `#0B0C0F` (dark) — output pasted; **and**
      `grep -r -- '--background' frontend/storybook-static/assets/*.css` succeeds
      after `build-storybook`.
- [ ] Theme decorator parity: sets/removes both `light`+`dark` classes and
      `localStorage['theme']`, restores in cleanup; `initialGlobals` pinned.
- [ ] MSW is the live network boundary: fixtures served by handlers; removing a
      handler fails the run (`onUnhandledFrame` calling `defaults.error()` for `/api`
      red-proof pasted, Step 7 — MSW 3 responds 500, so assert error UI or an
      empty unhandled-request `Set`);
      fixtures typecheck against `schema.d.ts` (a deliberate field rename breaks
      `tsc` — probe optional but encouraged).
- [ ] Feature stories: `test-storybook` output shows **zero console-error
      annotations** for feature stories (paste); story count in `test-storybook`
      output **≥ inventory count** (paste count — guards against silently running
      0 stories).
- [ ] a11y active: `parameters.a11y.test` set (`'error'` for new stories); pasted
      `test-storybook` a11y summary — zero violations on new stories, baseline
      `'todo'` count for pre-existing-component stories recorded in Notes.
- [ ] `npm test` runs 0 story files and requires no Playwright browser
      (`npx vitest list | grep -c '\.stories\.'` = 0); all pre-existing unit suites
      green.
- [ ] `npm run test-storybook` green locally after `npx playwright install
      chromium`; `npm ls playwright` shows a single version.
- [ ] Red-proofs pasted: broken story assertion → non-zero exit; removed MSW
      handler → non-zero exit (or asserted 500/error UI); probe lint failure →
      workflow red.
- [ ] `npx tsc -b --force`, `npx tsc --noEmit -p tsconfig.app.json`, and
      `npm run lint` clean (`.storybook` + story files covered; override in place);
      `npm run gen:api -- --check` green.
- [ ] CI workflow present, convention-compliant (SHA pins, self-path,
      `workflow_dispatch`), green via `workflow_dispatch` or first PR run — link/SHA
      pasted in Notes (until then: syntax-valid + first-run documented).
- [ ] `frontend/CONVENTIONS.md` `## Storybook` section added (incl. MSW, `sb.mock`,
      state-coverage rules); dated, tagged lesson appended to
      `docs/lessons/frontend.md`; `.gitignore` has `storybook-static/`;
      `package-lock.json` committed with local `npm ci` green.

## Notes

(filled in during/after implementation)

Research decisions (Nov 2026, revised 2026-11-13): Storybook **10.6** stable over v11
beta; official `init` over manual install; `@storybook/addon-vitest` (not
`test-runner`) per Storybook 10 direction (supports Vitest 5, PR #36671; addon-vitest
≥ 10.3 auto-provisions `setProjectAnnotations`); **CSF 3** (init scaffolds it; CSF 4
`definePreview` is preview/experimental until v11); MSW via **msw-storybook-addon v3**
(no `initialize`; `mswLoader` factory from `msw-storybook-addon/csf3`; handlers via
`beforeEach({ msw })` — `parameters.msw` deprecated); **all-latest MSW**:
`msw@^3.0.5` + `msw-storybook-addon@^3.0.3` (Storybook's own agent-eval fixture
and the Svelte skill confirm msw 3 works with addon 3 despite the addon README
still saying `msw@2.x`); uses `onUnhandledFrame({ frame, defaults })` (MSW 3 ignores
the legacy `onUnhandledRequest` key) and `defaults.error()` which responds 500 rather
than throwing — red-proof records unhandled `/api` requests in a `Set` and asserts
it empty in `afterEach` (or asserts the component's error UI); **`npm test` scoped to
`--project=unit`** (a
browser project inside the default run would break `backend-rs.yml`'s `docker-e2e`,
which runs `npm run test` before installing Chromium); **`extends: true` + explicit
`tailwindcss()`** for the storybook vitest project instead of `mergeConfig` (avoids
bleeding React-Compiler babel + `/api` proxy into story tests); **MSW v3**
(`msw` + `msw-storybook-addon`) as the network boundary — the ecosystem standard
per official Storybook docs, Chromatic, and Red Hat's 2026 writeup; chosen over a
bespoke `fetch` stub (non-standard, shallower interception, no handler reuse with
vitest unit tests) and over `vi.mock` (incompatible with `storybook dev`);
fixtures typed against the OpenAPI-generated `schema.d.ts` so mock/real drift is a
compile error; **`sb.mock`** named as the only sanctioned module-mocking API;
a11y via `parameters.a11y.test` (`'error'` new / `'todo'` baseline) since the
addon's axe checks fail standalone vitest runs; `autodocs` tag verified rather
than assumed; CI + feature-component stories included per user decision (feature
decorators are the main cost).

Deferred to follow-up tasks: **visual regression** (Vitest browser mode's
`toMatchScreenshot()` with in-repo baselines, or Chromatic's 5,000-snapshot free
tier — both need deterministic rendering/pinned fonts first); a **component-without-
story coverage gate** (5-line CI script, premature until the inventory lands);
optional **MDX foundations/tokens page** (nice-to-have in Step 9); **tightening
a11y** `'todo'` baseline stories to `'error'`.

Risks: **R-1** init adds ~15–25 MB deps + Chromium + MSW worker download → first
CI run slow (cache `~/.cache/ms-playwright` keyed on playwright version). **R-2**
CI egress needs npm registry + Playwright CDN. **R-3** `index.css` `@font-face`
fetches `fonts.gstatic.com` at story runtime — non-blocking (`font-display: swap`)
but exempt from the console-error AC; consider self-hosting the woff2 later.
**R-4** telemetry disabled via flag. **R-5** local Node 26 is outside Storybook's
tested matrix (CI pins Node 22) — record any init/build anomalies in Lessons.
**R-6** MSW service worker in vitest browser mode is gated explicitly (Step 5e) —
if it proves flaky, fall back to the previously planned `fetch`-stub decorator and
record the deviation in Notes.

## Lessons

(pending)
