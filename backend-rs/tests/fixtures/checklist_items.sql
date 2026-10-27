-- tests/fixtures/checklist_items.sql — FK level 6 (needs checklists).
INSERT INTO `checklist_items` (`id`, `text`, `is_completed`, `checklist_id`, `position`) VALUES
  (71, 'Write test', 0, 61, 0),
  (72, 'Pass test', 1, 61, 1);
