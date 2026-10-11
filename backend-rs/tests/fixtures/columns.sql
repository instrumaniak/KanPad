-- tests/fixtures/columns.sql — FK level 3 (needs boards).
-- `columns` is a reserved word: always backticked. Dense positions 0..2 on 21.
INSERT INTO `columns` (`id`, `name`, `position`, `board_id`) VALUES
  (31, 'To Do', 0, 21),
  (32, 'In Progress', 1, 21),
  (33, 'Done', 2, 21),
  (34, 'Solo Column', 0, 22);
