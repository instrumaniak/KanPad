-- tests/fixtures/cards.sql — FK level 4 (needs columns).
-- 41 carries a description (has_description=true), 42/43 do not.
INSERT INTO `cards` (`id`, `title`, `column_id`, `position`, `description`, `due_date`) VALUES
  (41, 'First card', 31, 0, 'First card body', NULL),
  (42, 'Second card', 31, 1, NULL, NULL),
  (43, 'Lonely card', 32, 0, NULL, NULL);
