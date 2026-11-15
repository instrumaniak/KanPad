-- tests/fixtures/columns.sql — FK level 3 (needs boards).
-- `columns` is a reserved word: always backticked. Dense sort keys a0.. on 21.
INSERT INTO `columns` (`id`, `name`, `sort_key`, `board_id`) VALUES
  (31, 'To Do', 'a0', 21),
  (32, 'In Progress', 'a1', 21),
  (33, 'Done', 'a2', 21),
  (34, 'Solo Column', 'a0', 22);
