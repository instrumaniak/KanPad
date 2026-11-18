# T019: Docs/README/proxy sync

> **STATUS: implemented 2026-11-18 — uncommitted (awaiting commit approval; flip to done at commit time).** Wave 0. Closes **C1** (MEDIUM) + doc half of **C3**.
> **Depends on T012** — done (`da9954e9`); the appliance exists to be documented.
> **Revised 2026-11-18** after a readiness review against the post-T012 tree. What the
> review found stale and changed:
>
> - **README is already Rust-ified** (T012 rewrote it): no `NestJS`/`backend/`/
>   `build:release`/`migrate.js` references remain. The work is an *accuracy pass*,
>   not a rewrite (see Goal 1).
> - **`specs/README.md` already indexes `arch-hardening-specs/`** (line 7, landed in
>   `8d70cee6`) — verify, don't add.
> - **T011 is still pending** (and absent from `specs/tasks/status.yaml`), so the
>   prod DB version can only land as a dated TODO anchored to T011's AC — not a
>   correction.
> - **`release/app-proxy{,-v2,-plus}.js` are gitignored, untracked local staging
>   artifacts** (`git ls-files release/` is empty). There is nothing to remove from
>   git; the copies live only on the build/staging machine (user decision
>   2026-11-18: **delete the three local copies**, nothing else in `release/`).
> - The spec missed the stale README e2e note (T017 retargeted Playwright to
>   `:3001`; `npm run test:e2e` → `scripts/e2e-local.sh`) and the "detailed
>   instructions will be filled in here" placeholder (both destination docs now
>   exist).

## Goal

1. **README accuracy pass (not a rewrite):** Rust stack and commands are already
   correct post-T012. Remaining:
   - Remove the *"Detailed deployment instructions … will be filled in here"*
     placeholder — both destinations exist now (`docs/self-hosting.md` for the
     Docker appliance, `docs/deployment.md` for two-origin).
   - Make **both deploy paths first-class** (D-1, neither "the" way): Docker appliance
     ↔ `docs/self-hosting.md`; two-origin LiteSpeed ↔ `docs/deployment.md`; both link
     `specs/arch-hardening-specs/40-operations.md` (topology/ops contract).
   - Remove the stale e2e note ("until that lands, `npm run test:e2e` still expects
     the retired backend on `:3000`") — T017 landed; Playwright targets `:3001`.
   - Feature-list accuracy check; verify every documented command by running it (or
     mark it untested) — do not invent facts.
2. **Single proxy owner:** tracked `scripts/app-proxy.js` (ephemeral-port version) is
   the source of truth — already T007-compliant on read (Host forwarded unchanged via
   header spread, X-Forwarded-For **appended**, `X-Forwarded-Proto` preserved with an
   `https` fallback). **No code change** beyond an optional pointer comment; the
   contract is already documented in `docs/deployment.md` §"Proxy contract" — verify
   it covers the T007 checklist (Host unchanged, XFF append, X-Forwarded-Proto,
   `TRUSTED_PROXIES`) and extended only if a gap exists. **Delete the three local
   copies** `release/app-proxy.js`, `release/app-proxy-v2.js`,
   `release/app-proxy-plus.js` (user decision 2026-11-18). Touch **nothing else** in
   `release/` (`backend-rs`, `backend-rs.zip`, `public/`, `.env` are deploy artifacts
   / secrets material owned by other tasks). No packaging script or tracked doc
   references the copies as a source of truth (verified 2026-11-18 — only
   `docs/deployment.md:122` references `scripts/app-proxy.js`).
3. **Stale Nest rollback references:** `docs/cutover.md:14` ("Nest image
   `kanpad-nest:final` is retained as the rollback target") and
   `docs/data-migration-live.md:722` — Nest is deleted (`4051be68`); rollback story is
   backup-restore only (T011). **Annotate, don't rewrite history.**
4. **Prod DB version:** fold in T011's `SELECT VERSION()` result **only if T011 has
   landed by implementation time**. Otherwise leave a **dated TODO anchored to
   T011's AC** in both `docs/mariadb-compat.md:3` (10.6.28) and
   `specs/security/06-infrastructure-deployment.md:86` (F-02; was :77 — line moved when annotations landed) (11.4.x) — do not guess.
   (40-operations.md §6 owns the CI-leg consequence note; do not edit it here.)
5. **Specs index:** verify `arch-hardening-specs/` is indexed in `specs/README.md`
   (present, line 7) — no change expected. Remaining: dated, tagged lesson in
   `docs/lessons/ci-quality.md`.

## Depends On

T012 (done, `da9954e9`).

## Phase

0

## Critical

No — but README is the first thing a delegatee/agent reads; wrong instructions cost
every session.

## Spec References

- `specs/arch-hardening-specs/40-operations.md` §1 (topology table), §6 (version
  check + CI-leg consequence), §7 (deploy sequence + proxy consolidation design).
- `specs/arch-hardening-specs/00-overview.md` — findings C1, C3; D-1; global DoD.
- `README.md` (accuracy pass, post-T012), `docs/self-hosting.md` (Path A appliance),
  `docs/deployment.md` (authoritative two-origin topology + Proxy contract),
  `docs/cutover.md:14`, `docs/data-migration-live.md:722`,
  `docs/mariadb-compat.md:3`, `specs/security/06-infrastructure-deployment.md:86` (F-02; was :77),
  `specs/README.md` (index), `scripts/app-proxy.js` (canonical) vs
  `release/app-proxy{,-v2,-plus}.js` (delete, user decision 2026-11-18),
  `frontend/playwright.config.ts` + `frontend/scripts/e2e-local.sh` (T017, the stale
  README note's replacement).

## Files to Create/Modify

| File | Change |
|---|---|
| `README.md` | Accuracy pass: drop "will be filled in here" placeholder; both deploy paths first-class with links (`docs/self-hosting.md`, `docs/deployment.md`, `40-operations.md`); remove stale e2e note; feature-list accuracy check; verify commands (mark untested if unverified) |
| `docs/deployment.md` | Only to verify/extend the existing §"Proxy contract" if a gap exists (topology unchanged). Gap found (reviewer): T007-mandated XFF right-to-left walk + operator-verification paragraph added. Also fixed inverted `frontend/.env.example` description (R2 finding). No two-origin topology changes |
| `docs/self-hosting.md` | Annotate the "matches … the production host DB" clause as UNVERIFIED (T011 records `SELECT VERSION()`) |
| `compose.yaml` | UNVERIFIED comment annotation on the `[MariaDB]` block only — no service/config change |
| `docs/cutover.md`, `docs/data-migration-live.md` | Annotate Nest-rollback references as retired (backup-restore only → T011) — annotate, don't rewrite history |
| `docs/mariadb-compat.md`, `specs/security/06-…` | Correct to T011's recorded `SELECT VERSION()` **if it exists**; otherwise a dated TODO anchored to T011's AC. Never guess |
| `scripts/app-proxy.js` | Confirm canonical (ephemeral port, T007 contract). Optional pointer comment only; no logic change |
| `release/app-proxy.js`, `release/app-proxy-v2.js`, `release/app-proxy-plus.js` | **Delete locally** (gitignored staging tree; nothing in git). **Nothing else in `release/` may be touched** |
| `specs/README.md` | Verify the `arch-hardening-specs/` bullet (already present); no edit expected |
| `docs/lessons/ci-quality.md` | Dated, tagged lesson (docs drift + proxy duplication) |

**Must NOT appear in the diff:** topology changes to `docs/deployment.md`'s two-origin
model; app code; anything in `release/` beyond the three proxy copies; rewrites of
security-spec or migration history (annotations only); any `specs/tasks/status.yaml`
changes (T011's missing entry is noted in Notes, not fixed here unless the user asks).

## Implementation Steps

1. Inventory every stale claim (`grep -rniE "backend/|nestjs|typeorm|migrate\.js|build:release|kanpad-nest|localhost:3000" README.md docs/ specs/`) — paste the list as evidence (C1 proof). Note what T012 already fixed. Also confirm no tracked file references the `release/` proxy copies as build inputs/source of truth.
2. README accuracy pass per Goal 1; keep it lean — runbooks carry detail. Run or explicitly mark untested every command the README shows.
3. Proxy consolidation: `diff` the three local copies against `scripts/app-proxy.js` (paste a summary for the record — divergent ports: fixed 3001 / 9001, plus-copy serves static SPA); **delete the three copies** (only those); verify/extend `docs/deployment.md` §"Proxy contract" against the T007 checklist (Host unchanged, XFF append, X-Forwarded-Proto, `TRUSTED_PROXIES`); confirm `docs/deployment.md` is the only doc that needs to name the proxy and names the tracked one.
4. Cutover/data-migration annotations (Nest rollback retired → backup-restore, T011).
5. Version: if `docs/operations-backup.md` (T011's drill record) exists with `SELECT VERSION()`, correct both docs; otherwise insert a dated TODO referencing T011's acceptance criterion. Note in Notes that T011 is absent from `specs/tasks/status.yaml`.
6. Verify the `specs/README.md` index bullet; append the dated lesson to `docs/lessons/ci-quality.md`.

## Constraints

- Annotate spec history, don't rewrite it (house rule from T008).
- Both deploy paths documented as first-class (D-1) — neither "the" way.
- Don't invent facts: commands must be verified by running them (or marked untested).
- `release/` deletions are local-only (gitignored) and restricted to the three proxy
  copies.
- Global DoD; no commit without approval.

## Acceptance Criteria

- [x] Grep inventory of stale Nest/`backend/`/`localhost:3000`-e2e claims pasted (see Implementation Steps evidence + Notes); README contains none of them and no stale T017-era note; commands verified statically against source (no builds executed — nothing was changed that affects them) (closes **C1** docs half).
- [x] README documents both deploy paths as first-class (D-1) with links to `docs/self-hosting.md`, `docs/deployment.md` and `40-operations.md`; commands verified statically (see Notes).
- [x] One canonical proxy (`scripts/app-proxy.js`) documented with the T007 contract (via `docs/deployment.md` §"Proxy contract", verified complete — no gaps); the three gitignored local copies deleted; nothing else in `release/` touched; no tracked references to the copies as source of truth.
- [x] Nest-rollback references annotated as retired (backup-restore only, → T011) — `docs/cutover.md`, `docs/data-migration-live.md`.
- [x] Prod DB version: dated TODOs anchored to T011's AC inserted in both `docs/mariadb-compat.md:3` and `specs/security/06-…` F-02 (now :86; was :77) (T011 pending); never guessed. (doc half of **C3**).
- [x] `specs/README.md` indexes `arch-hardening-specs/` (verified present); dated lesson appended to `docs/lessons/ci-quality.md`.

## Inventory (2026-11-18, implementation evidence — C1)

Command: `grep -rniE "backend/|nestjs|typeorm|migrate\.js|build:release|kanpad-nest|localhost:3000" README.md docs/ specs/`
(full output saved by the shell to a `.out` file; categorized below).

- **`README.md`: ZERO matches** (exit 1) — already Rust-ified by T012; remaining staleness was the e2e note, the placeholder, and the portainer/dokploy checkbox (all fixed).
- **`docs/` matches, all historical or annotated as intended:** `docs/cutover.md` (lines 6/11/14 — annotated RETIRED), `docs/data-migration-live.md` (body is the historical migration procedure; `kanpad_nest_restore` is the scratch-DB name; rollback steps annotated RETIRED), `docs/lessons/*` (dated lesson logs — append-only, untouched). No live instructions in `docs/` still tell a reader to deploy/roll back via Nest.
- **`specs/implementation-artifacts/` (~600 matches):** per-story history of the pre-Rust era — annotate-nothing per house rule (history, not instructions); untouched.
- **`specs/arch-hardening-specs/` + `specs/security/`:** findings/scope statements describing the pre-T019 state (C1/C3 in `00-overview.md`, `40-operations.md` §7, security recon scope) — historical problem statements, intentionally untouched.
- **`specs/project-context.md`:** explicitly documents the deletion — untouched.
- **`specs/tasks/T019-*.md`:** this task's own references — expected.
- **Tracked references to `release/app-proxy*` as source of truth: none** (only historical problem statements, plus the pointer comment in `scripts/app-proxy.js` and this spec).
- **`specs/tasks/status.yaml`:** T019 flipped to `in_progress` by the task harness before implementation (not by this task).

## Notes

- **T011 not landed at implementation time** (`docs/operations-backup.md` absent) → `SELECT VERSION()` not on record → dated TODOs anchored to T011's AC inserted in `docs/mariadb-compat.md:3` and `specs/security/06-…` F-02 (now :86; was :77). The F-01 "11.4.x" line is a historical banner-grab — left alone. CI-MariaDB-leg consequence stays owned by `40-operations.md` §6 (untouched).
- **T011 has no `specs/tasks/status.yaml` entry** (noted, not fixed — out of scope).
- **Pre-delete diff summary of the three `release/` copies** (gitignored local staging tree; nothing in git; deleted 2026-11-18, user decision): `app-proxy.js` 100 lines, fixed backend port 3001, pre-ephemeral-port; `app-proxy-v2.js` 127 lines, ephemeral-port ancestor of the tracked script (quote-style/comment divergence, ~159 diff lines); `app-proxy-plus.js` 240 lines, fixed 9001 + static SPA serving from `public/`. `release/` otherwise untouched (`backend-rs`, `backend-rs.zip`, `.env`, `public/`).
- **Proxy**: tracked `scripts/app-proxy.js` confirmed canonical (ephemeral port, T007-compliant read); pointer comment added (no logic change, `node --check` passes). `docs/deployment.md` §"Proxy contract" already covers the full T007 checklist (Host unchanged, XFF append, X-Forwarded-Proto https-fallback, `TRUSTED_PROXIES` = the Node hop) — **no edit needed there**. No tracked file references the copies as source of truth (only historical scope statements in `specs/security/00|06` and the `arch-hardening-specs` findings/40-operations, which describe them as the *problem*; left as-is intentionally).
- **README**: accuracy pass only (T012 had already Rust-ified it): dropped the "will be filled in here" placeholder; both deploy paths now first-class with links to `docs/self-hosting.md`, `docs/deployment.md`, `specs/arch-hardening-specs/40-operations.md`; stale T017-era e2e note (`:3000`) replaced (`:3001` / `scripts/e2e-local.sh` / `frontend/playwright.config.ts`); "Templates for common self-hosting setup (portainer, dokploy)" checked off — backed by `docs/self-hosting.md` (Dokploy + Portainer walkthroughs) and `compose.yaml`. Commands **verified statically, not executed** (no builds run): `cargo run -- migrate run` / `create-admin` / `serve` exist (`backend-rs/src/main.rs` `Commands`), rust-toolchain pinned 1.99.1, `cargo llvm-cov --fail-under-lines 85` matches `.github/workflows/backend-rs.yml`, Vite proxy `/api`→`:3001`, published compose port `127.0.0.1:5173:80` matches the README's 5173 claim, TEST DB URL matches `backend-rs/docker-compose.test.yml` (throwaway `root:dev_root_pw@127.0.0.1:3337`), `scripts/e2e-local.sh` exists.
- **Nest-rollback annotations** (banner + inline retired markers; history preserved): `docs/cutover.md`, `docs/data-migration-live.md` (Option A step 7, Option B block, cleanup note).
- **`specs/README.md`** index bullet verified present (line 7, `arch-hardening-specs/`, landed in `8d70cee6`); no edit.
- **Lesson appended** to `docs/lessons/ci-quality.md` ("docs drift + proxy duplication", 2026-11-18, T019).
- `status.yaml` shows T019 flipped to `in_progress` — by the task harness, not by this task; leaving as-is per the Must-NOT constraint. Flip to `done` only at commit time with approval.
- **Not committed** (no commit approval).

## Review fixes (2026-11-18, sub-agent review round 1)

Reviewer 3 (proxy/ops) and Reviewer 2 (docs accuracy) found issues; all fixed, annotations only where history was involved:

- **`docs/deployment.md`:** §"Proxy contract" extended with the T007-mandated XFF **right-to-left** walk (skip `TRUSTED_PROXIES` and non-IP-parseable entries, first remaining entry — a **custom `ClientIpExtractor` (T30) in `backend-rs/src/lib.rs`**, *not* `tower_governor`'s `SmartIpKeyExtractor`, which reads the leftmost parseable entry and ignores any trusted-proxy list) + the operator verification (two-clients throttle smoke check; accurate pass-through failure-mode description; no echo endpoint exists in-tree) and the "never widen `TRUSTED_PROXIES` beyond the exact hop IPs" prohibition. Also fixed the inverted `frontend/.env.example` description (prod URL is commented out; the active default is `http://localhost:3001`).
- **`specs/security/06-…:16,21`:** §1.1 test-plan lines annotated SUPERSEDED 2026-11-18 (XFF append — never overwrite; wrapper binds `0.0.0.0:$PORT` with the Rust child on an ephemeral loopback port). F-02 bullet now reads "_was reported_ as MariaDB 11.4.x" (UNVERIFIED).
- **`specs/arch-hardening-specs/40-operations.md` §7:** single-proxy-owner bullet annotated **Executed 2026-11-18 (T019)** — copies deleted, contract living in deployment.md §"Proxy contract".
- **`docs/self-hosting.md:78`:** "matches CI and the production host DB" → "matches CI" + UNVERIFIED annotation for the prod-host-DB clause. Same clause annotated in **`compose.yaml`** `[MariaDB]` comment (comment-only, no service/config change).
- **`docs/mariadb-compat.md`:** line 3 now notes CI's MariaDB 11.4 leg + docker-compose.test.yml; the "Two-minute verification" step 1 no longer asserts 10.6 as fact (record + compare to the T011 drill value).
- **`docs/data-migration-live.md`:** Appendix "production flavour" annotated UNVERIFIED; the Option B imperative line struck through.
- **`docs/cutover.md`:** line 6 de-duplicated URL; "Nest service may be retired…" line marked moot (Nest already deleted).
- Reviewer 1 (spec compliance): PASS — noted only `release/.env` mtime (2026-11-18 15:19, **predates this task's deletions at 16:06; this task never read or wrote `release/.env`**) and the spec header status (fixed: now `implemented 2026-11-18 — uncommitted`).

## Review fixes (2026-11-18, sub-agent review round 2 — minor wording, all applied)

- `docs/deployment.md`: right-to-left bullet now names the custom `ClientIpExtractor` (T30) and adds the non-IP-skip + peer-fallback clauses; the operator-verification bullet now describes the two pass-through failure modes accurately (no-XFF clients → shared 127.0.0.1 bucket, probe-detectable; XFF-capable clients → spoofable bucket, not probe-detectable) and states plainly that no echo endpoint ships with the API (use a local header-dump stub).
- `compose.yaml`: "matches KanPad CI and the production host DB" → "matches KanPad CI" + UNVERIFIED note folded into the comment (comment-only).
- `docs/data-migration-live.md` appendix: "Production _was documented_ as MariaDB 10.6.28" (F-02-style fails-safe phrasing).
- **Note:** the same `SmartIpKeyExtractor` misattribution exists in the T007 task file (`specs/tasks/T007-…md`, ~line 238) — left untouched as T007's historical spec text; flagged here for the user.
- Still open for T011: prod DB version lands only when the drill records `SELECT VERSION()` (40-operations.md §6); F-02/TODO stay until then. CI MariaDB 11.4-leg consequence stays owned by §6 (untouched).

## Lessons

Promoted to `docs/lessons/ci-quality.md` (2026-11-18, "docs drift + proxy duplication"): re-verify a plan's premises before executing; deploy artifacts need in-tree references or explicit recorded ownership; "ready for dev" means re-running the task's own inventory.
