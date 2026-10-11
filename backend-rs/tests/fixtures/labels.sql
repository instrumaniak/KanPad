-- tests/fixtures/labels.sql — FK level 1 (needs users).
-- Rows 51-56 mirror prod DEFAULT_LABELS exactly (name+color per user 1);
-- 57 is user 2's foreign label. Colors satisfy CHK_labels_color.
INSERT INTO `labels` (`id`, `name`, `color`, `user_id`) VALUES
  (51, 'Urgent', 'red', 1),
  (52, 'Important', 'orange', 1),
  (53, 'In Progress', 'yellow', 1),
  (54, 'Done', 'green', 1),
  (55, 'Bug', 'blue', 1),
  (56, 'Feature', 'purple', 1),
  (57, 'Foreign', 'red', 2);
