-- tests/fixtures/users.sql — FK level 0 (no dependencies).
-- Fixed ids; passwords are opaque placeholders (auth suites overwrite with
-- real Argon2/bcrypt hashes). Parallel-safe: loaded into per-test isolated
-- DBs via #[sqlx::test(fixtures(..))], or once per shared-DB suite.
INSERT INTO `users` (`id`, `email`, `password`, `role`) VALUES
  (1, 'owner@example.com', '$argon2id$v=19$m=19456,t=2,p=1$c2VlazEyM3NhbHQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA', 'user'),
  (2, 'intruder@example.com', '$argon2id$v=19$m=19456,t=2,p=1$c2VlazEyM3NhbHQ$BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB', 'user');
