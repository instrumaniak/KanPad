# Lessons learned — Nest backend (backend/)

Source: extracted from root `AGENTS.md` (2026-11-10).

- Backend unit-spec mocks: avoid `as any` (triggers `@typescript-eslint/no-unsafe-*`). Use `as unknown as Repository<T>` + `jest.Mock` typed helpers, and assert on the mock variable directly instead of `expect(repo.method)` (avoids `@typescript-eslint/unbound-method`).
- Backend `Board` maps `project_id` twice (`@Column` + `@ManyToOne/@JoinColumn` on the same column). `save()` persists the FK from the **relation side**: when updating `project_id`, always set `board.project` too (entity or `null`), never the FK column alone. Ownership checks must use an injected `Repository<Project>` (`TypeOrmModule.forFeature` + `@InjectRepository(Project)`) — never `manager.findOne('project', ...)` string lookup (throws 500).
