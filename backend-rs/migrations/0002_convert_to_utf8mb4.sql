-- 0002_convert_to_utf8mb4.sql — KanPad live charset conversion (T02b).
--
-- Per specs/api-rewrite-rs-specs/31-live-unicode-migration.md §31.3: one-shot
-- per-table `CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci`
-- plus `ROW_FORMAT=DYNAMIC` (index math: `users.email varchar(255)` UNIQUE is
-- 255x4=1020 bytes — OK for DYNAMIC/3072, fails for COMPACT/767).
--
-- Rules honored:
--   * No hardcoded schema names (runs on staging/dev/prod alike — never
--     `ALTER DATABASE <name>`). No `ALTER DATABASE` at all: the DB default is
--     already utf8mb4 from the baseline (`0001` spells it on every table).
--   * No FK change (order irrelevant; FKs survive CONVERT untouched).
--   * No Nest `session` convert: staging migrated from the baseline has no
--     Nest table (unconditional ALTER would fail fail-fast); abandoned at
--     cutover. `tower_sessions.session` is created utf8mb4 by the baseline
--     already, so it is NOT converted here (store-owned — see baseline
--     deviation 11).
--   * `CONVERT TO` (rewrites data) — never bare `MODIFY` (relabels bytes).
--     On an already-utf8mb4 database this is a rewrite no-op; the real
--     convert happens only against a latin1 database (proven via the
--     latin1-fixture staging proof, not CI).
-- Covers every app table from 0001_baseline.sql (12 tables).

ALTER TABLE `users` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `projects` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `boards` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `columns` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `cards` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `labels` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `card_labels` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `checklists` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `checklist_items` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `notes` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `tags` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
ALTER TABLE `note_tags` CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci, ROW_FORMAT=DYNAMIC;
