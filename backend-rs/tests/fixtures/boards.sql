-- tests/fixtures/boards.sql — FK level 2 (needs users, projects).
-- 21 project-linked, 22 standalone, 23 foreign (user 2), 24 archived.
INSERT INTO `boards` (`id`, `name`, `background_color`, `user_id`, `project_id`, `is_archived`, `view_mode`) VALUES
  (21, 'Board One', '#0079BF', 1, 11, 0, 'board'),
  (22, 'Solo Board', '#FF5733', 1, NULL, 0, 'list'),
  (23, 'Foreign Board', '#0079BF', 2, NULL, 0, 'board'),
  (24, 'Archived Board', '#0079BF', 1, 11, 1, 'board');
