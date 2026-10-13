# Cutover runbook (T51)

## Default target

- Vite dev proxy and `compose.yaml` frontend env now default to the Rust backend on `:3001`.
- Roll back the app only: set `VITE_API_URL=http://localhost:3000` (or `VITE_API_URL=http://localhost:3000` in compose / frontend build) and restart/rebuild. No DB change is needed for the app rollback.
- Confirm routing with the `served-by` response header: Rust responses carry `served-by: rust`, Nest responses do not.

## App rollback drill (< 5 minutes)

1. Set `VITE_API_URL=http://localhost:3000`.
2. Rebuild/restart the frontend (or revert the single default change).
3. Verify `served-by: rust` is gone and Nest `:3000` serves `/api`.
4. Nest image `kanpad-nest:final` is retained as the rollback target.

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
- Nest service may be retired only after the rollback window is closed.
