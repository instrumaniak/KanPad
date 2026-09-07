# T002: npm workspace adoption

## Goal

Convert the project from two independent npm packages (backend/, frontend/) to an npm workspaces monorepo with a shared root package.json. This unifies dependency management, eliminates version drift on shared dev tooling, and enables a single `npm install` from the root.

## Depends On

(None)

## Phase:

1

## Critical:

Yes

## Spec References

- _bmad-output/planning-artifacts/architecture.md

## Files to Create/Modify

**Create:**
- `package.json` (root — new)

**Modify:**
- `backend/package.json` — rename `name` to `@kanbanflow/backend`, remove shared devDeps that move to root
- `frontend/package.json` — rename `name` to `@kanbanflow/frontend`, remove shared devDeps that move to root

**Delete:**
- `backend/package-lock.json` (replaced by root lock file)
- `frontend/package-lock.json` (replaced by root lock file)

## Implementation Steps

### Step 1: Create root `package.json` with workspaces config

Create `package.json` at project root:

```json
{
  "name": "kanbanflow",
  "private": true,
  "workspaces": [
    "backend",
    "frontend"
  ],
  "scripts": {
    "dev:backend": "npm run start:dev --workspace=backend",
    "dev:frontend": "npm run dev --workspace=frontend",
    "dev": "concurrently \"npm run dev:backend\" \"npm run dev:frontend\"",
    "build": "npm run build --workspace=frontend && npm run build --workspace=backend",
    "test": "npm run test --workspace=backend && npm run test --workspace=frontend",
    "test:backend": "npm run test --workspace=backend",
    "test:frontend": "npm run test --workspace=frontend",
    "test:e2e": "npm run test:e2e --workspace=backend && npm run test:e2e --workspace=frontend",
    "lint": "npm run lint --workspace=backend && npm run lint --workspace=frontend",
    "format": "prettier --write \"backend/src/**/*.ts\" \"backend/test/**/*.ts\" \"frontend/src/**/*.{ts,tsx}\""
  },
  "devDependencies": {
    "concurrently": "^9.1.0"
  }
}
```

Note: `build:release` and `release:verify` are NOT included here — they are added in T003 (streamlined release workflow).

### Step 2: Move shared dev dependencies to root

These packages are duplicated between backend and frontend with divergent versions. Move them to root `devDependencies` and remove from both workspace package.json files:

| Package | Backend version | Frontend version | Root version (latest) |
|---------|----------------|-----------------|----------------------|
| `@eslint/js` | ^9.18.0 | ^9.39.4 | ^9.39.4 |
| `eslint` | ^9.18.0 | ^9.39.4 | ^9.39.4 |
| `globals` | ^16.0.0 | ^17.4.0 | ^17.4.0 |
| `typescript` | ^5.7.3 | ~5.9.3 | ~5.9.3 |
| `typescript-eslint` | ^8.20.0 | ^8.57.0 | ^8.57.0 |
| `@types/node` | ^22.10.7 | ^24.12.0 | ^24.12.0 |
| `prettier` | ^3.4.2 | — | ^3.4.2 |

After moving to root, these packages will be hoisted to the root `node_modules` and available to both workspaces. Remove them from `backend/package.json` and `frontend/package.json` devDependencies.

### Step 3: Update workspace package.json names

**backend/package.json:**
- Change `"name": "backend"` → `"name": "@kanbanflow/backend"`
- Remove devDeps that moved to root (eslint, typescript, @eslint/js, globals, typescript-eslint, @types/node, prettier)
- Keep backend-specific devDeps: `@nestjs/cli`, `@nestjs/schematics`, `@nestjs/testing`, `@types/express`, `@types/jest`, `@types/supertest`, `jest`, `ts-jest`, `ts-loader`, `ts-node`, `tsconfig-paths`, `source-map-support`, `supertest`

**frontend/package.json:**
- Change `"name": "frontend"` → `"name": "@kanbanflow/frontend"`
- Remove devDeps that moved to root (eslint, @eslint/js, globals, typescript, typescript-eslint, @types/node)
- Keep frontend-specific devDeps: `@playwright/test`, `@rolldown/plugin-babel`, `@testing-library/*`, `@types/dompurify`, `@types/react`, `@types/react-dom`, `@vitejs/plugin-react`, `babel-plugin-react-compiler`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`, `jsdom`, `vite`, `vitest`

### Step 4: Install and deduplicate

```bash
# Remove old lock files and node_modules
rm -rf backend/node_modules frontend/node_modules backend/package-lock.json frontend/package-lock.json

# Install from root — npm will resolve and hoist shared deps
npm install

# Verify hoisting worked
ls node_modules/typescript/package.json  # should exist at root
ls node_modules/eslint/package.json      # should exist at root
```

### Step 5: Verify workspace setup

```bash
# Verify install
npm install

# Verify backend builds
npm run build --workspace=backend

# Verify frontend builds
npm run build --workspace=frontend

# Verify backend tests
npm run test --workspace=backend

# Verify frontend tests
npm run test --workspace=frontend

# Verify lint
npm run lint --workspace=backend
npm run lint --workspace=frontend

# Verify dev scripts from root
npm run dev:backend  # should start NestJS
npm run dev:frontend # should start Vite
```

## Constraints

- Must not break the backend's webpack build or NestJS compilation
- Must not break the frontend's Vite build or TypeScript compilation
- The `.prettierrc` at root remains the single source of truth for formatting
- Each workspace retains its own tsconfig.json (different module systems: `nodenext` vs `bundler`)
- `npm install` from root must work without requiring `cd` into subdirectories
- Release workflow changes are NOT part of this task — see T003

## Acceptance Criteria

- [ ] Root `package.json` exists with `"workspaces": ["backend", "frontend"]`
- [ ] Single `npm install` from root installs all dependencies for both packages
- [ ] `npm run dev` from root starts both backend and frontend concurrently
- [ ] `npm run test` from root runs tests for both packages
- [ ] No duplicate `node_modules` at `backend/node_modules` or `frontend/node_modules` (hoisted to root)
- [ ] Shared dev tooling (eslint, typescript, prettier) has consistent versions across both packages
- [ ] `npm run lint` and `npm run format` work from root
- [ ] TypeScript compilation works for both backend (`nest build`) and frontend (`tsc -b && vite build`)

## Notes

(filled in during/after implementation)
