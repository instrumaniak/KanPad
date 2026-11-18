# T018: Secrets hygiene + dependency declarations

> **STATUS: DONE 2026-11-18 — locally verified; one item is user action.**
> Closes the secret half of **C3** **locally** (remote host copy of `release/.env`
> still trusts the burned secret — user action pending; see §Implementation Record)
> and the package half of **C2** (complete).
>
> **Amended 2026-11-18** (review pass; findings in §Review Findings, verified on disk):
> scope enlarged to all on-disk env files (root `.env`, `backend-rs/.env.test` were
> missed), factual errors fixed (modes were 664 not 644; CSS `@import`s exist),
> SESSION_SECRET rotation upgraded from open call to **decided: rotate now**,
> phantom-dependency list completed, unsatisfiable `--omit=dev` proof replaced.
>
> **Implemented 2026-11-18.** §Implementation Record (below) supersedes the step list
> where they diverge; divergences are explicit. **One review finding was WRONG and is
> corrected in §Review Findings:** `checkbox.tsx` is not dead code (production importer
> `src/features/checklists/checklist-item.tsx` + a story) — deleted then restored via
> `git restore`, and `@radix-ui/react-checkbox@^1.4.1` declared instead.

## Review Findings (2026-11-18, verified on disk — read-only)

Recorded so the implementer doesn't re-litigate; re-verify cheaply before acting.

- **File modes were 664** (`-rw-rw-r--`), not 644, on all on-disk env files. All are
  gitignored; only `.env.example` files are tracked.
- **Five** on-disk env files, not three: `backend-rs/.env`, `release/.env`,
  `frontend/.env`, **`.env` (repo root — read by `compose.yaml` for
  `SESSION_SECRET`/`DB_PASSWORD`/`CORS_ORIGINS` interpolation)**, and
  **`backend-rs/.env.test`** (test DB creds).
- **Shared `SESSION_SECRET` confirmed**: `backend-rs/.env` and `release/.env` carry the
  **identical** value (locally hash-compared, 2026-11-18). Root `.env` has a different
  one; `frontend/.env` has none.
- **User decision 2026-11-18** (asked & answered during review): `release/` has been
  **uploaded to a remote host before** — its `.env` must be treated as exposed.
  Decision: **rotate the shared secret everywhere now.** Announce re-login.
- `release/.env`'s `DB_PASSWORD` is **unique** (differs from both other envs) and its
  `DB_HOST` points at the **local DB** on this machine → low practical risk; record in
  Notes, optional rotation. `release/` contents checked: `app-proxy*.js` Passenger
  proxies contain no secrets (keyword hits were `process.env` / Passenger
  lifecycle comments); built SPA carries no server secrets.
- **Undeclared direct imports** (all hoisted in the lock today, none declared):
  - Runtime: `@radix-ui/react-tabs` + `@radix-ui/react-tooltip` (1.2.1 / 1.4.1 via the
    `radix-ui@1.8.1` umbrella), `@radix-ui/react-checkbox` (1.4.1, via same umbrella —
    **in production use**), `micromark-util-chunked`, `micromark-util-classify-character`,
    `micromark-util-resolve-all` (`src/features/notes/remark-underline.ts`).
  - Type-only (declared as devDependencies): `mdast` import resolves via
    **`@types/mdast`** (automatic @types lookup — no `mdast` package), `unified`,
    `mdast-util-from-markdown`, `micromark-util-types`.
- **Build-time consumers exist in `src/`** (the old "never imported by `src/`" claim
  was false): `src/index.css:1-3` has `@import "tw-animate-css"; "shadcn/tailwind.css";
  "tailwindcss"`; `vite.config.ts:3` imports `@tailwindcss/vite`. `devDependencies`
  classification is still correct — these are consumed at build time and `dist/` is
  static (no `node_modules` at runtime).
- ~~`checkbox.tsx` is dead code~~ — **WRONG (corrected 2026-11-18)**: grep filter
  `-v components/ui/checkbox` deleted the evidence lines (the import path contains the
  pattern). Production importer: `src/features/checklists/checklist-item.tsx`.
  Story: `src/components/ui/checkbox.stories.tsx`. Fix: `git restore` + declare
  `@radix-ui/react-checkbox@^1.4.1`.
- `pele@1.13.0` (exact pin) present; `mermaid` absent (T016, done 2026-11-18) — intact
  after the dep reshuffle (verified post-`npm ci`).
- `docs/deployment.md` was the right host for the checklist (now present, incl.
  rotation runbook); `docs/self-hosting.md` needs no change.
- Spec `40-operations.md` §5 listed only the three envs — **synced 2026-11-18**.

## Goal

1. **Secret file permissions:** `chmod 600` **all five** on-disk env files; deploy-time
   check documented in `docs/deployment.md` (`stat -c %a` = 600). ✅ done
2. **SESSION_SECRET — rotate now, everywhere** (user decision 2026-11-18). ✅ done &
   verified (distinct per-instance secrets; old cookie 401; remote host copy = **user
   action**, see §Notes).
3. **Undeclared imports:** declare the complete list. ✅ done — checkbox declared, not
   deleted (see Review Findings correction).
4. **Build-only packages → `devDependencies`** (`shadcn`, `tailwindcss`,
   `@tailwindcss/vite`, `tw-animate-css`). ✅ done.

## Depends On

**T016** (done 2026-11-18) — both tasks edit `frontend/package.json` (T016 wired the
budget script; T018 adds/moves dependencies). Serialized so two agents never edit the
file at once.

## Phase

0

## Critical

**Yes — upgraded 2026-11-18.** Confirmed upload of `release/.env`; rotation executed
2026-11-18. Residual risk lives in the remote host copy (user action pending).

## Spec References

- `specs/arch-hardening-specs/40-operations.md` §5 — **synced 2026-11-18** (five envs,
  rotation executed, status).
- `specs/arch-hardening-specs/00-overview.md` — findings C2, C3; global DoD.
- `specs/security/06-infrastructure-deployment.md:34-35` (`.env` permissions 600).
- `docs/deployment.md` — new § "Deploy-time secret checklist (T018)" + rotation
  runbook.
- `compose.yaml` — root `.env` interpolation (documented in the checklist).

## Files Actually Created/Modified (2026-11-18)

| File | Change |
|---|---|
| On-disk `.env` files (all five) | `chmod 664 → 600` (not a git diff) |
| `frontend/package.json` | + `@radix-ui/react-tabs` ^1.2.1, `@radix-ui/react-tooltip` ^1.4.1, `@radix-ui/react-checkbox` ^1.4.1, `micromark-util-chunked`/`-classify-character`/`-resolve-all` ^2.0.1 (deps); + `@types/mdast` ^4.0.4, `mdast-util-from-markdown` ^2.0.3, `micromark-util-types` ^2.0.2, `unified` ^11.0.5 (devDeps, type-only); moved `shadcn`/`tailwindcss`/`@tailwindcss/vite`/`tw-animate-css` → devDeps |
| `frontend/package-lock.json` | Regenerated; `npm ci` clean-room verified; diff reviewed (pele 1.13.0 intact, mermaid absent; churn = dev-flag re-marking) |
| `frontend/src/features/boards/archived-boards.test.tsx` | Pre-existing failure fixed at the **fixture** (`is_archived: true`) — component was correct per its documented contract; 840/840 green |
| `docs/deployment.md` | + § "Deploy-time secret checklist (T018)" (all five envs + remote-copy rule) + rotation runbook |
| `specs/arch-hardening-specs/40-operations.md` §5 | Synced: five envs, rotation executed, distinct-secret rule, standing packaging-guard rule |
| `docs/lessons/frontend.md` + `docs/lessons/ci-quality.md` | Dated, tagged lessons appended |
| `specs/tasks/T018-secrets-hygiene-dependency-declarations.md` | This file (amended + implemented) |

**Must NOT / did NOT appear in the diff:** any secret value (git-diff scanned — 32+-hex
strings and `SESSION_SECRET=`/`DB_PASSWORD=` assignments: none outside `.env*`, which is
gitignored), `.env` contents, runtime source changes (the only frontend source touch is
a **test fixture**; `checkbox.tsx` was deleted then `git restore`d — net zero).

## Implementation Record (2026-11-18)

**Secrets / rotation:**
- `git check-ignore` re-asserted for all five env files (all ignored).
- `stat -c %a`: 664 → 600 for all five (root `.env`, `backend-rs/.env`,
  `backend-rs/.env.test`, `frontend/.env`, `release/.env`).
- Pre-rotation baseline: local backend (dev, `:3001`) logged in — throwaway user
  `t018-verify-<ts>@example.invalid` registered (201), login 200, protected
  `GET /api/v1/auth/me` → **200** (cookie saved, then)…
- Rotation: **distinct** fresh secrets (`openssl rand -hex 32` ×2), written write-only
  (no printing/logging), `chmod 600` re-applied after `sed -i` (it recreates files with
  umask mode — re-chmod is mandatory, now a lesson).
  Pre-rotation line hashes: both `ff14b1c89769`. Post: `d7510c14836e` (backend-rs) /
  `574f23a8c6a5` (release) — distinct, both ≠ pre. (Prefixes only; values never printed.)
- Restart: old-secret instance stopped; fresh backend started. **Pre-rotation cookie →
  401; fresh login → 200; fresh cookie → 200;** API `/ready` → 200.
- Throwaway user deleted from dev DB afterward (scoped `DELETE … WHERE id=6 AND
  email='…'` — matched exactly 1 row). Dev backend left **stopped** (it was not running
  at task start).
- Root `.env`: `chmod` only, value **untouched** (different secret, not implied
  exposed). `backend-rs/.env.test`: chmod only.
- Announcement: only user is raziur (self) — no channel needed; recorded.
- **Remote host copy of `release/.env`: USER ACTION PENDING** — update env + restart
  (or decommission) its release-testing instance; rotation commands are in
  `docs/deployment.md` § "Deploy-time secret checklist (T018)". Until done, the old
  secret still signs cookies for the remote instance.

**Dependencies:**
- `checkbox.tsx` deletion → build broke (`tsc -b`: used by `checklists/checklist-item.tsx`
  + story) → `git restore` → declared `@radix-ui/react-checkbox@^1.4.1`.
- Declared all phantom direct imports (see Files table). `@types/mdast` covers the bare
  `import type … from 'mdast'` (no `mdast` package exists).
- Moved the four build-only packages to `devDependencies`.
- `npm ci` (clean-room) ✅; `npm run build` ✅ (8.3 s); `vitest --project=unit`
  **840/840 ✅** (after the fixture fix); `tsc -b` ✅ (via build);
  `gen:api -- --check` ✅ (up to date); `npm run budget` ✅ (initial gzip **258,443 B**
  / cap 310,000); `eslint .` ✅ (silent); `dist/` standalone smoke ✅
  (`vite preview :4173`: `/` → 200, entry JS → 200, `node_modules` not required, API
  `/ready` → 200).
- `npx depcheck`: **no Missing dependencies**. "Unused devDependencies" reported:
  `shadcn`, `tailwindcss`, `tw-animate-css` (CSS consumers — expected false positives;
  stay), `@chromatic-com/storybook`, `@storybook/addon-a11y`, `@storybook/addon-docs`
  (Storybook config-driven addons — **not purged** per the "leave in place, list for
  future task" rule). `@tailwindcss/vite` not flagged (seen in `vite.config.ts`).
- `package-lock.json`: +~346 lines — mostly `"dev": true` re-marking of the now
  dev-flagged subtrees (shadcn/tailwind family) + root manifest section; no unexpected
  transitive version churn; `pele@1.13.0` exact pin intact; `mermaid` absent.
- `npm install` reports **33 vulnerabilities (11 moderate / 22 high)** — **not
  introduced by T018** (all added pins == the hoisted transitive versions already in
  the tree). Out of scope; flagged for a future deps/audit task.

**Notes on mistakes made & fixed:**
- The amended task's "checkbox.tsx is dead code — verified" was wrong; see Review
  Findings. A grep filter deleted its own evidence. Lesson appended (frontend.md).
- `sed -i` on an env file re-created it at 664 — caught via `stat` in the same step;
  re-chmod 600; lesson appended (ci-quality.md).
- T018 recorded one pre-existing test failure unrelated to deps (archived-boards
  fixture) and fixed it at the fixture so the AC "vitest green" could hold.

## Constraints

(all still in force — see git history for the amended list)

## Acceptance Criteria

- [x] All five env files mode 600 — `stat -c %a` pasted above; `docs/deployment.md`
      checklist covers all five (secret half of **C3** closed **locally**; the remote
      host copy of `release/.env` remains open as user action — see AC below).
- [~] **SESSION_SECRET rotated and verified locally** — distinct fresh secrets; old
      cookie 401; fresh login 200; root `.env` mode 600, value unchanged; re-login
      announced (self). **Remote host copy: USER ACTION PENDING** (update + restart or
      decommission) — not verifiable from this machine; until closed, the remote
      instance still trusts the burned secret.
- [x] All undeclared direct imports declared (or removed — none; checkbox declared);
      `npm ci` clean; build + unit vitest + `tsc -b` + `gen:api -- --check` +
      `npm run budget` green (closes undeclared half of **C2**).
- [x] Build-only packages in `devDependencies`; build + vitest green; `dist/`
      standalone-serve smoke passes; `package-lock.json` diff reviewed — `pele@1.13.0`
      intact, `mermaid` absent (rest of **C2**).
- [x] `40-operations.md` §5 synced (five envs + distinct-secret + status);
      `docs/deployment.md` checklist + rotation runbook added; lessons appended
      (dated, tagged).
- [x] No secret values in the diff; evidence hygiene confirmed (32+-hex scan clean).

## Notes

**Settled 2026-11-18:** release/ uploaded to remote host before; DB creds local &
unique; shared SESSION_SECRET rotated everywhere (this machine) with distinct values;
root `.env` value untouched.

**Settled 2026-11-18 (Open calls):**
1. *Distinct vs identical fresh secrets* — **distinct, used.**
2. *Remote host copy* — **user will update it themselves** (update env + restart or
   decommission); commands in `docs/deployment.md`.
3. *`checkbox.tsx`* — kept + declared (NOT dead code; the earlier finding was falsified
   by `tsc -b`).
4. *Packaging guard* — out of scope; recommended Wave-1 follow-up; recorded in
   `40-operations.md` §5 as a standing rule.
5. *release/.env `DB_PASSWORD`* — unique, local DB → not rotated (optional hygiene).

**Announcement:** raziur only — noted, no external users.

**Dev DB hygiene:** throwaway verification account (register→login→401 test) deleted
after use; dev DB otherwise untouched. `backend-rs` left stopped (not running at start).

**Follow-ups (not T018):**
- Packaging guard excluding `.env*` from release artifacts (Wave 1).
- `npm audit` 33 vulns — deps/audit task; not introduced by T018.
- `status.yaml` still says `in_progress` and `critical: false` (your earlier edit) —
  flip to `done` when satisfied, and decide the `critical` flag to match the
  upgraded task header (or leave it false if it tracks Wave-0 urgency separately);
  the task file header records the upgrade.
- No commit made — awaiting your go-ahead.

## Lessons

Done — `docs/lessons/frontend.md` (grep-evidence trap; CSS-consumer classification;
explicit phantom-dep declarations) and `docs/lessons/ci-quality.md` (uploaded-tree
`.env` exposure; sed re-chmod; five-files-not-three; fixture-side test fix).
