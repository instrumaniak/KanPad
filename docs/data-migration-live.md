# Live data migration guide: NestJS / TypeORM -> Rust / backend-rs

Status: tested on local dev import with:
- Nest dump: `raziurco_kanbanflow_prod_03_full-2026-10-6.sql.gz`
- Rust staging DB: `kanpad_rs_dev`
- Rust schema after `cargo run migrate run` (`0001_baseline.sql` + `0002_convert_to_utf8mb4.sql`)

This runbook migrates application data from a live Nest/TypeORM database to the Rust
schema while preserving IDs. It does not migrate sessions: Rust uses the store-owned
`tower_sessions.session` schema and sessions are force-relogin on cutover.

## Principles

- Do not run the Rust baseline migration against a populated Nest database.
- Build a separate Rust database with an empty schema, then import data.
- Restore Nest data into a temporary source database first.
- Strip ghost columns and Nest-only tables during import.
- Validate staging before switching production traffic.
- Own DB rollback as backup restore, never `migrate revert`.

## Required inputs

- Rust sources: `backend-rs/`
- Nest data dump, preferably `mysqldump --no-data` reviewed against deviations plus `--no-create-info` data dump.
- MySQL 8.4 or MariaDB 10.6 staging instance.
- Empty Rust target database, e.g. `kanpad_rs_dev`.

## 1. Back up production

```bash
mysqldump --single-transaction --quick --routines --triggers \
  --source_data=ON --set-gtid-purged=OFF \
  raziurco_kanbanflow > kanpad-prod-pre-cutover.sql
gzip -k kanpad-prod-pre-cutover.sql
```

Keep this backup outside the database host. It is the only production DB rollback.

## 2. Create a temporary Nest source database

```bash
mysql -u "$DB_USER" -p"$DB_PASSWORD" \
  -e "DROP DATABASE IF EXISTS kanpad_nest_restore; CREATE DATABASE kanpad_nest_restore CHARACTER SET latin1 COLLATE latin1_swedish_ci;"
```

Restore the Nest dump (schema + data) into that temporary database:

```bash
gunzip -c raziurco_kanbanflow_prod_03_full-2026-10-6.sql.gz \
  | mysql -u "$DB_USER" -p"$DB_PASSWORD" kanpad_nest_restore
```

This keeps the live database untouched and lets you apply column-level transforms safely.

## 3. Prepare the Rust staging database

Use an empty database only. Configure `backend-rs/.env` or environment variables:

```bash
DB_HOST=127.0.0.1
DB_PORT=3306
DB_USERNAME=user
DB_PASSWORD=password
DB_NAME=kanpad_rs_dev
NODE_ENV=development
SESSION_SECRET="..."
```

Apply the Rust migrations:

```bash
cd backend-rs
cargo run migrate run
cargo run migrate info   # expect: pending=0
```

The Rust schema is in `migrations/0001_baseline.sql` and `migrations/0002_convert_to_utf8mb4.sql`.

## 4. Clear Rust staging app tables

The database is already migrated, but app data should start empty:

```sql
SET FOREIGN_KEY_CHECKS=0;
SET time_zone='+00:00';

TRUNCATE TABLE note_tags;
TRUNCATE TABLE tags;
TRUNCATE TABLE notes;
TRUNCATE TABLE checklist_items;
TRUNCATE TABLE checklists;
TRUNCATE TABLE card_labels;
TRUNCATE TABLE labels;
TRUNCATE TABLE cards;
TRUNCATE TABLE columns;
TRUNCATE TABLE boards;
TRUNCATE TABLE projects;
TRUNCATE TABLE users;
```

The `_sqlx_migrations` and `tower_sessions.session` tables are intentionally untouched.

## 5. Import application data with transforms

Run cross-database INSERTs from `kanpad_nest_restore` into `kanpad_rs_dev`.

### Ghost columns stripped

Nest has columns that the Rust baseline deliberately omits:

- `users.username`
- `projects.description`

These must be omitted from the INSERT column list. The Rust baseline does not have them.

### Tables imported

```sql
SET FOREIGN_KEY_CHECKS=0;
SET time_zone='+00:00';
SET NAMES utf8mb4;

INSERT INTO kanpad_rs_dev.users (id, email, password, role, created_at, updated_at)
SELECT id, email, password, role, created_at, updated_at
FROM kanpad_nest_restore.users;

INSERT INTO kanpad_rs_dev.projects (id, name, user_id, created_at, updated_at)
SELECT id, name, user_id, created_at, updated_at
FROM kanpad_nest_restore.projects;

INSERT INTO kanpad_rs_dev.boards
  (id, name, background_color, user_id, project_id, is_archived, view_mode, created_at, updated_at)
SELECT id, name, background_color, user_id, project_id, is_archived, view_mode, created_at, updated_at
FROM kanpad_nest_restore.boards;

INSERT INTO kanpad_rs_dev.columns
  (id, name, position, board_id, created_at, updated_at)
SELECT id, name, position, board_id, created_at, updated_at
FROM kanpad_nest_restore.columns;

INSERT INTO kanpad_rs_dev.cards
  (id, title, column_id, position, description, due_date, created_at, updated_at)
SELECT id, title, column_id, position, description, due_date, created_at, updated_at
FROM kanpad_nest_restore.cards;

INSERT INTO kanpad_rs_dev.labels
  (id, name, color, user_id, created_at, updated_at)
SELECT id, name, color, user_id, created_at, updated_at
FROM kanpad_nest_restore.labels;

INSERT INTO kanpad_rs_dev.card_labels (card_id, label_id)
SELECT card_id, label_id
FROM kanpad_nest_restore.card_labels;

INSERT INTO kanpad_rs_dev.checklists (id, title, card_id, created_at, updated_at)
SELECT id, title, card_id, created_at, updated_at
FROM kanpad_nest_restore.checklists;

INSERT INTO kanpad_rs_dev.checklist_items
  (id, text, is_completed, checklist_id, position, created_at, updated_at)
SELECT id, text, is_completed, checklist_id, position, created_at, updated_at
FROM kanpad_nest_restore.checklist_items;

INSERT INTO kanpad_rs_dev.notes
  (id, title, content, board_id, project_id, card_id, user_id, created_at, updated_at)
SELECT id, title, content, board_id, project_id, card_id, user_id, created_at, updated_at
FROM kanpad_nest_restore.notes;

INSERT INTO kanpad_rs_dev.tags (id, name, color, user_id, created_at)
SELECT id, name, color, user_id, created_at
FROM kanpad_nest_restore.tags;

INSERT INTO kanpad_rs_dev.note_tags (note_id, tag_id)
SELECT note_id, tag_id
FROM kanpad_nest_restore.note_tags;

SET FOREIGN_KEY_CHECKS=1;
```

## 6. Reset AUTO_INCREMENT counters

MySQL does not accept user variables in `AUTO_INCREMENT = ...`, so use literals:

```sql
ALTER TABLE kanpad_rs_dev.users AUTO_INCREMENT = 3;
ALTER TABLE kanpad_rs_dev.projects AUTO_INCREMENT = 4;
ALTER TABLE kanpad_rs_dev.boards AUTO_INCREMENT = 12;
-- repeat for columns/cards/labels/checklists/checklist_items/notes/tags
```

Generate the literal values from `SELECT COALESCE(MAX(id), 0) + 1 ...`.

## 7. Tables intentionally not imported

- `session` / Nest authentication sessions: Rust uses `tower_sessions.session` in schema `tower_sessions`, with a different shape and cookie codec. Users will force-relogin.
- `typeorm_migrations`: Rust uses `_sqlx_migrations`.
- `users.username` and `projects.description`: Rust baseline intentionally omits these ghost columns.

## 8. Authentication compatibility

Nest passwords are bcrypt hashes (`$2a$` / `$2b$`). Rust login:
1. verifies Argon2 for Rust-created passwords;
2. verifies bcrypt cost-10 hashes on imported Nest data;
3. upgrades successful bcrypt logins to Argon2 and writes the row back.

No manual password conversion is needed.

## 9. Validation checklist

```sql
-- zero orphan FKs
SELECT COUNT(*) FROM kanpad_rs_dev.projects p
LEFT JOIN kanpad_rs_dev.users u ON u.id = p.user_id WHERE u.id IS NULL;
-- repeat for every relation

-- counts match live source
SELECT COUNT(*) FROM kanpad_rs_dev.cards;
SELECT COUNT(*) FROM kanpad_nest_restore.cards;
```

Application-level checks:

- `/health` returns `200 {"status":"ok"}`.
- `/ready` returns `200` when DB is up.
- Login with imported user succeeds and upgrades password hash.
- `/api/v1/boards` returns imported boards.
- `/api/v1/boards/:id` returns nested columns ordered by `position ASC`.
- Cards list does not include `description` key but `has_description` is correct.
- Labels and tags match imported data.
- Notes return `content` only on detail, not list.

Unicode gate, if live data contains emoji:

```sql
SELECT HEX(title) FROM kanpad_rs_dev.notes WHERE title LIKE '%🚀%';
```

Also verify existing curly apostrophes round-trip as `E28099`.

## 10. Cutover options

### Option A: Local or controlled blue-green (recommended)

1. Start Rust on `:3001` using `kanpad_rs_dev`.
2. Build frontend with `VITE_API_URL=http://localhost:3001`.
3. Verify `served-by: rust`.
4. Freeze writes.
5. Repeat only if data changed after staging import (or keep manual sync window short).
6. Promote Rust as default.
7. Unfreeze writes.

### Option B: One-line app rollback

If anything fails:

```bash
VITE_API_URL=http://localhost:3000
```

Point the frontend back to Nest. No database rollback is lost.

### Option C: DB rollback

DB rollback is restore from `kanpad-prod-pre-cutover.sql.gz` only. Never run `migrate revert` on production.

## 11. Post-cutover cleanup

After validating:

```bash
mysql ... -e "DROP DATABASE kanpad_nest_restore;"
```

Remove deployment temp files but keep the final compressed backup outside the host.

## New-user defaults

Nest data may not contain Rust’s six default labels for every user. Rust registers them automatically for newly registered users. Existing imported users will simply not have those defaults unless you seed them:

```sql
INSERT INTO kanpad_rs_dev.labels (name, color, user_id, created_at, updated_at)
SELECT d.name, d.color, u.id, NOW(6), NOW(6)
FROM kanpad_rs_dev.users u
CROSS JOIN (
  SELECT 'Urgent' AS name, 'red' AS color
  UNION ALL SELECT 'Important', 'orange'
  UNION ALL SELECT 'In Progress', 'yellow'
  UNION ALL SELECT 'Green', 'green'
  UNION ALL SELECT 'Blue', 'blue'
  UNION ALL SELECT 'Purple', 'purple'
) d
WHERE NOT EXISTS (
  SELECT 1 FROM kanpad_rs_dev.labels l
  WHERE l.user_id = u.id AND l.name = d.name
);
```

## Summary

This migration works by:
- preserving IDs;
- stripping ghost columns;
- importing data separately from schema creation;
- upgrading bcrypt logins to Argon2;
- abandoning Nest sessions;
- using staging validation and backup-only DB rollback.
