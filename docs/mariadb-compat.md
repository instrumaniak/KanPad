# MariaDB 10.6 compatibility (T006)

Production runs **MariaDB 10.6.28**; dev/CI run **MySQL 8.4**. The Rust backend must run on both.

## Supported matrix

- MySQL **8.0.19+**
- MariaDB **10.6+**

## Two-minute verification

1. Version: `SELECT VERSION();` -> `10.6.x-MariaDB`.
2. Login smoke: `POST /api/auth/login` with real credentials -> `200` + `Set-Cookie: __Host-kanpad.sid=...`.
3. `SELECT id, expiry_date FROM sessions;` -> one row per login.

## Portable-SQL rule

No row-alias upserts (`... VALUES (...) AS new`), no `utf8mb4_0900_*` collations, no `INSERT ... RETURNING`, no MySQL-only JSON operators (`->`/`->>`). Prefer re-binding the post-insert values as extra placeholders. See `specs/api-rewrite-rs-specs/30-db-schema.md` ("Supported servers") and `backend-rs/src/common/session_store.rs` (reference implementation).