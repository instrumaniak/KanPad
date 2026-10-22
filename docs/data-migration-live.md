# Data migration runbook: NestJS/TypeORM → Rust (backend-rs)

**Who this is for:** a human operator moving the production KanPad database from the
old NestJS backend to the new Rust backend, in one controlled window.

**What it does, in one sentence:** it restores the prod dump into a scratch database,
creates a *fresh* Rust database from the repo's migrations, then `INSERT … SELECT`s
the app data across — preserving IDs, dropping nothing, and converting the data to
proper utf8mb4 on the way.

**Seats at the table:**

| Database | Role |
|---|---|
| prod (live) | **read-only.** We never run migrations against it. Dump it once and leave it alone. |
| `kanpad_nest_restore` | scratch. One-time copy of the prod dump; safe to delete afterwards. |
| `kanpad_rs_dev` (staging) | Rust-shaped target. Validated first, promoted to prod later. |
| `kanpad_rs_ref` | reference. Same migrations, empty — the "what the schema should be" baseline for the step 5(d) drift gate. Dropped in step 10. |

**Total hands-on time:** ~20 minutes for a database of this size, plus the API spot checks.

> **Devices needed:** shell access to the staging host, the prod dump file
> (`raziurco_kanbanflow_prod_04_full-2026-10-18.sql.gz` or a *fresher* one — always
> regenerate the backup right before cutover), and one prod user password so you can
> test logging in later.

**Step order matters:** verify (steps 5–6) runs **before** the label seed (step 7),
because the seed *adds* rows to `labels` and would otherwise break the row-count
parity gate. Run the steps in the order given.

---

## 0. Pre-flight (2 min)

From the repository root:

```bash
# A) Confirm the Rust code is current — check migrations have NOT been
#    migrated into some older database with old file checksums
git log -1 --format='%h %ad %s' --date=short -- backend-rs/migrations/0001_baseline.sql
#    ✓ you should see commit d71178c5 (2026-10-19) or later. Anything earlier
#    means the baseline's session table didn't live in the app DB and any
#    pre-existing staging DB will refuse to re-migrate (see Troubleshooting #1).

# B) Load credentials (backend-rs/.env drives every later command)
cat backend-rs/.env
```

`backend-rs/.env` needs exactly these keys (edit to match your staging MySQL):

```dotenv
DB_HOST=127.0.0.1
DB_PORT=3306
DB_USERNAME=user
DB_PASSWORD=<staging password>
DB_NAME=kanpad_rs_dev
APP_ENV=development
# The connection's collation is PINNED (`54` Part C §C6): sqlx 0.9 sends
# only `SET NAMES utf8mb4` and lets the server pick the collation — MySQL 8's
# default is `utf8mb4_0900_ai_ci`, not this schema's `utf8mb4_unicode_ci`.
# The DATABASE_URL below must therefore carry it too.
SESSION_SECRET=<32+ hex chars — set a FRESH value at cutover>
```

> The value of `DB_NAME` is the staging DB used in steps 3–8. Keep it as
> `kanpad_rs_dev` unless you have a reason to rename it.

Shell helper for the next steps — run once per terminal:

```bash
set -a; source backend-rs/.env; set +a

# Credentials go into a chmod-600 client config, NOT the environment or the
# command line: MYSQL_PWD is deprecated and shows up in `ps`, and `-p<pass>`
# would leak the password into shell history. MySQL/MariaDB read them from
# this file, so every later command stays non-interactive.
umask 177
cat > "$HOME/.my-kanpad.cnf" <<EOF
[client]
host=${DB_HOST:-127.0.0.1}
port=${DB_PORT:-3306}
user=${DB_USERNAME}
password=${DB_PASSWORD}
EOF
# (a DB_HOST of "localhost" makes the mysql CLI use the Unix socket — fine when
#  that's the same daemon Rust talks to over 3306; otherwise set it explicitly.)
chmod 600 "$HOME/.my-kanpad.cnf"

# delete it again at the end:  rm "$HOME/.my-kanpad.cnf"

MYSQL="mysql --defaults-extra-file=$HOME/.my-kanpad.cnf"   # flag must be FIRST
DUMP=/path/to/raziurco_kanbanflow_prod_04_full-2026-10-18.sql.gz

$MYSQL -e "SELECT 1;"     # ✓ prints "1" — credentials work, no prompt asked
```

---

## 1. Back up production (do this FIRST — it is your only DB rollback)

On the production database host:

```bash
mysqldump --single-transaction --quick --routines --triggers \
  raziurco_kanbanflow | gzip -c > kanpad-prod-pre-cutover.sql.gz
```

> No replication-position flags here on purpose: `--source-data` /
> `--set-gtid-purged` differ between MySQL and MariaDB versions and are only
> needed for binlog/replication-based rollbacks — ours is a file restore, so
> plain `--single-transaction … --triggers` is the portable, valid form on the
> MariaDB 10.6 prod host.

Copy `.gz` off the server (another box / object storage). **Keep it until the Rust
rollback window is over.** If anything goes wrong in production later, *restore from
this file* — never `sqlx migrate revert` (see Options → C below).

---

## 2. Restore the prod dump into a scratch database

```bash
$MYSQL -e "DROP DATABASE IF EXISTS kanpad_nest_restore;
           CREATE DATABASE kanpad_nest_restore \
           CHARACTER SET latin1 COLLATE latin1_swedish_ci;"

gunzip -c "$DUMP" | $MYSQL kanpad_nest_restore
#    ✓ you should see NO output at all — silence is success here
```

*Why latin1:* the prod tables are themselves `latin1` (that's the old bug the Rust
side fixes). The dump's `CREATE TABLE` statements carry no explicit charset, so the
tables inherit the **scratch database's** default — declaring the scratch DB as
`latin1` is what makes the restored copy byte-identical to prod before any
conversion happens.

> **The dump file itself is already UTF-8.** `mysqldump` transcoded the latin1
> columns to utf8mb4 when it *read* them, so `…prod_04…sql.gz` is valid UTF-8: it
> carries card 67's `’` as the bytes `E2 80 99` and contains **no** raw `0x92`
> byte. Restoring that file into the `latin1` scratch DB therefore *re-narrows*
> it to cp1252 bytes on insert — deliberately, because that makes the scratch copy
> a **byte-exact replica of prod** (prod stores `0x92`; the scratch stores `0x92`).
> Restoring the same file into a utf8mb4 database instead yields **identical text**
> (verified on prod_04: 0 differences on every text column of every table) — so
> the scratch DB exists to be a trustworthy *reference* for the step-6 fidelity
> gates, not because utf8mb4 storage would lose anything. The actual
> latin1→utf8mb4 conversion happens later, in step 4's `INSERT … SELECT` (client
> charset `utf8mb4` → utf8mb4 target columns); `0002_convert_to_utf8mb4.sql` is a
> no-op on the target, because the baseline already creates utf8mb4 tables.

Confirm the copy looks like prod:

```bash
$MYSQL -N -e "SELECT table_name FROM information_schema.tables \
              WHERE table_schema='kanpad_nest_restore' ORDER BY 1;"
```

✓ exactly **14** tables: `boards, card_labels, cards, checklist_items, checklists,
columns, labels, note_tags, notes, projects, session, tags, typeorm_migrations, users`.
Two of them (`session`, `typeorm_migrations`) are Nest-only — they are **skipped by
design** in step 4.

Also confirm the charset actually took (every table must be `latin1_swedish_ci`):

```bash
$MYSQL -N -e "SELECT DISTINCT table_collation FROM information_schema.tables \
              WHERE table_schema='kanpad_nest_restore';"
# ✓ exactly one row: latin1_swedish_ci
```

---

## 3. Create and migrate the empty Rust staging database

> ⚠ **Never run `migrate run` on a populated database.**
> `0001_baseline.sql` has no `IF NOT EXISTS` on purpose (it fails loudly instead
> of silently skipping drift), and **sqlx 0.8.6 has no `stamp` / mark-applied
> command** — so the Rust schema can only ever be built on an *empty* database.
> Verified with the real CLI:
> ```
> $ sqlx migrate run
> error: while executing migration 1: error returned from database:
>   1050 (42S01): Table 'sessions' already exists
> ```
> **If you ever hit this, the journal is now dirty.** sqlx writes the row
> *before* running the migration, so `_sqlx_migrations` holds `version=1,
> success=0` and every later run dies with:
> ```
> error: migration 1 is partially applied; fix and remove row from '_sqlx_migrations'
> ```
> Recovery, then re-create the database from this step:
> ```sql
> DELETE FROM _sqlx_migrations WHERE success = 0;
> ```
> Never hand-edit a `success=1` row — sqlx checksums are SHA-384s over the
> migration file's bytes (see Troubleshooting #1).

```bash
$MYSQL -e "DROP DATABASE IF EXISTS kanpad_rs_dev;
           CREATE DATABASE kanpad_rs_dev \
           CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

cd backend-rs
cargo run migrate run      # ✓ prints:  migrations applied: 2
cargo run migrate info
# ✓ prints three lines; the LAST one must be:  applied: 2 pending: 0
#   (the exact string is "applied: 2 pending: 0" — 2 = the number of migration
#    FILES, not tables; the baseline creates 12 app tables + sessions, and
#    0002 converts those 12 to utf8mb4)
```

Double-check the outcome that matters — the session table must live **inside the app
database** (Rust changed this on 2026-10-19):

```sql
-- run this from any mysql client
SHOW CREATE TABLE kanpad_rs_dev.sessions;
-- ✓ expect: (id CHAR(22), data BLOB, expiry_date TIMESTAMP(6), KEY IDX_session_expiry_date)
```

If `sessions` doesn't exist, or `migrate info` reports a row named
`tower_sessions`/an old schema — your working tree predates the 2026-10-19 change
(or the DB was migrated by it). Re-clone / drop-and-re-migrate (Troubleshooting #1).

---

## 4. Import the data (the actual migration)

Open a `mysql` client against the staging database and paste/execute the SQL below.
It copies every app table from `kanpad_nest_restore` into `kanpad_rs_dev`,
preserving IDs and timestamps. The two "ghost columns" Nest had but Rust doesn't
want (`users.username`, `projects.description`) are simply omitted.

```sql
-- connect: $MYSQL kanpad_rs_dev   (runs in the staging DB; cross-DB queries name kanpad_nest_restore)
SET FOREIGN_KEY_CHECKS=0;
SET time_zone='+00:00';
SET NAMES utf8mb4;

INSERT INTO users (id, email, password, role, created_at, updated_at)
SELECT id, email, password, role, created_at, updated_at FROM kanpad_nest_restore.users;

INSERT INTO projects (id, name, user_id, created_at, updated_at)
SELECT id, name, user_id, created_at, updated_at FROM kanpad_nest_restore.projects;

INSERT INTO boards (id, name, background_color, user_id, project_id, is_archived, view_mode, created_at, updated_at)
SELECT id, name, background_color, user_id, project_id, is_archived, view_mode, created_at, updated_at
FROM kanpad_nest_restore.boards;

INSERT INTO `columns` (id, name, position, board_id, created_at, updated_at)
SELECT id, name, position, board_id, created_at, updated_at FROM kanpad_nest_restore.`columns`;

INSERT INTO cards (id, title, column_id, position, description, due_date, created_at, updated_at)
SELECT id, title, column_id, position, description, due_date, created_at, updated_at
FROM kanpad_nest_restore.cards;

INSERT INTO labels (id, name, color, user_id, created_at, updated_at)
SELECT id, name, color, user_id, created_at, updated_at FROM kanpad_nest_restore.labels;

INSERT INTO card_labels (card_id, label_id)
SELECT card_id, label_id FROM kanpad_nest_restore.card_labels;

INSERT INTO checklists (id, title, card_id, created_at, updated_at)
SELECT id, title, card_id, created_at, updated_at FROM kanpad_nest_restore.checklists;

INSERT INTO checklist_items (id, text, is_completed, checklist_id, position, created_at, updated_at)
SELECT id, text, is_completed, checklist_id, position, created_at, updated_at
FROM kanpad_nest_restore.checklist_items;

INSERT INTO notes (id, title, content, board_id, project_id, card_id, user_id, created_at, updated_at)
SELECT id, title, content, board_id, project_id, card_id, user_id, created_at, updated_at
FROM kanpad_nest_restore.notes;

INSERT INTO tags (id, name, color, user_id, created_at)
SELECT id, name, color, user_id, created_at FROM kanpad_nest_restore.tags;

INSERT INTO note_tags (note_id, tag_id)
SELECT note_id, tag_id FROM kanpad_nest_restore.note_tags;

SET FOREIGN_KEY_CHECKS=1;
```

✓ success = **no output and no errors**. A validation error here (`CHECK`, `1062`
duplicate, `1406` too long) means prod data violates a Rust schema rule — see
Troubleshooting #3/#4 before improvising.

> **Sessions are intentionally NOT imported.** Nest's `session` table has a
> different shape and cookie codec; every user logs in again after cutover. The old
> `typeorm_migrations` journal is likewise abandoned (Rust uses `_sqlx_migrations`).

---

## 5. Verify the data landed intact

Four checks. **All of them must be green before you touch cutover.**

> **Run these steps BEFORE step 7 (the label seed).** The seed adds rows to
> `labels`, which would break the row-count parity gate in (a). Steps 5–6 compare
> the import *as it came out of step 4*.

**(a) Row counts match the source — every table, all at once:**

```sql
-- every table is named explicitly, so this runs from ANY database
SELECT
  (SELECT COUNT(*) FROM kanpad_nest_restore.users)=(SELECT COUNT(*) FROM kanpad_rs_dev.users) AS users,
  (SELECT COUNT(*) FROM kanpad_nest_restore.projects)=(SELECT COUNT(*) FROM kanpad_rs_dev.projects) AS projects,
  (SELECT COUNT(*) FROM kanpad_nest_restore.boards)=(SELECT COUNT(*) FROM kanpad_rs_dev.boards) AS boards,
  (SELECT COUNT(*) FROM kanpad_nest_restore.`columns`)=(SELECT COUNT(*) FROM kanpad_rs_dev.`columns`) AS cols,
  (SELECT COUNT(*) FROM kanpad_nest_restore.cards)=(SELECT COUNT(*) FROM kanpad_rs_dev.cards) AS cards,
  (SELECT COUNT(*) FROM kanpad_nest_restore.labels)=(SELECT COUNT(*) FROM kanpad_rs_dev.labels) AS labels,
  (SELECT COUNT(*) FROM kanpad_nest_restore.card_labels)=(SELECT COUNT(*) FROM kanpad_rs_dev.card_labels) AS card_labels,
  (SELECT COUNT(*) FROM kanpad_nest_restore.checklists)=(SELECT COUNT(*) FROM kanpad_rs_dev.checklists) AS checklists,
  (SELECT COUNT(*) FROM kanpad_nest_restore.checklist_items)=(SELECT COUNT(*) FROM kanpad_rs_dev.checklist_items) AS checklist_items,
  (SELECT COUNT(*) FROM kanpad_nest_restore.notes)=(SELECT COUNT(*) FROM kanpad_rs_dev.notes) AS notes,
  (SELECT COUNT(*) FROM kanpad_nest_restore.tags)=(SELECT COUNT(*) FROM kanpad_rs_dev.tags) AS tags,
  (SELECT COUNT(*) FROM kanpad_nest_restore.note_tags)=(SELECT COUNT(*) FROM kanpad_rs_dev.note_tags) AS note_tags;
```
✓ every column must read `1`.

**(b) No orphan rows** — one query per FK. Run them all; every one must return `0`:

```sql
-- connect to kanpad_rs_dev
SELECT 'projects->users' AS fk, COUNT(*) AS orphans FROM kanpad_rs_dev.projects p
  LEFT JOIN kanpad_rs_dev.users u ON u.id=p.user_id WHERE u.id IS NULL
UNION ALL SELECT 'boards->users', COUNT(*) FROM kanpad_rs_dev.boards b
  LEFT JOIN kanpad_rs_dev.users u ON u.id=b.user_id WHERE u.id IS NULL
UNION ALL SELECT 'boards->projects', COUNT(*) FROM kanpad_rs_dev.boards b
  LEFT JOIN kanpad_rs_dev.projects p ON p.id=b.project_id
  WHERE b.project_id IS NOT NULL AND p.id IS NULL
UNION ALL SELECT 'columns->boards', COUNT(*) FROM kanpad_rs_dev.`columns` c
  LEFT JOIN kanpad_rs_dev.boards b ON b.id=c.board_id WHERE b.id IS NULL
UNION ALL SELECT 'cards->columns', COUNT(*) FROM kanpad_rs_dev.cards c
  LEFT JOIN kanpad_rs_dev.`columns` cl ON cl.id=c.column_id WHERE cl.id IS NULL
UNION ALL SELECT 'labels->users', COUNT(*) FROM kanpad_rs_dev.labels l
  LEFT JOIN kanpad_rs_dev.users u ON u.id=l.user_id WHERE u.id IS NULL
UNION ALL SELECT 'card_labels->cards', COUNT(*) FROM kanpad_rs_dev.card_labels cl
  LEFT JOIN kanpad_rs_dev.cards c ON c.id=cl.card_id WHERE c.id IS NULL
UNION ALL SELECT 'card_labels->labels', COUNT(*) FROM kanpad_rs_dev.card_labels cl
  LEFT JOIN kanpad_rs_dev.labels l ON l.id=cl.label_id WHERE l.id IS NULL
UNION ALL SELECT 'checklists->cards', COUNT(*) FROM kanpad_rs_dev.checklists ch
  LEFT JOIN kanpad_rs_dev.cards c ON c.id=ch.card_id WHERE c.id IS NULL
UNION ALL SELECT 'checklist_items->checklists', COUNT(*) FROM kanpad_rs_dev.checklist_items ci
  LEFT JOIN kanpad_rs_dev.checklists ch ON ch.id=ci.checklist_id WHERE ch.id IS NULL
UNION ALL SELECT 'notes->users', COUNT(*) FROM kanpad_rs_dev.notes n
  LEFT JOIN kanpad_rs_dev.users u ON u.id=n.user_id WHERE u.id IS NULL
UNION ALL SELECT 'notes->boards', COUNT(*) FROM kanpad_rs_dev.notes n
  LEFT JOIN kanpad_rs_dev.boards b ON b.id=n.board_id
  WHERE n.board_id IS NOT NULL AND b.id IS NULL
UNION ALL SELECT 'notes->projects', COUNT(*) FROM kanpad_rs_dev.notes n
  LEFT JOIN kanpad_rs_dev.projects p ON p.id=n.project_id
  WHERE n.project_id IS NOT NULL AND p.id IS NULL
UNION ALL SELECT 'notes->cards', COUNT(*) FROM kanpad_rs_dev.notes n
  LEFT JOIN kanpad_rs_dev.cards c ON c.id=n.card_id
  WHERE n.card_id IS NOT NULL AND c.id IS NULL
UNION ALL SELECT 'note_tags->notes', COUNT(*) FROM kanpad_rs_dev.note_tags nt
  LEFT JOIN kanpad_rs_dev.notes n ON n.id=nt.note_id WHERE n.id IS NULL
UNION ALL SELECT 'note_tags->tags', COUNT(*) FROM kanpad_rs_dev.note_tags nt
  LEFT JOIN kanpad_rs_dev.tags t ON t.id=nt.tag_id WHERE t.id IS NULL;
```
✓ 16 rows, every `orphans` = 0.

**(c) AUTO_INCREMENT counters are past the max imported ID** (so the next Rust
`INSERT` can't collide with an imported row):

```sql
-- connect to kanpad_rs_dev
-- ⚠ the stats-expiry override is REQUIRED: MySQL 8.0 caches
--   information_schema.TABLES for up to `information_schema_stats_expiry`
--   seconds (default 86400 = 24 h), so AUTO_INCREMENT can read STALE —
--   e.g. reporting 22 while the real counter is already 29. Reading a stale
--   value makes this gate fail on a perfectly healthy database.
SET SESSION information_schema_stats_expiry = 0;

SELECT t.table_name, t.auto_increment, COALESCE(m.maxid,0)+1 AS expected_next,
       (t.auto_increment >= COALESCE(m.maxid,0)+1) AS collision_safe
FROM information_schema.TABLES t
JOIN (SELECT 'users' tn, MAX(id) maxid FROM users UNION ALL
      SELECT 'projects', MAX(id) FROM projects UNION ALL
      SELECT 'boards',   MAX(id) FROM boards UNION ALL
      SELECT 'columns',  MAX(id) FROM `columns` UNION ALL
      SELECT 'cards',    MAX(id) FROM cards UNION ALL
      SELECT 'labels',   MAX(id) FROM labels UNION ALL
      SELECT 'checklists', MAX(id) FROM checklists UNION ALL
      SELECT 'checklist_items', MAX(id) FROM checklist_items UNION ALL
      SELECT 'notes',    MAX(id) FROM notes UNION ALL
      SELECT 'tags',     MAX(id) FROM tags) m ON m.tn = t.table_name
WHERE t.table_schema='kanpad_rs_dev';
```
✓ every row's `collision_safe` must read `1`.

> **Why `>=` and not `=`?** Inserting explicit IDs raises the InnoDB counter to
> exactly `max(id)+1` on its own (MySQL 8.0, MariaDB 10.6 — so the old manual
> `ALTER TABLE … AUTO_INCREMENT` step is unnecessary). But a counter *above*
> `max(id)+1` is equally safe: it only skips a few IDs. The counter can legitimately
> end up higher when a **multi-row insert with auto-generated IDs** runs, because
> `innodb_autoinc_lock_mode=2` (the default) reserves a *block* of ID values per
> statement. That is exactly what step 7's label seed does — so `labels` is
> expected to read e.g. `29` while `expected_next` is `28`. Only a counter *below*
> `max(id)+1` is a real defect (the next insert would collide with an imported
> row).
>
> **If `collision_safe` reads 0, confirm it is not a stale-cache artifact**
> before touching anything: re-run the query with the `stats_expiry` override
> above (or run `ANALYZE TABLE kanpad_rs_dev.labels;` first). A stale read can
> report a counter *below* the truth and make a healthy table look broken.

**(d) Schema drift gate — the database must still be exactly what the migrations
build.** This is the gate that proves the schema the Rust app will run against is
the schema in `0001_baseline.sql` — and it is what catches a hand-written `ALTER`
(Troubleshooting #3/#4) that quietly diverged from the baseline.

Build a *reference* database from the same migrations and compare the two
**semantically** (index names + their columns + constraint names), not by raw
text: column and index *order* legitimately differs between two builds, and
`AUTO_INCREMENT=` counters differ by design.

```bash
# Run from the repository root (step 3 leaves you inside backend-rs/).
ROOT="$(git rev-parse --show-toplevel)"

# Reference DB, built by the same migrations from the same working tree.
$MYSQL -e "DROP DATABASE IF EXISTS kanpad_rs_ref;
           CREATE DATABASE kanpad_rs_ref \
           CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

cd "$ROOT/backend-rs"
# DB_NAME on the command line overrides backend-rs/.env (real env beats .env —
# dotenvy never overrides an already-set variable); every other DB_* key still
# comes from .env, so no password ever reaches the command line.
DB_NAME=kanpad_rs_ref cargo run migrate run      # ✓ prints:  migrations applied: 2
cd "$ROOT"
```

```sql
-- Both directions: anything in kanpad_rs_dev that is missing from the reference,
-- and anything in the reference that is missing from kanpad_rs_dev.
-- Runs from ANY database (both schemas are named explicitly).
SELECT 'dev-only index' AS drift, a.table_name, a.index_name AS name,
       a.seq_in_index AS seq, a.column_name AS col
FROM information_schema.statistics a
LEFT JOIN information_schema.statistics b
  ON b.table_schema='kanpad_rs_ref' AND b.table_name=a.table_name
 AND b.index_name=a.index_name AND b.seq_in_index=a.seq_in_index
WHERE a.table_schema='kanpad_rs_dev'
  AND (b.index_name IS NULL OR b.column_name <> a.column_name)
UNION ALL
SELECT 'ref-only index', b.table_name, b.index_name, b.seq_in_index, b.column_name
FROM information_schema.statistics b
LEFT JOIN information_schema.statistics a
  ON a.table_schema='kanpad_rs_dev' AND a.table_name=b.table_name
 AND a.index_name=b.index_name AND a.seq_in_index=b.seq_in_index
WHERE b.table_schema='kanpad_rs_ref'
  AND (a.index_name IS NULL OR a.column_name <> b.column_name)
UNION ALL
SELECT 'dev-only constraint', a.table_name, a.constraint_name, NULL, NULL
FROM information_schema.table_constraints a
LEFT JOIN information_schema.table_constraints b
  ON b.table_schema='kanpad_rs_ref' AND b.table_name=a.table_name
 AND b.constraint_name=a.constraint_name
WHERE a.table_schema='kanpad_rs_dev' AND b.constraint_name IS NULL
UNION ALL
SELECT 'ref-only constraint', b.table_name, b.constraint_name, NULL, NULL
FROM information_schema.table_constraints b
LEFT JOIN information_schema.table_constraints a
  ON a.table_schema='kanpad_rs_dev' AND a.table_name=b.table_name
 AND a.constraint_name=b.constraint_name
WHERE b.table_schema='kanpad_rs_ref' AND a.constraint_name IS NULL;
```
✓ **0 rows.** (Verified non-vacuous: adding a stray `ALTER TABLE cards ADD INDEX
IDX_DRIFT_PROBE (title)` makes the gate report exactly that row, and removing it
returns the gate to 0 rows.)

And the charset / row-format half of the same guarantee:

```sql
-- every table must be utf8mb4_unicode_ci and DYNAMIC
-- (utf8mb4 makes users.email varchar(255) UNIQUE = 1020 bytes, which only fits
--  DYNAMIC/COMPRESSED's 3072-byte index limit, not COMPACT's 767)
SELECT table_name, table_collation, row_format
FROM information_schema.tables WHERE table_schema='kanpad_rs_dev' ORDER BY table_name;
-- ✓ every row: utf8mb4_unicode_ci + Dynamic
```
✓ 14 rows (12 app tables + `sessions` + `_sqlx_migrations`).

> This gate is trivially green right after step 3 — the staging DB *is* the
> migrations' output. Its value is after any hand-written `ALTER` (Troubleshooting
> #3/#4), and it is the gate to re-run against the **promoted production**
> database before unfreezing writers. The reference DB is data-independent, so a
> re-import (step 9, Option A) does not invalidate it — rebuild it only if
> `0001_baseline.sql` changed. Drop `kanpad_rs_ref` in step 10.

---

## 6. Verify text fidelity (the latin1 → utf8mb4 conversion)

The staging DB is utf8mb4; the prod (source) tables were latin1. Two facts
make the checks here trustworthy:

1. MySQL's `latin1` is cp1252-flavoured, so legacy single-byte smart quotes migrate
   to their *correct* Unicode chars. Live example from prod_04: card 67 contained
   the byte `0x92` ("But it\x92s") in the source; after migration the staging row
   stores the true curly quote (`’` = UTF-8 sequence `E2 80 99`). The **text, not
   the raw bytes, is what changed** — and it changed for the better.
2. Compare with a *charset-aware text equality*, not raw `HEX()` byte equality —
   a byte-exact comparison would false-fail on the re-encoded rows above.

```sql
-- Master gate: every text field compares as EQUAL TEXT across the two DBs.
-- Run per table, one line each; every result must be 0.
-- ⚠ connect to kanpad_rs_dev — the first table name is unqualified.
SELECT SUM(a.title<>b.title OR a.description<>b.description) FROM cards a JOIN kanpad_nest_restore.cards b USING(id);   -- ✓ 0
SELECT SUM(a.title<>b.title OR a.content<>b.content) FROM notes a JOIN kanpad_nest_restore.notes b USING(id);           -- ✓ 0
SELECT SUM(a.name<>b.name) FROM boards a JOIN kanpad_nest_restore.boards b USING(id);                                   -- ✓ 0
SELECT SUM(a.name<>b.name) FROM projects a JOIN kanpad_nest_restore.projects b USING(id);                               -- ✓ 0
SELECT SUM(a.name<>b.name) FROM labels a JOIN kanpad_nest_restore.labels b USING(id);                                   -- ✓ 0
SELECT SUM(a.title<>b.title) FROM checklists a JOIN kanpad_nest_restore.checklists b USING(id);                         -- ✓ 0
SELECT SUM(a.text<>b.text) FROM checklist_items a JOIN kanpad_nest_restore.checklist_items b USING(id);                 -- ✓ 0
SELECT SUM(a.name<>b.name) FROM tags a JOIN kanpad_nest_restore.tags b USING(id);                                       -- ✓ 0
```

Two additional hex gates for anything the text check can't see (emoji are inserted
as multibyte sequences — they don't round-trip in the latin1 scratch DB):

```sql
-- emoji present in the migrated data?
SELECT COUNT(*) FROM kanpad_rs_dev.notes
WHERE HEX(CONCAT(title, IFNULL(content,''))) LIKE '%F09F%';     -- ✓ expect 0 (prod has none as of 2026-10-18)

-- curly apostrophe properly encoded as E28099?
SELECT COUNT(*) FROM kanpad_rs_dev.cards
WHERE HEX(description) LIKE '%E28099%';                        -- ✓ ≥ 1 (card 67's ’)
```

> **Known, pre-existing content damage (do not try to fix here).** Four note
> bodies contain literal `?` where arrows/emoji once were (e.g. "To Do ? Doing ?
> Done"). That loss happened years ago in prod under the latin1 regime. The
> migration preserves it exactly as-is; the migration did NOT introduce it.

---

## 7. Seed the six default labels for users who lack them

Rust creates 6 default labels for every *newly registered* user. Imported users
don't get them automatically — run this once (it is idempotent, safe to re-run):

```sql
-- connect to kanpad_rs_dev
INSERT INTO labels (name, color, user_id, created_at, updated_at)
SELECT d.name, d.color, u.id, NOW(6), NOW(6)
FROM kanpad_rs_dev.users u
CROSS JOIN (
  SELECT 'Urgent' AS name, 'red' AS color
  UNION ALL SELECT 'Important', 'orange'
  UNION ALL SELECT 'In Progress', 'yellow'
  UNION ALL SELECT 'Done', 'green'
  UNION ALL SELECT 'Bug', 'blue'
  UNION ALL SELECT 'Feature', 'purple'
) d
WHERE NOT EXISTS (
  SELECT 1 FROM kanpad_rs_dev.labels l
  WHERE l.user_id = u.id AND l.name = d.name
);
```

✓ no output, no errors.

Now re-check the `labels` table with the **seed-aware** gates — the plain count
parity of step 5(a) no longer applies here, because this step adds rows:

```sql
-- connect to kanpad_rs_dev

-- (A) no source label was lost or renamed by the seed
SELECT COUNT(*) AS source_labels_lost
FROM kanpad_nest_restore.labels s
  LEFT JOIN kanpad_rs_dev.labels l ON l.user_id = s.user_id AND l.name = s.name
WHERE l.id IS NULL;
-- ✓ 0

-- (B) the target gained ONLY the six defaults — nothing else
SELECT COUNT(*) AS unexpected_labels
FROM kanpad_rs_dev.labels l
  LEFT JOIN kanpad_nest_restore.labels s ON s.user_id = l.user_id AND s.name = l.name
WHERE s.id IS NULL
  AND l.name NOT IN ('Urgent','Important','In Progress','Done','Bug','Feature');
-- ✓ 0

-- (C) every user now has all six defaults
SELECT u.id, u.email,
  (SELECT COUNT(*) FROM kanpad_rs_dev.labels l
    WHERE l.user_id = u.id
      AND l.name IN ('Urgent','Important','In Progress','Done','Bug','Feature')) AS defaults_present
FROM kanpad_rs_dev.users u ORDER BY u.id;
-- ✓ every row reads 6

-- (D) reconciliation: how many rows did the seed add?
SELECT (SELECT COUNT(*) FROM kanpad_rs_dev.labels) AS target,
       (SELECT COUNT(*) FROM kanpad_nest_restore.labels) AS source,
       (SELECT COUNT(*) FROM kanpad_rs_dev.labels)
     - (SELECT COUNT(*) FROM kanpad_nest_restore.labels) AS added;
-- ✓ target = source + added; on prod_04: 27 = 21 + 6 (user 2 had no labels)
```

> **A pre-existing prod label may shadow a default name.** In prod_04, user 1
> already has a label named **"Bug" with colour `red`** (the default colour for
> "Bug" is `blue`). The seed matches on **name only**, so it skips that row and
> the user keeps their red "Bug" while user 2 gets a blue "Bug". That is the same
> name-based semantics Rust's own `ensure_default_labels` uses (`INSERT IGNORE`
> against `UQ_labels_user_name`) — it is not a migration defect, but don't let it
> surprise you during the check.

---

## 8. Boot the Rust backend and test it by hand

```bash
cd backend-rs
cargo run serve
# ✓ first log line:  backend-rs listening  port=3001
```

```bash
curl -s -i http://127.0.0.1:3001/health      # ✓ 200 {"status":"ok"}
curl -s -i http://127.0.0.1:3001/ready       # ✓ 200 {"db":"up","status":"ok"}
# ✓ every response carries:  served-by: rust
```

Log in with a real prod account (this also *upgrades* its password hash to
Argon2id — after this, the user's prod password still works, the hash just changes
shape):

```bash
curl -s -i -X POST http://127.0.0.1:3001/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -c /tmp/kanpad-cookies.txt \
  -d '{"email":"<your prod email>","password":"<your prod password>"}'
# ✓ 200 with {"data":{"id":…,"email":…,"role":…}}
# ✓ set-cookie: kanpad.sid=…; HttpOnly; SameSite=Lax; Path=/; Max-Age=86400
```

> **The cookie is `kanpad.sid`, not `__Host-kanpad.sid`,** as long as
> `APP_ENV=development` (the staging setting in step 0). The `__Host-` prefix is
> used only when `APP_ENV=production` — and it must be, because a `__Host-`
> cookie is required to carry `Secure`, which browsers only accept over HTTPS.
> Staging serves plain HTTP, so the dev cookie name is the correct one.
>
> **The service refuses to start if `NODE_ENV` is set.** `APP_ENV` replaced the
> Node.js `NODE_ENV` (T54), and boot **fails** rather than silently defaulting
> to `development` — a stale export would otherwise drop `Secure`, the
> `__Host-` prefix and the prod CORS default. If the service refuses to boot
> with *"NODE_ENV is not used by backend-rs"*, unset `NODE_ENV` and set
> `APP_ENV` instead.

Signed in, spot-check the migrated data (use the cookie jar you just saved):

```bash
curl -s -b /tmp/kanpad-cookies.txt http://127.0.0.1:3001/api/v1/boards      # ✓ imported boards listed
curl -s -b /tmp/kanpad-cookies.txt http://127.0.0.1:3001/api/v1/boards/1    # ✓ columns nested, position ASC
```

The cards of one column are listed under the **column** route:

```bash
COL=1   # any column id from the board detail above
curl -s -b /tmp/kanpad-cookies.txt "http://127.0.0.1:3001/api/v1/columns/$COL/cards"
# ✓ cards listed; each item carries "has_description" and does NOT carry "description"
```

> ⚠ `GET /api/v1/cards?column_id=1` is **not** a valid route — there is no GET on
> `/cards` (it is POST-only for card creation), so it answers **405 Method Not
> Allowed**. The list route is `/api/v1/columns/{colId}/cards`.

Each user only sees their own data (this is the ownership scope, not data loss) —
on prod_04, user 1 sees 9 of the 10 boards and 2 of the 3 projects, user 2 sees
the remaining board and project.

How the login worked without converting passwords: Nest stored bcrypt (`$2b$…`).
Rust tries Argon2 first, falls back to bcrypt, and on a bcrypt hit **re-hashes and
updates the row in place** — no batch conversion needed. Healthy after step 8:

```sql
SELECT id, LEFT(password,15) FROM kanpad_rs_dev.users;
-- ✓ logged-in user's hash now begins with $argon2id$v=19$; the rest still $2b$ until they log in
```

Stop the test server when done (`Ctrl+C`).

---

## 9. Cutover

### Option A — Blue-green, controlled window (RECOMMENDED)

1. Freeze writers on prod (app maintenance notice / read-only mode).
2. Re-dump prod **now** with step 1's command (data may have changed since staging import) and redo steps 2–8 against the fresh dump. With a small window you typically only redo step 2 replay + 4–7.
3. Promote the staging DB to production (rename / repoint the connection).
4. Start the Rust backend against it and build the frontend pointed at it (`VITE_API_URL` unset defaults to the Rust `:3001` proxy after T51; see `docs/cutover.md`).
5. Confirm `served-by: rust` + log into the app in the browser.
6. Unfreeze writers. **Users will be logged out once** — that's the intended behavior (session cookies were abandoned).
7. Keep the Nest image + the pre-cutover `.gz` backup until the rollback window closes.

### Option B — One-line app rollback

Point the frontend at the Nest backend again:

```bash
VITE_API_URL=http://localhost:3000
```

Nothing about the databases changed; Nest still runs on the untouched prod DB.

### Option C — Database rollback

Restore from `kanpad-prod-pre-cutover.sql.gz` only. **Never `sqlx migrate revert`
against production.** A restore drill belongs in `docs/cutover.md`'s log.

---

## 10. Cleanup

```bash
$MYSQL -e "DROP DATABASE kanpad_nest_restore;
           DROP DATABASE kanpad_rs_ref;"     # kanpad_rs_ref = the step 5(d) reference DB
# keep: the .gz pre-cutover backup, off-host; the Nest image until rollback window closes
```

---

## Troubleshooting

| # | Symptom | Cause & fix |
|---|---|---|
| 1 | `migrate run` dies with `VersionMismatch` on an existing DB, or `migrate info` grows `pending` without applying | Your working tree is older than `d71178c5` or the DB was migrated by pre-2026-10-19 code; the file bytes changed since. Drop & re-create the staging DB from this runbook's step 3. Fix on disk: never hand-edit `_sqlx_migrations` — the checksums are SHA-384s of the file bytes. |
| 2 | Restore errors mentioning "sandbox mode" (`/*M!999999…*`) | The prod dump was taken by MariaDB's mysqldump; it blocks very old clients. MySQL 8.x and MariaDB ≥ 10.6 restore it fine (verified on MySQL 8.0.46). Older MariaDB: strip line 1 (` gunzip -c "$DUMP" \| sed '1{/sandbox/d}' \| mysql …`). |
| 3 | Import fails with CHECK constraint on `view_mode`/`color`/`role` | A prod value falls outside the Rust-allowed enum. Inspect: `SELECT DISTINCT view_mode FROM kanpad_nest_restore.boards;` etc. Fix the data (or bump the Rust enum) **before** promoting — don't fail silently at cutover. |
| 4 | Import fails with 1062 duplicate on `labels` | SQL collation makes `UQ_labels_user_name` case-insensitive: two prod rows like "Work" vs "work" for the same user collide. Rename one of the dupe pair in the scratch DB and re-run the labels insert. |
| 5 | Import fails with FK errors | The provided SQL block disables FK checks at its start. If you're running pieces one by one, wrap each in its own `SET FOREIGN_KEY_CHECKS=0; … ; =1;` pair. |
| 6 | `cargo run serve` gives 401 on every API call | That's correct pre-login. First `POST /api/v1/auth/login`, send back the session cookie it sets (`-b /tmp/kanpad-cookies.txt`), then the API opens up. |
| 7 | Timeouts importing | Run statements one by one (per table) instead of the whole block in a single interactive paste. |
| 8 | A step-5/6 query fails with `Table 'cards' doesn't exist` | Those queries use unqualified table names and must be run **connected to `kanpad_rs_dev`** (`$MYSQL kanpad_rs_dev`). |
| 9 | Step 5(a) shows `labels` = 0 instead of 1 | You ran the label seed (step 7) *before* the parity gate. Re-run steps 5–6 on a fresh import, or use the seed-aware gates in step 7. |
| 10 | Step 5(c) shows `collision_safe` = 0 for some table | Either the AUTO_INCREMENT counter really is *below* `max(id)+1` (the next insert would collide with an imported row — fix it with `ALTER TABLE kanpad_rs_dev.<table> AUTO_INCREMENT = <max(id)+1>;`), or you read a **stale** `information_schema` cache. Re-run the query with `SET SESSION information_schema_stats_expiry = 0;` (or `ANALYZE TABLE` first) before concluding anything. A counter that is merely *higher* is fine — see the note in step 5(c). |
| 11 | `migrate run` dies with `1050 (42S01): Table '…' already exists` | You ran it on a **populated** database. `0001` has no `IF NOT EXISTS` by design and sqlx 0.8.6 has no stamp command, so the Rust schema can only be built on an empty DB. Drop and re-create the staging DB from step 3 — and clear the dirty journal row first (see #12). |
| 12 | `error: migration 1 is partially applied; fix and remove row from '_sqlx_migrations'` | A previous `migrate run` failed mid-flight and left `success=0` behind (sqlx records the row *before* running it). `DELETE FROM _sqlx_migrations WHERE success = 0;` — then fix the real cause before re-running. Never hand-edit a `success=1` row: sqlx checksums are SHA-384s over the migration file's bytes. |
| 13 | `1553 Cannot drop index 'X': needed in a foreign key constraint`, or `1826 Duplicate foreign key constraint name 'X'` | Rules for reshaping indexes by hand: (a) never `DROP INDEX` an index that backs a FK — drop the FK first, in its **own** statement; (b) never `DROP FOREIGN KEY x` and `ADD CONSTRAINT x` in the *same* ALTER — same-name duplicate (1826); (c) to rename an index use `ALTER TABLE t RENAME INDEX old TO new` — it works on FK-backing indexes (MariaDB ≥ 10.5.2, MySQL ≥ 5.7) and the FK follows the rename and stays enforced. |

---

## 11. What was verified in the last live test (2026-10-21)

- Dump used: `raziurco_kanbanflow_prod_04_full-2026-10-18.sql.gz` (MariaDB 10.6.28
  origin, 21 KB gz). Restored into a fresh `kanpad_nest_restore` (MySQL 8.0.46
  accepted the MariaDB sandbox line without error — Troubleshooting #2).
- Scratch DB: **14** tables, every one `latin1_swedish_ci`; row counts 2 users,
  3 projects, 10 boards, 34 columns, 109 cards, 21 labels, 85 card_labels,
  9 checklists, 29 checklist_items, 13 notes, 1 tag, 1 note_tag.
- Staging: fresh `kanpad_rs_dev` → `migrate run` printed `migrations applied: 2`,
  `migrate info` printed `applied: 2 pending: 0`, `sessions` shape exactly as
  documented (`char(22)` / `blob` / `timestamp(6)` + `IDX_session_expiry_date`).
- Import: the step-4 block ran with **no output and no errors**; `sessions` still
  empty afterwards.
- Gates: **12/12** count-parity = 1 (pre-seed, as step 5 now requires); **16/16**
  orphan-FK checks = 0 (13 documented + the 3 extra `notes→boards|projects|cards`
  now listed); AUTO_INCREMENT **10/10 `collision_safe`**; text-equality **8/8** = 0;
  emoji gate = 0; `E28099` gate = 1.
- **A stale-cache trap in step 5(c).** MySQL 8.0 caches `information_schema.TABLES`
  for `information_schema_stats_expiry` seconds (default 86400 = 24 h). Right after
  the label seed, `AUTO_INCREMENT` for `labels` read **22** — while `MAX(id)` was
  27 and the *real* counter was 29. The gate read `collision_safe = 0` on a
  perfectly healthy table. `ANALYZE TABLE` (or the `stats_expiry = 0` override now
  in the gate) returned the truth, and a probe insert was assigned id 29 — no
  collision. **Always read the counter with the override in place.**
- Byte-level: card 67 was the raw latin1 byte `0x92` in the source and the correct
  `E28099` (U+2019) in the target; the four note bodies containing a literal `?`
  are byte-identical in source and target (pre-existing damage preserved, not
  introduced).
- Ghost columns confirmed absent: `users.username` and `projects.description` are
  not columns of the Rust schema.
- Step 8: `cargo run serve` logged `backend-rs listening … port=3001`; `/health`
  and `/ready` returned 200 with `served-by: rust`; an unauthenticated
  `/api/v1/boards` returned 401. Login returned 200 `{"data":{…}}` with a
  `kanpad.sid` cookie, and the logged-in user's bcrypt hash was **upgraded in
  place to `$argon2id$v=19$`** while the other user stayed `$2b$10$`. The session
  row landed in `kanpad_rs_dev.sessions` (the app DB), confirming the 2026-10-19
  store change. `/api/v1/boards` listed the imported boards and `/api/v1/boards/1`
  returned its columns nested in `position` ASC; `/api/v1/columns/1/cards`
  returned 42 cards carrying `has_description` and no `description` key.
  (`GET /api/v1/cards?column_id=1` returns 405 — there is no GET on `/cards`.)
- The label seed added exactly 6 rows (21 → 27: user 1 already had all six
  default *names*, user 2 had none); the four seed-aware gates in step 7 are all
  green. Note prod user 1's pre-existing "Bug" label is **red**, not the default
  blue — the seed matches on name only, by design.
- Not verified there: a production MariaDB 10.6 *staging* proof of step 6 with real
  prod data (the T02b staging proof). Do that gate once on MariaDB 10.6 before
  cutover — local dev's 8.0 engine behaves the same for `CONVERT TO`, but the
  cutover box should be the real engine.
- **Still a manual check:** step 8's login with the *real* prod password. The run
  above proved the mechanism (bcrypt verify → in-place Argon2id upgrade) using a
  locally generated bcrypt hash standing in for the real credential. Run it with
  the actual prod password during your window.

### Addendum (2026-10-21) — migration-path comparison + the new gates

A second live run (fresh MySQL 8.4 container, same prod_04 dump) compared three
candidate ways to get the data across, and produced the evidence behind the
runbook's current shape.

- **The dump file is already UTF-8.** Byte scan of the decompressed dump: valid
  UTF-8, one `U+2019` as `E2 80 99`, **zero** raw `0x92` bytes — `mysqldump`
  transcoded latin1→utf8mb4 when it read the prod tables. Restoring the same
  file into a latin1 DB re-narrows it to cp1252 bytes (a byte-exact prod
  replica); restoring it into a utf8mb4 DB yields **identical text** (0 diffs on
  every text column of every table). Hence the scratch DB is a *reference*, and
  the conversion itself happens in step 4's `INSERT … SELECT`.
- **Three paths, one result.** (A) scratch latin1 → fresh migrated DB →
  `INSERT … SELECT`; (B) restore → convert → 13-statement in-place `ALTER` delta
  → hand-stamped journal; (C) restore latin1 → `CONVERT TO utf8mb4`. **All three
  produce identical text on all 12 app tables** (0 diffs) and identical row
  counts. Path B's *schema* also matches the migration-built schema on all 13
  tables — except column/index *order* and the `AUTO_INCREMENT` counters.
- **Path B was NOT adopted.** It needs a hand-written delta script that must be
  re-verified whenever `0001_baseline.sql` changes (a first draft of it shipped a
  renamed-but-still-single-column index — the schema diff caught it, but that is
  exactly the risk), plus a hand-inserted `_sqlx_migrations` journal. The
  checksum is computable (sqlx 0.8.6 = SHA-384 over the migration file's bytes;
  stamping both rows made `migrate info` report `applied: 2` and `migrate run` a
  clean no-op), but a stamp rots the moment a migration file is touched. The
  baseline-built schema has no such tail.
- **`sqlx migrate run` on a populated DB fails with `1050 (42S01): Table
  'sessions' already exists`**, and the failed run leaves a **dirty** journal row
  (`success=0`) that makes every later run die with `migration 1 is partially
  applied`. Reproduced with the real `sqlx` CLI 0.8.6; now documented in step 3
  and Troubleshooting #11/#12.
- **DDL rules learned the hard way** (now Troubleshooting #13): `DROP INDEX` on
  an FK-backing index → `1553`; `DROP FOREIGN KEY x` + `ADD CONSTRAINT x` in one
  ALTER → `1826`; `ALTER TABLE … RENAME INDEX old TO new` works on FK-backing
  indexes (MariaDB ≥ 10.5.2, MySQL ≥ 5.7), the FK follows the rename and stays
  enforced (verified).
- **New gate 5(d) — schema drift.** Built a `kanpad_rs_ref` reference DB from
  the same migrations (`DB_NAME=kanpad_rs_ref cargo run migrate run` — the env
  override beats `.env`, no password on the command line) and compared indexes +
  constraints in both directions: **0 rows**. Negative-tested: an injected
  `ALTER TABLE cards ADD INDEX IDX_DRIFT_PROBE (title)` is reported as
  `dev-only index`, and removing it returns the gate to 0 rows. The
  charset/row-format half reports 14 rows, every one `utf8mb4_unicode_ci` +
  `Dynamic`.
- **This rebuild re-confirmed the existing gates**: `migrate run` →
  `migrations applied: 2`, `migrate info` → `applied: 2 pending: 0`, the step-4
  import block → no output and no errors, count parity **12/12** = 1,
  text-equality **8/8** = 0.
- **Still open (unchanged):** the MariaDB 10.6 *staging* proof of the
  charset/fidelity gates against the live engine, and step 8's login with the
  *real* prod password.

## Appendix — how passwords & sessions work across the switch

- **Passwords:** bcrypt cost-10 hashes are verified transparently, and upgraded to
  Argon2id on the login that confirms them. No user action, no data prep.
- **Sessions:** Nest's `session` table is left behind; the Rust `sessions` table
  (in the same DB) starts empty. Every user must log in after cutover — set a FRESH
  `SESSION_SECRET` key so stale Nest cookies can't authenticate against Rust.
- **Role data:** `users.role` is a plain varchar in both schemas — plain copy, no
  transformations of the enum/CHECK kind.

## Summary

- fresh staging DB + `migrate run` (never and nowhere on a populated DB)
- one dump → one scratch restore + cross-DB import with ghost-column strip
- the `.gz` is already UTF-8; the latin1 scratch is a **byte-exact prod replica**
  (the comparison reference), and the latin1→utf8mb4 conversion happens during
  step 4's `INSERT … SELECT` — `0002` is a no-op on the target
- verify counts, orphans, counters, text fidelity (incl. curly quote, emoji gate)
  **and schema drift** (step 5(d): staging schema vs a reference DB built from
  the same migrations) **before** seeding the default labels
- seed the default labels, then re-check `labels` with the seed-aware gates
- log in by hand; then blue-green cutover with backup-restore-only rollback
