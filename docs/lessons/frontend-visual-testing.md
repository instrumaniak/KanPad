# Frontend visual testing & verification with Playwright (2026-11-18, T044)

- Tag: `frontend,e2e,playwright,visual`
- Born from the T044 reproduction: a 148-card seeded board measured at 1600×900 → a column rendered **5996 px** tall while the document stayed 900 px, `<main>` silently became the only scroller, and the designed per-column `overflow-y-auto` never engaged. Screenshots showed the bug; **only the measurements located the actual defect** (a broken height chain) rather than the reported symptom ("columns grow"). Use this playbook for any "the layout does X" frontend report before proposing changes.

## Playbook

1. **Reproduce on a throwaway stack, never the dev DB.** Run a dedicated API against `kanpad_e2e` (migrations via the backend binary, not sqlx-cli) + Vite dev on :5173. Start the built binary directly (`$CARGO_TARGET_DIR/debug/backend-rs serve`) — fastest, no recompile. Check ports 3001/5173 are free first (`ss -ltn`). Seed data **via the HTTP API with a Playwright `request` context** (register → login → POST resources), never via the UI: it is fast, deterministic, and exercises the same wire contract. Use a `Date.now()` stamp in names for idempotent re-runs; accept `409` on re-register.
2. **Keep scratch artifacts in /tmp, not the repo.** Write standalone `.mjs` scripts (e.g. `/tmp/opencode/repro-*.mjs`) that `createRequire('<repo>/frontend/package.json')` to load `@playwright/test` — no repo files added or modified, no config needed, safe in plan mode. Output screenshots + a `measurements.json` to /tmp too.
3. **Seed for the *shape*, not just the count.** Mix title lengths (wrapping text varies card heights), attach a couple of labels via the label API to mimic real density, and distribute cards across columns unevenly (e.g. 70/45/15/12/6) — visual bugs hide in the skew. Unit tests with 2-card fixtures cannot see containment/overflow defects.
4. **Measure geometry with `page.evaluate`; don't eyeball it.** Collect per element: `getBoundingClientRect().height/top`, `scrollHeight`, `clientHeight`, `scrollTop`. Compare (a) `document.documentElement.scrollHeight` vs `innerHeight`, (b) each intended scroll container's `clientHeight` vs `scrollHeight` (equal ⇒ the scroll never engages — this pinpointed the containment bug), (c) `main`'s `scrollHeight/clientHeight/scrollTop` (the *actual* scroller is often not the window). Save the JSON next to the screenshots.
5. **FullPage screenshots lie about inner-div scrollers.** `page.screenshot({ fullPage: true })` stitches the *document*, not any `overflow-y-auto` descendant — if the document doesn't grow, it clips exactly like the viewport. Take the matrix: viewport / fullPage / `window.scrollTo(bottom)` / the real scroller (`document.querySelector('main').scrollTop = 1e6`) / mobile viewport (420×880). The scroller-scrolled shot is what proves content is reachable or unreachable. Bonus proof: `md5sum` the viewport and fullPage outputs — identical hashes mean the document had no extra content beyond the viewport (it did).
6. **Pair every screenshot with the code path it exposes.** Trace the captured element's ancestors for missing `flex`/`min-h-0`/`h-full` — in this case **two** links broke, and both must go in the fix: the `.transition-opacity` wrapper (`board-view.tsx:264` — no `flex`/`min-h-0`/`flex-1`, so `h-full` collapsed to content height) *and* `column-droppable.tsx` (a plain `<div>` with no `className` between the column flex-col and the scroller — a block child of content height making the scroller's `flex-1` a no-op). `<main>` absorbed everything (`app-layout.tsx:166`). The symptom ("page grows forever") looked like a pagination problem; the measurements + chain walk proved it was layout.
7. **Drive the browser like a user minimally, but measure like a probe.** Login through the UI (catches auth/cookie/CORS breakage), then `evaluate()` for numbers; assertions belong in the task's E2E specs afterwards.
8. **Env traps on this stack — what turned out to be true here (2026-11-18):**
   - The API reads `CORS_ORIGINS` from the **process CWD's `.env`** (dotenvy; `backend-rs/src/config.rs`) — not from a hardwired location. The prod origin lives in the **repo-root** `.env` (`CORS_ORIGINS=https://kanpad.raziur.com`); `backend-rs/.env` does **not** define it (keys there: APP_ENV, CARGO_*, DB_*, SESSION_SECRET only). The built-in default (`DEFAULT_CORS_ORIGIN`, `config.rs:52`) is `http://localhost:5173` — which is why the stock Playwright suite works with no override. Any `backend-rs serve` launched from the repo root inherits the prod origin and the browser path CORS-blocks with a healthy `/health`. Serve with `CORS_ORIGINS=http://localhost:5173` and verify the preflight matches what you set:
     `curl -i -X OPTIONS <api>/api/auth/login -H 'Origin: http://localhost:5173' -H 'Access-Control-Request-Method: POST' -H 'Access-Control-Request-Headers: content-type'` must echo `access-control-allow-origin`. Confirm via the boot-log line `cors_origins=[...]` — check the served instance's actual list, not your intention.
   - Vite must start with absolute `VITE_API_URL` (e.g. `http://localhost:3001`) or the two-origin path silently degrades to same-origin behind the dev proxy.
   - `APP_ENV=test` keeps session cookies non-`Secure` so login works over `http://localhost` (the cookie layer mounts with `.with_secure(is_production())`).
   - `page.waitForURL('/')` **does** resolve on same-document SPA navigations (pushState navigations are navigations): it waits for the matching URL; when the target URL already matches it only waits for `waitUntil` (default `'load'`), which a long-lived document has already fired — so it resolves promptly. The timeout observed here meant the navigation **never happened** (login was CORS-blocked). Diagnose with a console/pageerror/response listener (see `debug-login.mjs`) before blaming the wait; `waitUntil: 'commit'` is the documented way to not wait for `load`; `page.waitForFunction(() => window.location.pathname === '/')` is a valid alternative.
   - The one-command local path is `frontend/scripts/e2e-local.sh` (`npm run test:e2e`): derives `E2E_DATABASE_URL` from `backend-rs/.env`, creates `kanpad_e2e` (utf8mb4/utf8mb4_unicode_ci), migrates via the backend binary, runs Playwright. Use it for the one-shot spec suite; use manually held background servers only for the interactive screenshot/measurement loop.
   - `waitForTimeout()` is off the table for `e2e/` specs (AGENTS.md). The scratch `.mjs` measurement scripts may use it *only* to let paint settle after an explicit programmatic scroll — nothing else.
   - Teardown: `pkill -f 'backend-rs serve'` is safe; `pkill -f vite` **is not** — the invoking shell's own command line contains "vite" and it SIGTERMs itself (exit 143). Use `pgrep -af "node.*vite"` and kill by PID.
9. **Record the baseline before fixing.** Numbers + screenshots before the change are the acceptance criteria's "before" half (e.g. T044: 5996 px column, `windowScrolls:false`, `main` scrollHeight ≈ 6162 / clientHeight 844 — console-reported, not archived; archive future runs to a file). After the fix, re-run the same script untouched and diff the JSON.
10. **Clean up or document what you leave running.** Background servers (`backend-rs serve`, `vite dev`) and the seeded `kanpad_e2e` schema should be listed in the task notes (PIDs, ports, board id, test user) so the next agent can either reuse or nuke them (`pkill -f 'backend-rs serve'; DROP DATABASE kanpad_e2e`).

## Minimal repro skeleton

```js
import { createRequire } from 'node:module';
const require = createRequire('/abs/path/to/<repo>/frontend/package.json');
const { chromium, request } = require('@playwright/test');
// 1. seed: API context, register → login → create board → POST N cards (vary length, attach labels)
// 2. drive: browser login via UI → <SPA>/board/<id> → waitForLoadState('networkidle')
// 3. measure: page.evaluate(() => ({ doc: document.documentElement.scrollHeight,
//      inner: window.innerHeight, mainScroll: document.querySelector('main')?.scrollHeight,
//      cols: [...document.querySelectorAll('[data-column-id]')].map(el => ({
//        h: el.getBoundingClientRect().height,
//        scrol: el.querySelector('.overflow-y-auto')?.scrollHeight,
//        cli:   el.querySelector('.overflow-y-auto')?.clientHeight })) }))
// 4. shoot: viewport | fullPage | scrolled real scroller | mobile 420×880
```

## Related

- T044 (board column height containment + visual affordances) — first consumer of this playbook.
- Repo E2E rules (`AGENTS.md` §Frontend E2E Test Patterns): setup via API not UI, no `waitForTimeout()`, `waitForLoadState('networkidle')` — this playbook complements them for *visual/layout* verification, which the unit-jsdom layer cannot see.
- Cross-file candidates: if a new theme of visual regressions appears (e.g. dnd flicker), extend this file rather than opening a new one, unless it becomes a distinct discipline.
