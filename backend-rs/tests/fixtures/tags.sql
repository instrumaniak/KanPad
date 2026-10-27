-- tests/fixtures/tags.sql — FK level 1 (needs users).
-- 91 owned by user 1, 92 foreign (user 2).
INSERT INTO `tags` (`id`, `name`, `color`, `user_id`) VALUES
  (91, 'Work', 'teal', 1),
  (92, 'Foreign', 'red', 2);
