# Cutover runbook (T51)

> **RETIRED 2026-11-18 (T019):** the Nest backend was deleted (`4051be68`). Every
> instruction in this document that rolls the app back to Nest — `VITE_API_URL=http://localhost:3000`,
> the `:3000` drill, the `kanpad-nest:final` image — is **void**. Rollback is
> **backup-restore only** (T011; drill record lands in `docs/operations-backup.md`
> when T011 completes). For the current topology and deploy paths see
> `docs/deployment.md` and `specs/arch-hardening-specs/40-operations.md`. This file is
> kept as historical record; the annotations below mark each retired step inline.

## Default target

- Vite dev proxy and `compose.yaml` frontend env now default to the Rust backend on `:3001`.
- Roll back the app only: set `VITE_API_URL=http://localhost:3000` (in compose / frontend build) and restart/rebuild. No DB change is needed for the app rollback. *(RETIRED 2026-11-18 — Nest is deleted; see banner above.)*
- Confirm routing with the `served-by` response header: Rust responses carry `served-by: rust`, Nest responses do not.

## App rollback drill (< 5 minutes)

> **RETIRED 2026-11-18 (T019):** Nest `:3000` no longer exists; this drill cannot be
> executed. The only DB rollback path is backup-restore (T011). The only app rollback
> is redeploying the previous known-good artifact (previous `dist/` + previous
> `backend-rs` binary).

1. Set `VITE_API_URL=http://localhost:3000`. *(retired)*
2. Rebuild/restart the frontend (or revert the single default change). *(retired)*
3. Verify `served-by: rust` is gone and Nest `:3000` serves `/api`. *(retired)*
4. Nest image `kanpad-nest:final` is retained as the rollback target. *(retired 2026-11-18 — Nest is deleted (`4051be68`); rollback story is backup-restore only, → T011)*

## DB rollback

- DB rollback is **restore from backup only**. Never run `migrate revert` against production.
- Rehearse the restore-from-backup drill on staging and record timing.

## Session behavior

- Cookies are not portable between Nest and Rust (`SESSION_SECRET` and the session codec differ).
- Expect a forced re-login on both directions of a cutover/rollback.

## Blue-green production path (order is load-bearing)

See `specs/api-rewrite-rs-specs/31-live-unicode-migration.md` section 31.5 for the full staging proof checklist:

1. Freeze writers and back up production.
2. Boot baseline migration on an **empty** staging DB (MariaDB 10.6 image).
3. Restore the production dump with the ghost-column / session-table transform (`--ignore-table` for `session` and `typeorm_migrations`, strip ghost columns).
4. Run `migrate run` again so `0002_convert_to_utf8mb4.sql` applies, then run the unicode emoji gate and e2e suites.
5. Verify `migrate info` reports `pending=0`.
6. Promote staging to production, then unfreeze writers.
7. Never run `migrate run` in place against the live Nest DB.

## Post-cutover checks

- `/health` returns `200 {status: ok}`.
- `/ready` returns `200` when the DB is up and `503` problem+json when it is down.
- Every API route advertises `served-by: rust` and throttling headers under `/api/*`.
- Nest service may be retired only after the rollback window is closed. *(moot 2026-11-18 — Nest is already deleted (`4051be68`); nothing left to retire)*
