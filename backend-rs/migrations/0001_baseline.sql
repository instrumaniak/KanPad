-- 0001_baseline.sql — KanPad backend-rs squashed baseline (T02).
--
-- Source: Nest TypeORM migrations at tip (backend/src/migrations/*.ts, 10 files)
-- hand-cleaned per specs/api-rewrite-rs-specs/30-db-schema.md.
-- One CREATE TABLE per app entity (12 tables), no IF NOT EXISTS on app tables
-- (fail fast), explicit ENGINE/CHARSET/COLLATE, DATETIME(6), INT SIGNED.
--
-- Documented deviations from the Nest-tip mysqldump (see T02 diff review):
--   1. Ghost columns dropped: users.username, projects.description
--      (in 1700000000000-CreateUsersProjects.ts, in NO entity).
--   2. Composite indexes replace single-column ones: projects(user_id,created_at),
--      boards(user_id,updated_at), columns(board_id,position),
--      cards(column_id,position), checklist_items(checklist_id,position),
--      notes(board_id,updated_at) / (project_id,updated_at) /
--      (card_id,updated_at) / (user_id,updated_at).
--   3. New single-column indexes: cards(due_date), notes(title),
--      card_labels(label_id), note_tags(tag_id).
--   4. New UNIQUE constraints: UQ_labels_user_name, UQ_tags_user_name.
--   5. New CHECK constraints: CHK_users_role, CHK_boards_view_mode,
--      CHK_labels_color (VARCHAR plus CHECK, never MySQL ENUM).
--   6. Migration-only FKs added: notes.user_id and tags.user_id
--      ON DELETE CASCADE (no entity declares the relation).
--   7. notes.tag link made DB-side both sides: note_tags FKs
--      ON DELETE CASCADE (Nest relied on TypeORM remove() cleanup).
--   8. Explicit utf8mb4 plus utf8mb4_unicode_ci plus InnoDB on every app table
--      (TypeORM emitted bare ENGINE=InnoDB, inheriting DB defaults).
--   9. columns.position gains DEFAULT 0 (parity with cards/checklist_items).
--  10. cards.due_date normalized to DATETIME(6) NULL (was precision-0 datetime).
--  11. Store-owned session schema tower_sessions.session replaces the Nest
--      public-schema session table (no rename, no row port, force re-login).
--      Table DDL is verbatim from tower-sessions-sqlx-store 0.15
--      MySqlStore::migrate() (keeps the store IF NOT EXISTS: documented
--      exception). IDX_session_expiry_date is a baseline addition for the
--      sweeper: the store migrate() creates no index (verified against the
--      0.15.0 crate source), so SHOW CREATE TABLE differs from a pure
--      migrate() run by exactly this index.
--  12. sqlx journal is _sqlx_migrations (TypeORM journal abandoned).
--  13. AUTO_INCREMENT=N table options stripped (nondeterministic checksums).

-- First statement: store-owned session schema.
CREATE SCHEMA tower_sessions;

-- Store-owned session table, DDL verbatim from
-- tower-sessions-sqlx-store 0.15 MySqlStore::migrate().
CREATE TABLE IF NOT EXISTS `tower_sessions`.`session` (
  `id` CHAR(22) NOT NULL,
  `data` BLOB NOT NULL,
  `expiry_date` TIMESTAMP(6) NOT NULL,
  PRIMARY KEY (`id`)
);

-- Sweeper index (baseline addition, see deviation 11 above).
CREATE INDEX `IDX_session_expiry_date` ON `tower_sessions`.`session` (`expiry_date`);

CREATE TABLE `users` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `email` VARCHAR(255) NOT NULL,
  `password` VARCHAR(255) NOT NULL,
  `role` VARCHAR(255) NOT NULL DEFAULT 'user',
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  UNIQUE INDEX `IDX_users_email` (`email`),
  PRIMARY KEY (`id`),
  CONSTRAINT `CHK_users_role` CHECK (`role` IN ('user', 'admin'))
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `projects` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `name` VARCHAR(255) NOT NULL,
  `user_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_projects_user_created` (`user_id`, `created_at`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_projects_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `boards` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `name` VARCHAR(255) NOT NULL,
  `background_color` VARCHAR(7) NOT NULL DEFAULT '#0079BF',
  `user_id` INT NOT NULL,
  `project_id` INT NULL,
  `is_archived` TINYINT(1) NOT NULL DEFAULT 0,
  `view_mode` VARCHAR(10) NOT NULL DEFAULT 'board',
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_boards_user_updated` (`user_id`, `updated_at`),
  INDEX `IDX_boards_project_id` (`project_id`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_boards_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `FK_boards_project_id` FOREIGN KEY (`project_id`) REFERENCES `projects` (`id`) ON DELETE SET NULL ON UPDATE NO ACTION,
  CONSTRAINT `CHK_boards_view_mode` CHECK (`view_mode` IN ('board', 'list'))
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `columns` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `name` VARCHAR(255) NOT NULL,
  `position` INT NOT NULL DEFAULT 0,
  `board_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_columns_board_position` (`board_id`, `position`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_columns_board_id` FOREIGN KEY (`board_id`) REFERENCES `boards` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `cards` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `title` VARCHAR(255) NOT NULL,
  `column_id` INT NOT NULL,
  `position` INT NOT NULL DEFAULT 0,
  `description` TEXT NULL,
  `due_date` DATETIME(6) NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_cards_column_position` (`column_id`, `position`),
  INDEX `IDX_cards_due_date` (`due_date`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_cards_column_id` FOREIGN KEY (`column_id`) REFERENCES `columns` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `labels` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `name` VARCHAR(50) NOT NULL,
  `color` VARCHAR(20) NOT NULL,
  `user_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_labels_user_id` (`user_id`),
  CONSTRAINT `UQ_labels_user_name` UNIQUE (`user_id`, `name`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_labels_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `CHK_labels_color` CHECK (`color` IN ('red', 'orange', 'yellow', 'green', 'blue', 'purple'))
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `card_labels` (
  `card_id` INT NOT NULL,
  `label_id` INT NOT NULL,
  PRIMARY KEY (`card_id`, `label_id`),
  INDEX `IDX_card_labels_label_id` (`label_id`),
  CONSTRAINT `FK_card_labels_card_id` FOREIGN KEY (`card_id`) REFERENCES `cards` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `FK_card_labels_label_id` FOREIGN KEY (`label_id`) REFERENCES `labels` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `checklists` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `title` VARCHAR(255) NOT NULL,
  `card_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_checklists_card_id` (`card_id`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_checklists_card_id` FOREIGN KEY (`card_id`) REFERENCES `cards` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `checklist_items` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `text` VARCHAR(255) NOT NULL,
  `is_completed` TINYINT(1) NOT NULL DEFAULT 0,
  `checklist_id` INT NOT NULL,
  `position` INT NOT NULL DEFAULT 0,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_checklist_items_checklist_position` (`checklist_id`, `position`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_checklist_items_checklist_id` FOREIGN KEY (`checklist_id`) REFERENCES `checklists` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `notes` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `title` VARCHAR(255) NOT NULL,
  `content` MEDIUMTEXT NOT NULL,
  `board_id` INT NULL,
  `project_id` INT NULL,
  `card_id` INT NULL,
  `user_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  INDEX `IDX_notes_board_updated` (`board_id`, `updated_at`),
  INDEX `IDX_notes_project_updated` (`project_id`, `updated_at`),
  INDEX `IDX_notes_card_updated` (`card_id`, `updated_at`),
  INDEX `IDX_notes_user_updated` (`user_id`, `updated_at`),
  INDEX `IDX_notes_title` (`title`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_notes_board_id` FOREIGN KEY (`board_id`) REFERENCES `boards` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `FK_notes_project_id` FOREIGN KEY (`project_id`) REFERENCES `projects` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `FK_notes_card_id` FOREIGN KEY (`card_id`) REFERENCES `cards` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `FK_notes_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `tags` (
  `id` INT NOT NULL AUTO_INCREMENT,
  `name` VARCHAR(50) NOT NULL,
  `color` VARCHAR(20) NOT NULL DEFAULT 'teal',
  `user_id` INT NOT NULL,
  `created_at` DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  INDEX `IDX_tags_user_id` (`user_id`),
  CONSTRAINT `UQ_tags_user_name` UNIQUE (`user_id`, `name`),
  PRIMARY KEY (`id`),
  CONSTRAINT `FK_tags_user_id` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE `note_tags` (
  `note_id` INT NOT NULL,
  `tag_id` INT NOT NULL,
  PRIMARY KEY (`note_id`, `tag_id`),
  INDEX `IDX_note_tags_tag_id` (`tag_id`),
  CONSTRAINT `FK_note_tags_note_id` FOREIGN KEY (`note_id`) REFERENCES `notes` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION,
  CONSTRAINT `FK_note_tags_tag_id` FOREIGN KEY (`tag_id`) REFERENCES `tags` (`id`) ON DELETE CASCADE ON UPDATE NO ACTION
) ENGINE=InnoDB DEFAULT CHARACTER SET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
