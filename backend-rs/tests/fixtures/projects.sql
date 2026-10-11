-- tests/fixtures/projects.sql — FK level 1 (needs users).
-- 11/12 owned by user 1, 13 owned by user 2 (cross-user 404 oracle target).
INSERT INTO `projects` (`id`, `name`, `user_id`) VALUES
  (11, 'Alpha', 1),
  (12, 'Beta', 1),
  (13, 'Foreign Project', 2);
