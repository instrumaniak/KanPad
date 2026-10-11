-- tests/fixtures/notes.sql — FK level 5 (needs users, boards, projects, cards).
-- 81 is a general note (all links NULL), 82 is board-linked.
INSERT INTO `notes` (`id`, `title`, `content`, `board_id`, `project_id`, `card_id`, `user_id`) VALUES
  (81, 'General note', 'General note body', NULL, NULL, NULL, 1),
  (82, 'Board note', 'Board note body', 21, NULL, NULL, 1);
