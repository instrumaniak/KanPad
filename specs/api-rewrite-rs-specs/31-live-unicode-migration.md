# 31 — Live unicode + migration strategy (companion to 30; both files agree)

Sources verified 2026-10-11: prod dump
`kanpad-db-dump/raziurco_kanbanflow_prod_03_full-2026-10-6.sql.gz`
(MariaDB 10.6.28, `SET NAMES utf8mb4`), `backend/src/scripts/migrate.ts:1-37`,
`backend/webpack.config.js:4-10`, `backend/package.json:29-32`,
`backend/src/database/data-source-options.ts:5-22`. Percona utf8mb4
considerations (index byte math, `CONVERT TO` vs `MODIFY`) and sqlx
`Migrator` docs (embedded `migrate!`, `set_locking(true)` default,
`.gitattributes eol=lf` for stable checksums).

## 31.1 Prod findings (why this file exists)

* Every prod table is `DEFAULT CHARSET=latin1 COLLATE=latin1_swedish_ci`
  (`boards`, `cards`, `notes`, `users`, `session`, `typeorm_migrations`, …).
  Root cause: all 10 TypeORM migrations use bare `ENGINE=InnoDB` with no
  charset (`1700000000000-CreateUsersProjects.ts:18`,
  `1745862000000-CreateBoardsColumns.ts:8`, …) — tables inherit the live DB
  default, which is `latin1`.
* Wire is `utf8mb4` (`SET NAMES utf8mb4` in dump) but storage is `latin1`:
  MySQL converts on every insert. Characters outside the `latin1/cp1252`
  repertoire (emoji, Bengali `U+0980+`, CJK) become `?` permanently.
* Byte audit of prod_03: 62KB dump, **3 non-ASCII bytes = 1 char `’` U+2019**
  (in a card description). Zero emoji/CJK. `’` is inside `cp1252`, so it
  round-trips today. **Conversion is still lossless — but only just.**
* Journal: `typeorm_migrations` has exactly 10 rows (tip
  `AddViewModeToBoards1779600000000`). Ghost cols `users.username` /
  `projects.description` exist in prod (defaults/NULL, harmless).
* Release flow today: webpack bundles `release/{app,migrate,create-admin}.js`
  (`keep_classnames:true` because migration names persist in the journal);
  live runs `node migrate.js` manually (`transaction:'each'`).

## 31.2 Unicode persistence (end-to-end, test-gated)

1. **Charset/collation**: DB default + every `CREATE TABLE` + every text column
   spells `CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci`,
   `ENGINE=InnoDB`, `ROW_FORMAT=DYNAMIC`. (`30-db-schema.md:11` already says
   this for fresh DBs — this file extends it to live conversion.)
2. **Portability**: live is **MariaDB 10.6.28**, plan pins MySQL 8.4 for
   dev/CI. `utf8mb4_0900_ai_ci` does not exist on MariaDB 10.6
   (`Unknown collation` on import). Baseline stays `utf8mb4_unicode_ci`.
   Move to `utf8mb4_uca1400_ai_ci` (MariaDB ≥10.11) or `0900_ai_ci`
   (MySQL 8.4 engine switch) only as an explicit later migration.
3. **Connection**: `DATABASE_URL=...?charset=utf8mb4`; smoke test asserts
   `SELECT @@character_set_client, @@collation_connection`. Keep
   `mysqld --default-time-zone='+00:00'` + `NaiveDateTime`/`NOW(6)` (no change).
4. **Pre-checks on live** (paste outputs in PR):
   ```sql
   SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, CHARACTER_MAXIMUM_LENGTH, DATA_TYPE
   FROM information_schema.columns
   WHERE TABLE_SCHEMA = DATABASE() AND CHARACTER_MAXIMUM_LENGTH > 16383
     AND DATA_TYPE NOT LIKE '%text%' AND DATA_TYPE NOT LIKE '%blob%';
   -- expect 0 rows (max today is varchar(255)); else convert to TEXT first.
   ```
   Index math: `users.email varchar(255)` UNIQUE → `255×4=1020` bytes: OK for
   `DYNAMIC` (3072) but fails for `COMPACT` (767) — conversion migration sets
   `ROW_FORMAT=DYNAMIC` first.
5. **Gate test** (fails on `latin1`, passes after): insert
   `'কানপ্যাড 🚀 𐍈'` into `boards.name / cards.title / notes.content /
   checklist_items.text`, re-read, assert byte-identical. Post-convert spot
   check: `SELECT HEX(description) FROM cards WHERE ...` shows `E28099` for
   the existing `’` (not `92`).

## 31.3 One-shot live conversion (`0002_convert_to_utf8mb4.sql`)

Small DB (KBs) — no `pt-online-schema-change` needed. Use `CONVERT TO`
(rewrites data) — never bare `MODIFY` (relabels bytes → mojibake):

```sql
-- No hardcoded schema names (runs on staging/dev/prod alike — never `ALTER DATABASE raziurco_kanbanflow`).
-- No Nest `session` convert: staging migrated from the baseline has no Nest table (unconditional ALTER would fail fail-fast); the table is abandoned at cutover.
ALTER TABLE `boards` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
-- ... repeat per app table (order irrelevant, no FK change).
-- `sessions` (in the app DB) is created utf8mb4 by the baseline already.
```

Owned by new task `T02b` (see `50-tasks.md` Phase 6). Staging proves it on a
**MariaDB 10.6 image (live engine)** as well as MySQL 8.4 dev/CI — the
conversion gate must pass on the engine it will run on live; live runs it via
the same `migrate run` binary. Staging from the baseline is already utf8mb4, so `0002` there is a rewrite no-op — T02b ALSO builds a latin1 fixture DB containing `’`, runs `0002`, and asserts `HEX=E28099` (the only proof `CONVERT TO` is lossless). PII hygiene: redact emails/hashes in pasted PR evidence (hash-prefix only); staging access-controlled, dumps deleted after proof, backups encrypted.

## 31.4 Rust migration CLI (mirrors `node migrate.js`)

```
cargo build --release     # single static binary backend-rs
./backend-rs migrate run  # = node migrate.js (manual, SSH, same step)
./backend-rs migrate info # = migration:show (pending/applied + checksums)
./backend-rs serve        # = node dist/main (NO auto-migrate by default)
./backend-rs create-admin # = node release/create-admin.js
./backend-rs export-openapi  # writes openapi.json; boots NO pool/sessions
```

* Embed: `static MIGRATOR: Migrator = sqlx::migrate!("./migrations");` —
  migrations ship inside the binary (same property as the webpack bundle).
* `serve` does **not** migrate by default; `--auto-migrate` flag for
  dev/compose only. Rationale: multi-replica boot race, slow boots on large
  alters, surprise schema change on rollback. The manual step stays manual.
* Locking stays on (`set_locking(true)`, MySQL `GET_LOCK`). Never disable in
  prod. Add `.gitattributes`: `*.sql text eol=lf` (CRLF checkouts change
  SHA-384 → `VersionMismatch` on live). `sqlx.toml` pins
  `database-url-var`; journal stays `_sqlx_migrations` (do not rename).
* `migrate revert`/`.down.sql` for dev only. **Prod rollback = restore backup**,
  not down-migrations (down cannot un-`CONVERT` losslessly).

## 31.5 Cutover runbook (order is load-bearing; blue-green ONLY)

There is no in-place upgrade path: `migrate run` on the non-empty Nest DB is
**unsupported** (baseline `0001` has no `IF NOT EXISTS` by design — it fails
fast on existing tables), and sqlx 0.8 has no stamp/mark-applied command, so
a "one-shot baseline stamp" on live is not implementable. Cutover always
promotes a prepared staging database to prod. Two downtime windows (freeze →
final delta) are expected; plan them explicitly.

1. Freeze writes + backup: stop writers (maintenance page / read-only), then
   `mysqldump --default-character-set=utf8mb4 --single-transaction
   --routines --triggers prod > backup.sql` (+ `mysqladmin ping`). Keep
   `kanpad-nest:final` image for <5min app-level rollback (DB rollback is
   always restore-from-backup, never down-migrations — down cannot
   un-`CONVERT` losslessly).
2. Empty staging (MariaDB 10.6 image — live engine) → `backend-rs migrate run`
   (baseline `0001`, utf8mb4 DDL) → `migrate info` pending=0.
3. Restore into staging with an explicit transform — a plain data-only dump
   FAILS here (prod `INSERT`s name ghost columns `users.username` /
   `projects.description` and tables `session` / `typeorm_migrations`, none of
   which exist in the baseline):
   `mysqldump … --no-create-info --ignore-table=prod.session --ignore-table=prod.typeorm_migrations`
   + strip ghost columns from the dump (column-list rewrite or `sed`; the T02b
   staging proof pins the exact transform) with `FOREIGN_KEY_CHECKS=0` during
   load.
4. `backend-rs migrate run` again on staging (applies `0002` convert; on the
   already-utf8mb4 staging tables this rewrites but changes nothing — the real
   convert happens only if `0002` ever runs against a latin1 database) →
   emoji gate test → e2e (reorder tx, cascades) → `migrate info` pending=0.
5. Promote staging to prod (DNS / connection-string switch), keep
   `typeorm_migrations` abandoned one release on the OLD database only (don't
   drop, don't copy). Unfreeze writers. Old `connect.sid` cookies 401 → login
   screen on both switch directions (documented force re-login).
6. Rollback: app-level = one-line revert to `kanpad-nest:final` (<5 min, still
   against the OLD database while it is retained); DB-level = restore backup.
   Never run `migrate revert` on prod.
