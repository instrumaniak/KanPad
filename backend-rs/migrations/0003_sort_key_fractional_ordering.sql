-- 0003_sort_key_fractional_ordering.sql — T010: fractional sort keys replace
-- the integer `position` scheme on `columns`, `cards` and `checklist_items`.
--
-- Position moves become single-row writes that never touch siblings. The
-- wire contract is unchanged (integer 0-based `position`, computed at read —
-- see CONTRACT-CHANGES.md §12); `sort_key` never leaks to the wire.
--
-- Statement order (single release, brief downtime):
--   1. ADD `sort_key` (DEFAULT '' so pre-existing rows gain a placeholder).
--   2. Backfill: ROW_NUMBER() partitioned by parent, ordered by
--      (`position`, `id`) — deterministic under today's duplicate positions.
--      value v = rn - 1 encoded big-endian base62 with 3 digits; head `c`
--      declares a 4-char integer part ('c000' .. 'cZZZ', 62^3 = 238,328 keys
--      per parent). ELT returns NULL out-of-range -> NOT NULL aborts (loud,
--      never silent corruption). Keys are unique + monotonic per parent.
--      `ascii_bin` collation is load-bearing: base62 byte order == alphabet
--      order, any *_ci collation silently reorders keys.
--   3. Drop the temporary DEFAULT so future INSERTs must carry `sort_key`.
--   4. UNIQUE(parent, sort_key): UQ_columns_board_sort_key,
--      UQ_cards_column_sort_key, UQ_checklist_items_checklist_sort_key.
--   5. Drop the old non-unique composite position indexes and the columns.
-- NOT editable: applied once; changing bytes breaks the sqlx checksum.

ALTER TABLE `columns` ADD COLUMN `sort_key` VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT '' AFTER `name`;
ALTER TABLE `cards` ADD COLUMN `sort_key` VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT '' AFTER `title`;
ALTER TABLE `checklist_items` ADD COLUMN `sort_key` VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL DEFAULT '' AFTER `text`;

UPDATE `columns` t JOIN (
  SELECT `id`, ROW_NUMBER() OVER (PARTITION BY `board_id` ORDER BY `position`, `id`) AS rn
  FROM `columns`
) w ON w.`id` = t.`id`
SET t.`sort_key` = CONCAT('c',
  ELT((w.rn - 1) DIV 3844 + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'),
  ELT(MOD((w.rn - 1) DIV 62, 62) + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'),
  ELT(MOD(w.rn - 1, 62) + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'));

UPDATE `cards` t JOIN (
  SELECT `id`, ROW_NUMBER() OVER (PARTITION BY `column_id` ORDER BY `position`, `id`) AS rn
  FROM `cards`
) w ON w.`id` = t.`id`
SET t.`sort_key` = CONCAT('c',
  ELT((w.rn - 1) DIV 3844 + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'),
  ELT(MOD((w.rn - 1) DIV 62, 62) + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'),
  ELT(MOD(w.rn - 1, 62) + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'));

UPDATE `checklist_items` t JOIN (
  SELECT `id`, ROW_NUMBER() OVER (PARTITION BY `checklist_id` ORDER BY `position`, `id`) AS rn
  FROM `checklist_items`
) w ON w.`id` = t.`id`
SET t.`sort_key` = CONCAT('c',
  ELT((w.rn - 1) DIV 3844 + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'),
  ELT(MOD((w.rn - 1) DIV 62, 62) + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'),
  ELT(MOD(w.rn - 1, 62) + 1, '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z'));

ALTER TABLE `columns` ALTER COLUMN `sort_key` DROP DEFAULT;
ALTER TABLE `cards` ALTER COLUMN `sort_key` DROP DEFAULT;
ALTER TABLE `checklist_items` ALTER COLUMN `sort_key` DROP DEFAULT;

ALTER TABLE `columns` ADD CONSTRAINT `UQ_columns_board_sort_key` UNIQUE (`board_id`, `sort_key`);
ALTER TABLE `cards` ADD CONSTRAINT `UQ_cards_column_sort_key` UNIQUE (`column_id`, `sort_key`);
ALTER TABLE `checklist_items` ADD CONSTRAINT `UQ_checklist_items_checklist_sort_key` UNIQUE (`checklist_id`, `sort_key`);

ALTER TABLE `columns` DROP INDEX `IDX_columns_board_position`;
ALTER TABLE `cards` DROP INDEX `IDX_cards_column_position`;
ALTER TABLE `checklist_items` DROP INDEX `IDX_checklist_items_checklist_position`;

ALTER TABLE `columns` DROP COLUMN `position`;
ALTER TABLE `cards` DROP COLUMN `position`;
ALTER TABLE `checklist_items` DROP COLUMN `position`;
