-- tests/fixtures/cards.sql — FK level 4 (needs columns).
-- 41 carries a description (has_description=true), 42/43 do not.
INSERT INTO `cards` (`id`, `title`, `column_id`, `sort_key`, `description`, `due_date`) VALUES
  (41, 'First card', 31, 'a0', 'First card body', NULL),
  (42, 'Second card', 31, 'a1', NULL, NULL),
  (43, 'Lonely card', 32, 'a0', NULL, NULL);
