# Lessons learned — Frontend

Source: extracted from root `AGENTS.md` (2026-11-10).

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

## Full dependency upgrade to latest (2026-11-13)
- `npx npm-check-updates -u` then `npm install` hit three peer conflicts, all resolvable without downgrading: (1) `typescript-eslint@8.72.2` peers `typescript <6.1.0` so TS must stay at `~6.0.3` (TS 7.0.2 blocked until ts-eslint ships support — `npm outdated` will keep showing it); (2) `openapi-typescript@7.13.0` (already latest) has a stale `typescript@^5.x` peer — fixed with `overrides: { "openapi-typescript": { "typescript": "$typescript" } }`, and `npm run gen:api -- --check` verifies it still works; (3) `@types/dompurify` is a deprecated stub (dompurify ships its own types) — removed rather than upgraded.
- TS 6 errors on `baseUrl` (TS5101, "will stop functioning in 7.0"). With `moduleResolution: "bundler"`, delete `baseUrl` and keep `paths` — paths then resolve relative to the tsconfig file; no `ignoreDeprecations` needed.
- The `shadcn` npm package is the CLI, not a runtime lib, but `src/index.css` does `@import "shadcn/tailwind.css"` — so shadcn@1.0.0 (a stub) breaks `vite build` with "Can't resolve 'shadcn/tailwind.css'". Baseline `npm run build` before upgrading; the working tree had shadcn downgraded to ^1.0.0 and the build was already red.
- A Playwright minor bump (1.63→1.65) requires `npx playwright install chromium` — the browser build revision changes (1243→1254) and tests fail at `chromium.launch()` with "Executable doesn't exist" otherwise.
- `npm audit` "fix available via --force" that proposes a *downgrade* (eslint@9, shadcn@1.0.0, typescript-eslint@8.36.0) means the advisory affects the latest version too — no real fix exists; don't `audit fix --force` and undo the upgrade.
- `pkill -f vite` from a shell whose own command line contains "vite" kills the invoking shell (exit 143). Match the node binary instead: `pgrep -af "node.*vite"`.

## Vite 8 `configLoader: 'native'` warning (2026-11-13)
- Tag: `frontend`
- Vite 8 warns "unsupported by `configLoader: 'native'`" for `__dirname` in `vite.config.ts` (and `vitest.config.ts`, which reuses Vite's config loader — same warning on `npm run test`). `configLoader: 'native'` (plain native Node ESM import of the config) becomes the default in a future major, where `__dirname` is undefined in ESM scope. Use `import.meta.dirname` instead (Node ≥20.11, already implied by Vite 8's engines `^20.19 || >=22.12`).
- `import.meta.dirname` works under **both** loaders: Vite's `bundle` loader rewrites it to an injected `const __vite_injected_original_dirname = <config dir>`, and native Node provides it natively. Verify with `npx vite --configLoader native` (it crashes on `__dirname` before the fix, so this is a real forward-compat test, not just a silenced warning).
- `VITE_CONFIG_NATIVE_IGNORE_WARNING=true` only hides the message (it also disables the AST checker). Explicitly setting `configLoader: 'bundle'` does **not** suppress it — the warning is emitted *by* the bundle loader itself.
- `tsc -b` covers `vite.config.ts` only (`tsconfig.node.json` `include`); `import.meta.dirname` type-checks via `@types/node`'s `web-globals/importmeta.d.ts`, but `vitest.config.ts` / `playwright.config.ts` are never type-checked.

## Storybook setup (2026-11-14)
- Tag: `frontend`
- Canonical source for standing rules: `frontend/CONVENTIONS.md` `## Storybook`; this lesson is the dated narrative.
- Scaffolded via official `npx storybook@latest init`; audited installer edits to vitest.config.ts (converted to projects), eslint.config.js (storybook recommended), package.json.
- Preview uses CSF 3 shape (not definePreview); MSW v3 via `mswLoader` + `worker.start({ onUnhandledFrame })`; `defaults.error()` responds 500 — record unhandled `/api` requests in a `Set` and assert it empty (a 500 alone can be swallowed by error UI).
- `npm test` scoped to `--project=unit`; storybook project requires Playwright Chromium.
- Stories intentionally remain in tsconfig.app.json include for type coverage.
- React Compiler contingency not needed (build-storybook passed with `@rolldown/plugin-babel` merged).
- Installed vite version: 8.3.8 (>=8.1.4).

## Storybook setup — review fixes (2026-11-14)
- Tag: `frontend`
- Canonical source for standing rules remains `frontend/CONVENTIONS.md` `## Storybook`; this entry is the dated narrative of the review-pass fixes.
- **`parameters.msw` is only a migration path in msw-storybook-addon v3 CSF 3.** The `mswLoader` (`build/csf3.mjs`) calls `worker.resetHandlers()` per story, and the `msw` object handed to `beforeEach({ msw })` **is the `SetupWorker`** (`MswApi = SetupWorker`). Sanctioned wiring: empty `setupWorker()` in the loader, fixtures per-story via `beforeEach({ msw }) { msw.use(http.get('/api/v1/tags', ...)) }`. A module-level `setupWorker(...handlers)` in preview.ts works (initial handlers survive `resetHandlers`) but is broader than any single story needs.
- **Radix Dialog/Sheet portals content to `document.body`** — queries with `canvas` cannot see it. In browser-mode `play` functions use `screen` from `storybook/test` (file-scoped `canvas` is only for the story's own root).
- **Storybook required `args` even with a `render` override** when component props are all required (`satisfies Meta<...>` enforces it).
- **`npx vitest list` without `--project` lists browser-mode story tests too** — a story-count AC must use `--project=unit` (0), not the unfiltered list (108).
- **A referenced project without `tsBuildInfoFile` drops its cache next to the tsconfig** (`.storybook/tsconfig.tsbuildinfo`) instead of `node_modules/.tmp/` — and it wasn't gitignored. Redirect all three projects' `tsBuildInfoFile` to `node_modules/.tmp/` and gitignore `*.tsbuildinfo`.
- `vitest.shims.d.ts` scaffolded by init was dead (no tsconfig referenced it; addon-vitest ≥ 10.3 auto-provisions `setProjectAnnotations`) — deleted.
- a11y `'todo'` precedent: tag-picker `COLOR_PALETTE` buttons (axe `button-name`, 6 violations) — story sets `test: 'todo'` with a comment + Notes entry; app-code fix deferred (T028 tooling-only; no production code changes allowed).
- `mockResolvedValue(undefined)` on a `fn<typeof contextDefault['login']>()` spy + `toHaveBeenCalledWith` is the red-first pattern for context-mocked submit flows; `storybook/test`'s `fn` is `@vitest/spy`'s Mock (supports `mockResolvedValue`, custom matcher `toHaveBeenCalledOnce`).

## Storybook setup — independent review round 2 (2026-11-14)
- Tag: `frontend`
- Canonical source for standing rules remains `frontend/CONVENTIONS.md` `## Storybook`; this entry is the dated narrative of the second review pass (spec: `specs/tasks/T028-frontend-react-storybook-setup.md` §Review round 2, R15–R25).
- **A `tsconfig.json` `include` with a bare directory or `"."` can be vacuous** — `tsc --noEmit -p <proj>` exits 0 while checking zero files. Verify a tsconfig actually has inputs (`tsc --listFilesOnly`) before trusting its green result. Ours silently never checked `.storybook/preview.ts`; the first real check surfaced two latent errors.
- **Storybook 10 has no `showName` toolbar option.** It exists in no installed type and no manager bundle read it — dead config cargo-culted from older docs; removed. Current `ToolbarConfig`: `title?`, `icon?`, `items`, `shortcuts?`, `dynamicTitle?`, `preventDynamicIcon?`.
- **React 19 `createElement` requires `children` inside the props object** when the component's props mark it required — passing children as positional rest args does not satisfy the `Attributes & Props` check. Make `children` optional in props, or pass them in the attributes object.
- **An init-generated config is not a reviewed config.** Init collapsed `eslint.config.js` to a single line, added an unused `@vitest/coverage-v8`, and the MSW worker arrived fine but `.storybook/tsconfig.json`'s include was authored by hand and wrong. Audit every installer-touched file, not just the semantic ones.
- **`{Story()}` in a decorator bypasses React component identity** — hooks attach to the decorator's render pass. Always render `<Story />`. (R4 fixed auth stories; the same pattern survived in two more files — grep for `{Story()}` when an audit asks for one spot.)
- **Dead dep check after scaffold:** `npm uninstall -D <pkg>` after confirming nothing references it (no scripts, no config); verify with grep, not intuition.
- **Untick ACs that claim CI-green until the run exists.** A checked AC with "(until then: syntax-valid)" overstates state; CI-red proving still requires a push (or a local rehearsal of the exact gate sequence, clearly labelled as local).
- **`vite`/`vitest.config.ts`: use `import.meta.dirname`**, not a `typeof __dirname` fallback — the fallback trips Vite's native-loader warning on every run.
- Reformatting an installer-collapsed config is safe when `npm run lint` stays 0/0 — keep rule sets and ignores byte-identical, change only layout.
