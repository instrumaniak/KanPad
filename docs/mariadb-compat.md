# MariaDB 10.6 compatibility (T006)

Production runs **MariaDB 10.6.28**; dev/CI run **MySQL 8.4** (CI additionally runs a MariaDB 11.4 leg — the `backend-mariadb` job in `.github/workflows/backend-rs.yml`; `backend-rs/docker-compose.test.yml` starts both). The Rust backend must run on both.

> **TODO (2026-11-18, T019): this version line is unverified.** Prod flavor was documented as
> 10.6.28 here and 11.4.x in `specs/security/06-infrastructure-deployment.md` (F-02). T011's
> restore drill records `SELECT VERSION()`; once it lands, correct this line (and that one) to
> the recorded value. See `specs/arch-hardening-specs/40-operations.md` §6 (T011, T019).

## Supported matrix

- MySQL **8.0.19+**
- MariaDB **10.6+**

## Two-minute verification

1. Version: `SELECT VERSION();` -> record it; it must match the T011 drill value (unverified whether prod is 10.6.x or 11.4.x — see the TODO above).
2. Login smoke: `POST /api/auth/login` with real credentials -> `200` + `Set-Cookie: __Host-kanpad.sid=...`.
3. `SELECT id, expiry_date FROM sessions;` -> one row per login.

## Portable-SQL rule

No row-alias upserts (`... VALUES (...) AS new`), no `utf8mb4_0900_*` collations, no `INSERT ... RETURNING`, no MySQL-only JSON operators (`->`/`->>`). Prefer re-binding the post-insert values as extra placeholders. See `specs/api-rewrite-rs-specs/30-db-schema.md` ("Supported servers") and `backend-rs/src/common/session_store.rs` (reference implementation).