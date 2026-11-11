# T011: Automated DB backups + restore drill

> **STATUS: pending.** Wave 0 (reliability floor). Closes **R1** (HIGH).

## Goal

Make data loss impossible-by-accident: one backup script, scheduled on both deploy
paths, with retention, an optional offsite hook, and a **recorded restore drill** —
because `docs/cutover.md` makes restore the *only* DB rollback path and nothing today
automates or proves it.

Deliverables:
1. `scripts/backup-db.sh` — `mysqldump --single-transaction` (+ routines/events/triggers,
   `--source-data=2`) → gzip → timestamped file → `gzip -t` verify → retention prune
   (7 daily / 4 weekly, never deletes newest) → optional offsite copy (rsync/rclone if
   configured; loud log, non-fatal if it fails).
2. Scheduling for both paths (D-1): systemd timer example + cPanel cron line, documented.
3. `docs/` restore runbook + **filled drill record** (import into scratch DB, table/row
   parity, `migrate info` all-applied, login smoke, wall-clock time, `SELECT VERSION()`).

## Depends On

(None — but every production-migration task treats this as a prerequisite.)

## Phase

0

## Critical

Yes — R1 is the highest-severity reliability finding; restore is the only documented
DB rollback and it is currently untested.

## Spec References

- `specs/arch-hardening-specs/10-reliability.md` §R1 (full design: script, retention,
  drill procedure, edge cases).
- `specs/arch-hardening-specs/40-operations.md` §3 (ops contract), §6 (record
  `SELECT VERSION()` — resolves the MariaDB 10.6-vs-11.4 doc conflict).
- `specs/arch-hardening-specs/00-overview.md` — global DoD; findings R1, C3.
- `docs/data-migration-live.md` §1/§9 (existing dump commands to reuse),
  `docs/cutover.md:18-19` (restore-only rollback).
- `specs/security/09-logging-monitoring-ir.md:41` (the spec line this closes).

## Files to Create/Modify

| File | Change |
|---|---|
| `scripts/backup-db.sh` | **NEW** — dump/verify/prune/offsite (POSIX sh or bash, shellcheck-clean, `--dry-run` mode, no password on argv) |
| `docs/operations-backup.md` | **NEW** — schedule setup (systemd timer + cPanel cron), retention policy, restore runbook, drill template |
| `docs/operations-backup.md` or `docs/` drill log | **NEW** — first filled restore-drill record (date, duration, parity results, `SELECT VERSION()`) |
| `docs/mariadb-compat.md`, `specs/security/06-infrastructure-deployment.md` | Correct the prod-version line to whatever the drill records (hand off to T019 if doc-sync conflicts) |
| `docs/lessons/data-migration.md` | Dated, tagged lesson |

**Must NOT appear in the diff:** real credentials, `.env` values, `_sqlx_migrations`
edits, any app code.

## Implementation Steps

TDD-ish: the drill is the proof; script behavior gets dry-run tests.

1. **Recon:** confirm client tools available on both paths (`mysqldump` vs
   `mariadb-dump`); capture `SELECT VERSION()` from the real DB (evidence for C3).
2. **Script:** dump → `gzip -t` → prune (7d/4w pattern, never newest) → optional
   `$BACKUP_RSYNC_TARGET`/`$BACKUP_Rclone_REMOTE` copy. `MYSQL_PWD` or defaults-file
   auth, never `-p$PW` on argv; `set -euo pipefail`; shellcheck clean.
3. **Dry-run test:** `--dry-run` prints the exact commands without executing (paste as
   evidence); a fixture-based run against a scratch DB proves round-trip.
4. **Restore runbook + drill template** in `docs/operations-backup.md`.
5. **Execute the drill** against a *copy* of real data (or seeded compose DB): import to
   scratch → 12-table count parity + `migrate info` pending=0 → login smoke → record
   wall-clock + `SELECT VERSION()`.
6. **Scheduling docs:** systemd timer (Path A) + cPanel cron (Path B), both calling the
   same script with env from the deployment's `.env`.
7. **Spec/docs sync + lesson.**

## Constraints

- Script must work with **MySQL 8.4 and MariaDB** clients (global DoD).
- Never echo/log the password; `BACKUP_DIR` perms 700.
- Retention prune must be crash-safe (prune only after a *successful* verified dump).
- No new services/daemons beyond cron/timer (cost guardrail `40-operations.md` §8).
- No commit without user approval (AGENTS.md).

## Acceptance Criteria

- [ ] `scripts/backup-db.sh` runs end-to-end against a scratch DB: dump, `gzip -t`,
      prune honors 7d/4w and never deletes the newest file (red-proof: fake old
      timestamps get pruned, fresh file survives).
- [ ] `--dry-run` outputs the commands it would run; shellcheck clean.
- [ ] No password appears in argv, logs, or the script file.
- [ ] Restore runbook exists; **drill record committed**: parity results, `migrate info`
      pending=0, login smoke, duration, `SELECT VERSION()` (closes **R1**, evidence for
      **C3** version ambiguity).
- [ ] Both scheduling examples (systemd timer + cPanel cron) documented and reference
      the same script.
- [ ] Lesson appended to `docs/lessons/data-migration.md`.

## Notes

(filled in during/after implementation)

## Lessons

(pending)
