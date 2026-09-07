# T003: Streamlined release workflow

## Goal

Move the release build output from `backend/release/` to the project root `release/` directory. This makes the deployable artifact live at the top level alongside the source code, and provides a clean root-level `npm run build:release` command that orchestrates the entire build pipeline.

## Depends On

T002 (npm workspace adoption) — root `package.json` with workspaces must exist first.

## Phase:

2

## Critical:

Yes

## Spec References

- _bmad-output/planning-artifacts/architecture.md
- specs/tasks/T002-npm-workspace.md

## Current State Analysis

**Current release flow (all inside backend/):**

```
cd backend
npm run build:release
→ rm -rf release/                    # clears backend/release/
→ nest build --webpack               # outputs app.js, common.js into backend/release/
→ prepare-release.js
    → builds frontend (npm run build in frontend/)
    → copies frontend/dist → backend/release/public/
```

**Result:** `backend/release/` contains deployable artifact (`app.js`, `migrate.js`, `create-admin.js`, `common.js`, `public/`)

**Problems:**
- Release directory buried inside `backend/` — unintuitive for deployment
- `prepare-release.js` hardcodes relative paths (`../../../frontend`)
- No root-level entry point for the release workflow

## Files to Create/Modify

**Modify:**
- `backend/webpack.config.js` — change output path from `backend/release/` to root `release/`
- `backend/src/scripts/prepare-release.js` — point releaseDir to root, use workspace commands
- `backend/src/scripts/verify-release.js` — point releaseDir to root
- `backend/package.json` — update release scripts (`release:backend`, `build:release`)
- Root `package.json` (from T002) — update `build:release` script to orchestrate from root
- `README.md` — update deployment section

## Implementation Steps

### Step 1: Update `backend/webpack.config.js` — output to root release/

Change the webpack output path from `backend/release/` to the project root `release/`:

```js
// Before:
output: {
  ...options.output,
  path: path.join(__dirname, 'release'),
  filename: '[name].js',
  chunkFilename: '[name].js',
},

// After:
output: {
  ...options.output,
  path: path.resolve(__dirname, '../release'),
  filename: '[name].js',
  chunkFilename: '[name].js',
},
```

### Step 2: Update `backend/package.json` release scripts

```json
"scripts": {
  "release:backend": "nest build --webpack",
  "release:install": "node src/scripts/prepare-release.js",
  "build:release": "rm -rf ../release/ && npm run build --workspace=frontend && nest build --webpack && npm run release:install",
  "migrate:release": "node ../release/migrate.js",
  "create-admin:release": "node ../release/create-admin.js",
  "test:release": "node src/scripts/verify-release.js"
}
```

- `release:backend` — new script callable from root workspace (just webpack, no rm/cleanup)
- `build:release` — updated to point `rm -rf` at `../release/` instead of `release/`
- `migrate:release` and `create-admin:release` — updated to point at `../release/`

### Step 3: Update `backend/src/scripts/prepare-release.js` — point to root release/

```js
// Before:
const releaseDir = path.resolve(__dirname, '../../release');
const frontendDir = path.resolve(__dirname, '../../../frontend');

// After:
const releaseDir = path.resolve(__dirname, '../../../release');
const frontendDir = path.resolve(__dirname, '../../../frontend');
```

Also update the frontend build command to use workspace-aware invocation from root:

```js
// Before:
const frontendBuild = spawnSync('npm', ['run', 'build'], {
  cwd: frontendDir,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

// After:
const rootDir = path.resolve(__dirname, '../../../..');
const frontendBuild = spawnSync('npm', ['run', 'build', '--workspace=frontend'], {
  cwd: rootDir,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});
```

### Step 4: Update `backend/src/scripts/verify-release.js` — point to root release/

```js
// Before:
const releaseDir = path.resolve(__dirname, '../../release');

// After:
const releaseDir = path.resolve(__dirname, '../../../release');
```

### Step 5: Update root `package.json` build:release script

```json
"build:release": "rm -rf release/ && npm run build --workspace=frontend && npm run release:backend --workspace=backend"
```

Flow:
1. `rm -rf release/` — clear root release directory
2. `npm run build --workspace=frontend` — build frontend SPA
3. `npm run release:backend` — runs `nest build --webpack`, outputs backend JS to root `release/`
4. `prepare-release.js` (called by backend webpack) copies `frontend/dist/` → `release/public/`

### Step 6: Update README.md deployment section

**Deployment** section:
```bash
# From project root
npm run build:release

# This builds both frontend and backend into the release/ directory at project root
```

**Release folder** section:
```bash
cd release
touch .env
node migrate.js
node app.js
```

### Step 7: Verify the release build

```bash
# From project root
npm run build:release

# Verify all required artifacts exist
ls release/app.js release/migrate.js release/create-admin.js release/common.js
ls release/public/index.html

# Run release verification
npm run release:verify
```

## Constraints

- Must produce a `release/` directory at project root with the same structure as before
- `release/` must contain: `app.js`, `migrate.js`, `create-admin.js`, `common.js`, `public/`
- `release/public/index.html` must exist and be non-empty
- Must not break the backend's webpack build or NestJS compilation
- Must not break the frontend's Vite build
- `release/` is already in `.gitignore` — no change needed
- The `prepare-release.js` script must still be called after webpack build (it copies frontend assets)

## Acceptance Criteria

- [ ] `npm run build:release` from root produces `KanbanFlow/release/` with all required artifacts
- [ ] `release/app.js`, `release/migrate.js`, `release/create-admin.js`, `release/common.js` exist and are non-empty
- [ ] `release/public/index.html` exists and is non-empty
- [ ] No `backend/release/` directory is created (webpack output moved to root)
- [ ] `npm run release:verify` from root validates the release directory
- [ ] `npm run migrate:release` and `npm run create-admin:release` still work from backend workspace
- [ ] README.md deployment instructions are accurate

## Notes

(filled in during/after implementation)
