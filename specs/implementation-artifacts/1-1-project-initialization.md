# Story 1.1: Project Initialization

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a developer,
I want to initialize the monorepo with Vite frontend and NestJS backend,
so that the project has a solid foundation with modern tooling for building KanbanFlow.

## Acceptance Criteria

1. **Given** a fresh project directory, **When** I run the initialization commands, **Then** a monorepo structure is created with `frontend/` and `backend/` directories
2. **And** frontend uses Vite with React TypeScript template
3. **And** backend uses NestJS CLI with TypeScript
4. **And** shared `.prettierrc` is configured at root
5. **And** `frontend/` has shadcn/ui initialized with Tailwind CSS
6. **And** both projects have `package.json` with correct dependencies
7. **And** `.gitignore` covers node_modules, dist, .env files

## Tasks / Subtasks

- [x] Task 1: Initialize frontend with Vite + React + TypeScript (AC: 1, 2)
  - [x] Run `npm create vite@latest frontend -- --template react-ts`
  - [x] Run `npm install` in frontend directory
  - [x] Initialize shadcn/ui with `npx shadcn@latest init -t vite`
  - [x] Verify Tailwind CSS v4 is configured via `@tailwindcss/vite`
  - [x] Verify `components.json` exists for shadcn/ui config
- [x] Task 2: Initialize backend with NestJS + TypeScript (AC: 1, 3)
  - [x] Run `npx @nestjs/cli new backend --package-manager npm`
  - [x] Verify `nest-cli.json`, `tsconfig.json`, `tsconfig.build.json` exist
  - [x] Verify NestJS module structure in `src/`
- [x] Task 3: Configure shared tooling at monorepo root (AC: 4)
  - [x] Create `.prettierrc` with: `semi: true, singleQuote: true, trailingComma: "all", printWidth: 100, tabWidth: 2, arrowParens: "always"`
  - [x] Verify `.prettierrc` applies to both frontend and backend
- [x] Task 4: Verify project configuration files (AC: 5, 6, 7)
  - [x] Confirm `frontend/package.json` has React, Vite, Tailwind, shadcn/ui dependencies
  - [x] Confirm `backend/package.json` has NestJS dependencies
  - [x] Create/update `.gitignore` covering: `node_modules/`, `dist/`, `.env`, `.env.local`, `*.log`
- [x] Task 5: Verify TypeScript strict mode (AC: 2, 3)
  - [x] Confirm `frontend/tsconfig.json` has `strict: true`
  - [x] Confirm `backend/tsconfig.json` has `strict: true`
  - [x] Verify separate tsconfig for frontend (Vite) and backend (NestJS)

## Dev Notes

### Technology Stack

- **Frontend:** React 18+ with Vite (latest), TypeScript strict mode
- **Backend:** NestJS (latest), TypeScript strict mode
- **Styling:** Tailwind CSS v4 via `@tailwindcss/vite`, shadcn/ui (Radix UI primitives)
- **Build:** Vite for frontend, NestJS CLI/Webpack for backend
- **Testing:** Vitest (frontend), Jest (backend - NestJS default)

### Architecture Decisions

- Monorepo structure with `frontend/` and `backend/` directories at project root
- Official Vite + NestJS CLIs — no third-party scaffolds
- Shared Prettier config at monorepo root — no project-level overrides
- shadcn/ui initialized with `npx shadcn@latest init -t vite` for proper Tailwind v4 integration [Source: architecture.md#Starter Template Evaluation]

### Initialization Commands

**Frontend:**
```bash
npm create vite@latest frontend -- --template react-ts
cd frontend
npm install
npx shadcn@latest init -t vite
```

**Backend:**
```bash
npx @nestjs/cli new backend --package-manager npm
```

[Source: architecture.md#Starter Template Evaluation]

### Prettier Configuration

```json
{
  "semi": true,
  "singleQuote": true,
  "trailingComma": "all",
  "printWidth": 100,
  "tabWidth": 2,
  "arrowParens": "always"
}
```

[Source: architecture.md#Implementation Patterns - Code Formatting]

### Expected Project Structure After This Story

```
KanbanFlow/
├── .prettierrc
├── .gitignore
│
├── frontend/
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── components.json
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── index.css
│       └── components/
│           └── ui/
│
├── backend/
│   ├── package.json
│   ├── nest-cli.json
│   ├── tsconfig.json
│   ├── tsconfig.build.json
│   └── src/
│       ├── main.ts
│       └── app.module.ts
```

[Source: architecture.md#Complete Project Directory Structure]

### Critical Rules

- TypeScript strict mode enabled — no `any` type allowed [Source: project-context.md#Language-Specific Rules]
- `.gitignore` must cover `node_modules`, `dist`, `.env` files [Source: epic acceptance criteria]
- shadcn/ui components go in `components/ui/` — never modify directly [Source: project-context.md#Framework-Specific Rules]
- Separate `tsconfig.json` for frontend (Vite) and backend (NestJS) [Source: project-context.md#Language-Specific Rules]
- `synchronize: false` ALWAYS for TypeORM — no auto-sync (applies in later stories but set the pattern) [Source: architecture.md#Database Migration Strategy]

### Project Structure Notes

- This is the foundational story — all subsequent stories depend on this structure
- Feature-based folder organization (`features/`) will be added in later stories as features are built
- Backend module structure (`src/auth/`, `src/boards/`, etc.) will be added per-feature
- Tests co-located with implementation files (`.spec.ts` for backend, `.test.tsx` for frontend)

### References

- [Source: architecture.md#Starter Template Evaluation] — Initialization commands and rationale
- [Source: architecture.md#Complete Project Directory Structure] — Full expected tree
- [Source: architecture.md#Implementation Patterns - Code Formatting] — Prettier config
- [Source: project-context.md#Technology Stack & Versions] — Stack details and versions
- [Source: project-context.md#Language-Specific Rules] — TypeScript and naming rules
- [Source: epics.md#Story 1.1] — Original story acceptance criteria
- [Source: ux-design-specification.md#Design System Foundation] — shadcn/ui + Tailwind CSS rationale

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

- Frontend initialized with Vite + React + TypeScript template
- shadcn/ui initialized with Radix Nova preset and Tailwind CSS v4 via @tailwindcss/vite
- Backend initialized with NestJS CLI
- Root .prettierrc created with project standards, backend local .prettierrc removed
- Root .gitignore created with node_modules, dist, .env patterns
- TypeScript strict mode enabled in both frontend (tsconfig.app.json) and backend (tsconfig.json)
- Import alias @/* configured in frontend for shadcn/ui components
- All typechecks pass, backend tests pass

### File List

- .gitignore (created)
- .prettierrc (created)
- frontend/ (entire directory - scaffolded by Vite + shadcn)
  - frontend/vite.config.ts (modified - added Tailwind CSS plugin and path alias)
  - frontend/tsconfig.json (modified - added baseUrl and paths)
  - frontend/tsconfig.app.json (modified - added baseUrl and paths)
  - frontend/src/index.css (modified - added @import "tailwindcss")
  - frontend/components.json (created by shadcn init)
  - frontend/src/components/ui/button.tsx (created by shadcn init)
  - frontend/src/lib/utils.ts (created by shadcn init)
- backend/ (entire directory - scaffolded by NestJS CLI)
  - backend/tsconfig.json (modified - enabled strict: true)
  - backend/.prettierrc (deleted - using root config)
