# T018: Secrets hygiene + dependency declarations

> **STATUS: pending.** Wave 0. Closes secret/deps half of **C3** and the package half
> of **C2**.

## Goal

Close the cheap-but-real hygiene gaps:

1. **Secret file permissions:** `.env` files on disk are `644`; spec
   (`specs/security/06-infrastructure-deployment.md:34-35`) requires 600. chmod them
   and document the check in the deploy runbook.
2. **Shared live `SESSION_SECRET`:** `release/.env` (gitignored, contains a built SPA
   under `public/`) shares the real `SESSION_SECRET` with `backend-rs/.env`. If that
   tree was ever uploaded → **rotate** per the runbook (users re-login). Decide with
   user: rotate now vs record as mandatory-before-next-deploy.
3. **Undeclared imports:** `frontend/src/components/ui/tabs.tsx` / `tooltip.tsx` import
   `@radix-ui/react-tabs` / `@radix-ui/react-tooltip` which are **not in
   `package.json`** (work via hoisting only — breaks under strict installers). Declare
   them explicitly.
4. **Build-only packages in `dependencies`:** `shadcn`, `tailwindcss`,
   `@tailwindcss/vite`, `tw-animate-css` → `devDependencies` (they're never imported by
   `src/`).

## Depends On

**T016** — both tasks edit `frontend/package.json` (T016 wires the budget script;
T018 adds/moves dependencies). Serialized so two agents never edit the file at once.

## Phase

0

## Critical

No — LOW severity each, but trivial and they're security-adjacent (C3).

## Spec References

- `specs/arch-hardening-specs/40-operations.md` §5 (secrets & config hygiene design:
  chmod, rotation runbook, dep moves, compose-loopback pointer).
- `specs/arch-hardening-specs/00-overview.md` — findings C2, C3; global DoD.
- `frontend/package.json` (deps), `frontend/src/components/ui/{tabs,tooltip}.tsx`
  (undeclared imports), root `.gitignore` (already ignores `.env` — verified),
  `specs/security/06-infrastructure-deployment.md` (600 rule), `release/.env` +
  `backend-rs/.env` (on-disk, gitignored).

## Files to Create/Modify

| File | Change |
|---|---|
| On-disk `.env` files (`backend-rs/.env`, `release/.env`, `frontend/.env`) | `chmod 600` (not a git diff — record in evidence) |
| `docs/operations-backup.md` or `40-operations` runbook section | Deploy-time secret checklist (`stat -c %a` = 600) + SESSION_SECRET rotation runbook |
| `frontend/package.json` | + `@radix-ui/react-tabs`, `@radix-ui/react-tooltip` (deps); move `shadcn`/`tailwindcss`/`@tailwindcss/vite`/`tw-animate-css` → `devDependencies` |
| `frontend/package-lock.json` | Regenerated |
| `docs/lessons/frontend.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** any real secret value, `.env` contents (stays
gitignored), runtime source changes.

## Implementation Steps

1. Verify ignore status (`git check-ignore` on each `.env`) — already true, re-assert.
2. `chmod 600` the on-disk env files; paste `ls -l` before/after as evidence.
3. **Rotation decision with user** (open call below): if rotating — generate new
   `SESSION_SECRET`, update envs, restart, verify login + old cookie rejected; record.
4. `npm ls @radix-ui/react-tabs @radix-ui/react-tooltip` (currently hoisted/absent) →
   add explicit deps → `npm ci` with strict hoisting proof (e.g. `npm install
   --omit=dev` still builds? — build-only packages must not be needed at runtime;
   verify `npm run build` works after the move).
5. Move the four build-only packages to `devDependencies`; `npm ci` + `npm run build` +
   full vitest green.
6. Runbook section: secret checklist + rotation steps.
7. Lesson.

## Constraints

- Never print or commit secret values (evidence = file modes + hashes only).
- `npm run build`, `gen:api -- --check`, `tsc --noEmit`, vitest must all stay green
  after dep moves (proves nothing build-time leaked out of `devDependencies`).
- Do not touch Rust deps (Cargo) — scope is frontend + ops.
- Global DoD; no commit without approval.

## Acceptance Criteria

- [ ] All `.env` files mode 600 (paste `ls -l`), checklist documented (closes secret
      half of **C3**).
- [ ] SESSION_SECRET rotation decided + either executed or scheduled in the runbook
      (evidence in §Notes).
- [ ] `@radix-ui/react-tabs`/`-tooltip` explicitly declared; strict-install build green
      (closes the undeclared half of **C2**).
- [ ] Build-only packages in `devDependencies`; `npm run build` + vitest +
      `tsc --noEmit` green (rest of **C2**).
- [ ] No secret values in the diff; lesson appended.

## Notes

(filled in during/after implementation)

**Open call:** rotate SESSION_SECRET now vs before-next-deploy. *Default: rotate now
(cheap; `release/` tree contained a built SPA — treat as possibly exposed).*

## Lessons

(pending)
