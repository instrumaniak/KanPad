---
project_name: 'KanbanFlow'
user_name: 'Raziur'
date: '2026-03-20'
sections_completed: ['technology_stack', 'language_rules', 'framework_rules', 'testing_rules', 'code_quality_rules', 'workflow_rules', 'critical_rules']
status: 'complete'
rule_count: 75
optimized_for_llm: true
---

# Project Context for AI Agents

_This file contains critical rules and patterns that AI agents must follow when implementing code in this project. Focus on unobvious details that agents might otherwise miss._

---

## Technology Stack & Versions

### Core Runtime
- Node.js (backend runtime)
- TypeScript (strict mode) for both frontend and backend

### Frontend
- React 18+ with Vite (latest)
- Tailwind CSS v4 via `@tailwindcss/vite`
- shadcn/ui (Radix UI primitives)
- React Query (TanStack Query) for server state
- React Router for routing
- dnd-kit for drag-drop interactions
- react-markdown + rehype-highlight + remark-gfm for markdown rendering
- mermaid (via remark-mermaidjs or @mermaid-js/mermaid) for diagram rendering

### Backend
- NestJS (latest)
- TypeORM (latest stable) with MySQL 8.x
- `@nestjs/swagger` for API documentation
- `@nestjs/throttler` for rate limiting
- bcrypt for password hashing

### Testing
- Vitest for frontend unit tests
- Jest for backend unit tests (NestJS default)

### Build & Tooling
- Vite for frontend builds (fast HMR)
- NestJS CLI/Webpack for backend compilation
- Prettier with shared `.prettierrc` at monorepo root
- ESLint for linting

## Critical Implementation Rules

### Language-Specific Rules

- TypeScript strict mode enabled — no `any` type allowed
- Separate `tsconfig.json` for frontend (Vite) and backend (NestJS)
- Backend: NestJS modules with `@Module()` decorators, DI via constructor
- Frontend: Feature-based imports from `features/`, shared utilities from `lib/`
- Backend exceptions: Use NestJS built-in (`NotFoundException`, `BadRequestException`, etc.)
- Global exception filter formats all errors consistently
- Frontend errors: React Query `onError` handlers display toast notifications
- User-facing messages: Friendly, actionable (not raw error codes)
- Backend files: `kebab-case` (e.g., `board.entity.ts`, `create-board.dto.ts`)
- Frontend components: `PascalCase` (e.g., `BoardView.tsx`), files `kebab-case` (e.g., `board-view.tsx`)
- TypeScript interfaces: `I` prefix for NestJS (e.g., `IBoard`), no prefix for frontend
- Functions/methods: `camelCase`, Constants: `UPPER_SNAKE_CASE`

### Framework-Specific Rules

**React Frontend:**
- Use React Query (`useQuery`, `useMutation`) for ALL server state — no raw `fetch` calls
- Custom hooks in `hooks/` for shared logic, feature hooks co-located with feature
- Feature-based organization under `features/` — each owns components, hooks, API calls
- Shared UI from `components/ui/` (shadcn/ui) — never modify directly
- Layouts in `layouts/` for page structure — no data fetching
- Skeleton screens for initial page loads, spinner on buttons for form submissions
- Optimistic UI updates for drag-drop (no spinner)

**NestJS Backend:**
- Module-based architecture — each domain is a bounded context
- Controllers handle HTTP, Services contain business logic, Entities define schema
- DTOs define API contracts — validation at controller level
- Guards for auth (`AuthGuard`) and authorization (`AdminGuard`)
- Swagger decorators on all endpoints for API documentation
- Use NestJS built-in exceptions only (`NotFoundException`, `BadRequestException`, etc.)

### Testing Rules

**Test Organization:**
- Backend: Co-located `.spec.ts` files next to source (e.g., `boards.service.spec.ts`)
- Frontend: Co-located `.test.tsx` files next to components (e.g., `board-view.test.tsx`)
- No separate test directories — keeps tests close to implementation

**Unit Tests (Vitest + Jest):**
- Backend services: Test business logic in isolation, mock repositories
- Frontend components: Test rendering, user interactions, hooks
- Utilities: Test all helper functions in `lib/`

**Integration Tests:**
- Backend: Test API endpoints with real database (test DB), verify full request/response cycle
- Frontend: Test feature workflows (e.g., create board → add column → add card)
- Test TypeORM entity relationships and cascade deletes

**E2E Tests (Playwright recommended):**
- Full user journeys: Login → Create Project → Create Board → Drag Cards → Logout
- Admin workflows: View users → Block user → Toggle registration
- Cross-browser testing: Chrome, Firefox, Safari

**Coverage Requirements:**
- Unit tests: 80%+ line coverage for services and utilities
- Integration tests: All API endpoints covered
- E2E tests: Critical user paths (auth, CRUD, drag-drop)

**Mock Patterns:**
- Mock external dependencies (database, APIs) in unit tests
- Use real implementations in integration tests
- E2E tests use real browser + test database

**Test Boundaries:**
- Unit: Individual functions, components, services
- Integration: API endpoints, feature workflows, database operations
- E2E: Complete user journeys across the application

**Test Data:**
- Use factories/fixtures for consistent test data
- Clean up test database between integration tests
- Seed test data for E2E test suites

### Code Quality & Style Rules

**Prettier Rules:**
- Shared `.prettierrc` at monorepo root — no project-level overrides
- Config: `semi: true`, `singleQuote: true`, `trailingComma: "all"`, `printWidth: 100`, `tabWidth: 2`, `arrowParens: "always"`
- Format on save via editor integration
- Pre-commit hook via `lint-staged` + `husky` to enforce formatting
- Never override Prettier formatting with manual adjustments

**ESLint Rules:**
- ESLint handles linting, Prettier handles formatting (no conflicts)
- Enforces code naming conventions
- TypeScript strict mode catches type mismatches

**Code Organization:**
- Backend: Module-based (`src/auth/`, `src/boards/`, etc.)
- Frontend: Feature-based (`features/auth/`, `features/boards/`, etc.)
- Shared utilities in `lib/` — no business logic
- Tests co-located with implementation files

**Naming Conventions:**
- DB tables: `snake_case` plural (e.g., `users`, `boards`)
- DB columns: `snake_case` (e.g., `created_at`, `board_id`)
- API endpoints: RESTful `kebab-case` plural (e.g., `/api/boards`)
- Backend files: `kebab-case` (e.g., `board.entity.ts`)
- Frontend files: `kebab-case` (e.g., `board-view.tsx`)
- Components: `PascalCase` (e.g., `BoardView.tsx`)

**Documentation Requirements:**
- Swagger decorators on all API endpoints
- JSDoc comments for complex functions
- README.md for project setup and architecture overview

### Development Workflow Rules

**Branch Naming:**
- Feature branches: `feature/{issue-description}` (e.g., `feature/add-card-drag-drop`)
- Bug fixes: `fix/{issue-description}`
- Chores: `chore/{task-description}`

**Commit Message Format:**
- Use conventional commits: `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `refactor:`
- Example: `feat(boards): add drag-drop card reordering`
- Example: `fix(auth): resolve session timeout issue`

**Development Servers:**
- Frontend: `http://localhost:5173` (Vite dev server)
- Backend: `http://localhost:3000` (NestJS)
- API proxy: Vite proxies `/api/*` to backend during development

**Database Workflow:**
- `synchronize: false` ALWAYS — no auto-sync in any environment
- Every entity change requires a new TypeORM migration
- Run `npm run migration:run` before starting work
- Test migrations locally before committing
- Commit entity + migration together

**Deployment Patterns:**
- cPanel Node.js app deployment
- Build frontend (`npm run build`) and backend (`npm run build`)
- Run migrations on deployment
- Environment variables via `.env` files

**PR Requirements:**
- All tests passing before merge
- Migration files included for schema changes
- No data loss in migration `down` methods
- Entity + migration committed together

### Critical Don't-Miss Rules

**Anti-Patterns to Avoid:**
- ❌ Mixing `snake_case` and `camelCase` in the same layer
- ❌ Creating new folder structures outside the established patterns
- ❌ Returning raw database errors to the frontend
- ❌ Using `any` type — always define proper TypeScript types
- ❌ Fetching data without React Query (e.g., raw `fetch` in components)
- ❌ Building custom UI components when shadcn/ui equivalent exists
- ❌ Mixing formatting styles — Prettier enforces one style across codebase
- ❌ Disabling Prettier for specific files without team consensus
- ❌ Modifying shadcn/ui components directly in `components/ui/`
- ❌ Business logic in `lib/` — utilities only
- ❌ Data fetching in layout components

**Security Rules:**
- Never expose raw database errors to frontend
- Sanitize markdown output with DOMPurify (XSS prevention)
- Use bcrypt for password hashing — no plain text storage
- httpOnly cookies for session tokens
- CSRF tokens for state-changing operations
- Rate limiting on auth endpoints

**Performance Gotchas:**
- No `synchronize: true` in TypeORM — always use migrations
- No unnecessary re-renders — memoize expensive computations
- No blocking operations in API responses
- Use optimistic updates for drag-drop (no loading spinners)

**Edge Cases to Handle:**
- Cascade deletes: Project → Boards → Columns → Cards
- Empty states: No projects, no boards, no cards
- Concurrent edits: Last-write-wins for single-user (MVP)
- Session expiration: Redirect to login on 401

**TypeORM Migration Rules:**
- `synchronize: false` ALWAYS — no exceptions
- Every entity change = new migration
- Migrations run explicitly — never auto-run on app start
- Never modify applied migrations — create new one to fix
- Backup before production migrations

**Notes System Rules:**
- Notes use nullable FKs (`board_id`, `project_id`, `card_id`) — all NULL = top-level note
- Tags are cross-cutting: `Tag` entity with `note_tags` join table now; `card_tags`/`project_tags`/`board_tags` can be added later
- Markdown rendering MUST be sanitized — use DOMPurify or equivalent on all rendered HTML
- Mermaid diagrams render client-side only — no server-side mermaid processing
- Note auto-save: debounced 2s after typing stops, show "Saved" indicator
- Board view toggle preserves scroll position and filters when switching back

---

## Usage Guidelines

**For AI Agents:**
- Read this file before implementing any code
- Follow ALL rules exactly as documented
- When in doubt, prefer the more restrictive option
- Update this file if new patterns emerge

**For Humans:**
- Keep this file lean and focused on agent needs
- Update when technology stack changes
- Review quarterly for outdated rules
- Remove rules that become obvious over time

Last Updated: 2026-03-20