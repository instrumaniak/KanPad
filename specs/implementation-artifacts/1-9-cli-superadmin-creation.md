# Story 1.9: CLI Superadmin Creation

Status: done

---

## Story

As an administrator,
I want a CLI script to create the initial superadmin user,
so that I can set up admin access before opening registration.

## Acceptance Criteria

1. Given the application is deployed, when I run `npm run create-admin` (no arguments), then interactive prompts ask for both email and password (password input will be masked)
2. Given the script is executed with only --email flag, when I run `npm run create-admin -- --email=admin@example.com`, then interactive prompt asks for password only (password input will be masked)
3. Given the script is executed with both --email and --password flags, when I run `npm run create-admin -- --email=admin@example.com --password=securePass123`, then admin user is created without prompts
4. Given the script is executed, when invalid email format is provided (via flag or prompt), then an error message explains the validation failure
5. Given the script is executed, when weak password is provided (via flag or prompt), then an error message explains password strength requirements (min 8 chars, at least one letter and one number OR special character)
6. Given an admin already exists, when I try to create another admin, then an error message prevents duplicate admin creation
7. Given successful admin creation, when the script completes, then a success message confirms admin creation with the email address
8. Given .env file is missing or database connection fails, when I run the script, then a clear error message explains the issue
9. Given invalid arguments are provided, when I run the script, then a usage help message is displayed
10. Given I press Ctrl+C during interactive prompt, then the script exits cleanly without partial state

## Tasks / Subtasks

- [x] Task 1: Create CLI script entry point (AC: #1, #2, #3)
  - [x] Subtask 1.1: Add `create-admin` script to `backend/package.json`:
    ```json
    "create-admin": "ts-node -r tsconfig-paths/register src/scripts/create-admin.ts"
    ```
  - [x] Subtask 1.2: Create `src/scripts/create-admin.ts` CLI entry point
  - [x] Subtask 1.3: Install `inquirer@^8.x` (last CJS-compatible version) for interactive prompts
- [x] Task 2: Implement argument parsing (AC: #9)
  - [x] Subtask 2.1: Accept --email and --password flags
  - [x] Subtask 2.2: Display usage help when --help is used or arguments missing
- [x] Task 3: Implement interactive prompts (AC: #1, #2, #3, #10)
  - [x] Subtask 3.1: Prompt for email if not provided via flag
  - [x] Subtask 3.2: Prompt for password if not provided via flag (masked input)
  - [x] Subtask 3.3: Handle Ctrl+C for clean exit
- [x] Task 4: Implement email validation (AC: #4)
  - [x] Subtask 4.1: Add email format validation using regex
  - [x] Subtask 4.2: Return clear error message for invalid email
  - [x] Subtask 4.3: Reprompt on invalid input in interactive mode
- [x] Task 5: Implement password validation (AC: #5)
  - [x] Subtask 5.1: **CRITICAL**: Use IDENTICAL regex from `register.dto.ts` for consistency. The existing regex requires min 8 chars with "a number OR special character" (no letter requirement). Use exactly: `/^(?=.*[a-zA-Z])(?=.*\d|[!@#$%^&*])[a-zA-Z\d!@#$%^&*]{8,}$/`
  - [x] Subtask 5.2: Return clear error message for weak password
  - [x] Subtask 5.3: Reprompt on invalid input in interactive mode
- [x] Task 6: Implement admin user creation (AC: #3, #7)
  - [x] Subtask 6.1: Use bcrypt to hash password with explicitly 10 rounds: `await bcrypt.hash(password, 10)`
  - [x] Subtask 6.2: **CRITICAL**: Insert directly via TypeORM repository with `role: 'admin'` explicitly set. The `UsersService.create()` always defaults to `'user'` role via `CreateUserDto` which has no role field. Use: `repo.insert({ email, password: hashedPassword, role: 'admin' })`
  - [x] Subtask 6.3: **CRITICAL**: Initialize DataSource explicitly:
    ```typescript
    const dataSource = new DataSource(dataSourceOptions);
    await dataSource.initialize();
    try {
      // DB operations here
    } finally {
      await dataSource.destroy(); // Always cleanup
    }
    ```
- [x] Task 7: Implement duplicate admin prevention (AC: #6)
  - [x] Subtask 7.1: **CRITICAL**: Query repository directly (not via UsersService which has no findByRole method): `const existingAdmin = await repo.findOne({ where: { role: 'admin' } })`
  - [x] Subtask 7.2: Return error if admin already exists
- [x] Task 8: Implement error handling (AC: #8)
  - [x] Subtask 8.1: **CRITICAL**: `dotenv/config` must be the VERY FIRST import in the script (before any other import that reads `process.env`). This matches existing `data-source.ts` pattern:
    ```typescript
    import 'dotenv/config'; // MUST be first import
    import { DataSource } from 'typeorm';
    import bcrypt from 'bcrypt';
    import inquirer from 'inquirer';
    // ... other imports
    ```
  - [x] Subtask 8.2: Handle database connection failures
  - [x] Subtask 8.3: Set proper exit codes (0 success, 1 error)

## Dev Notes

### ⚠️ Security Warnings

**CLI Password Exposure (MAJOR):**
Using `--password` flag exposes the value in:
- Shell history (`~/.bash_history`)
- Process list (`ps aux | grep create-admin`)
- Terminal buffer

**Safer alternatives:**
1. **Interactive mode** (RECOMMENDED): `npm run create-admin` - prompts for masked password
2. **Environment variable**: `ADMIN_PASSWORD=... npm run create-admin -- --email=x`
3. **--password-stdin** (optional enhancement): Read password from stdin to avoid shell history

Document the security risk in README or script help output.

### Architecture Context

This CLI script is a one-time setup tool for administrators to create the first admin user. It supports three usage modes:

**Usage Modes:**

| Mode | Command | Behavior |
|------|---------|----------|
| Interactive | `npm run create-admin` | Prompts for email + password (input masked) |
| Hybrid | `npm run create-admin -- --email=x` | Prompts for password only (input masked) |
| Scripted | `npm run create-admin -- --email=x --password=y` | No prompts |

**Argument Flow:**
```typescript
const args = parseArgs(); // --email, --password, --help

// Fallback chain:
const email = args.email ?? await promptEmail();
const password = args.password ?? await promptPassword({ mask: true });
```

**Error Messages (standard format):**
- Email validation: "Invalid email format. Please provide a valid email address."
- Password validation: "Password must be at least 8 characters and contain at least one letter and one number or special character."
- Duplicate admin: "An admin user already exists. Only one superadmin is allowed."
- Missing .env: "Configuration file (.env) not found. Please ensure .env is configured."
- Database error: "Failed to connect to database. Please check your database configuration."

**Exit Codes:**
- `0` - Success
- `1` - Error (validation failure, database error, etc.)

### Technical Requirements

**Backend Module:** `users` module
- Entity: `user.entity.ts` - `role` is a plain `string` column (NOT an enum). Do NOT attempt to import a non-existent enum. Use literal string `'admin'`.

**Database Connection:**
- Use `data-source.ts` for CLI database connection (same as migration scripts)
- Configure via environment variables from `.env` file

**Dependencies Required:**
- `bcrypt` for password hashing
- `inquirer@^8.x` for interactive prompts (CJS-compatible)
- `dotenv` for loading environment variables
- `tsconfig-paths` for resolving paths in ts-node (install if not present)

### Source Tree Components to Touch

```
backend/
├── package.json                          # Add create-admin script
├── src/
│   ├── data-source.ts                   # Reuse for CLI database connection
│   ├── users/
│   │   └── entities/
│   │       └── user.entity.ts           # Reference for user entity structure
│   └── scripts/
│       └── create-admin.ts               # NEW - CLI script
```

### Testing Standards Summary

**Manual Test Cases:**

**Interactive Mode (no args):**
- [ ] Run with no args → prompts for both email + password
- [ ] Enter invalid email → reprompt with error
- [ ] Enter weak password → reprompt with error
- [ ] Ctrl+C during prompt → clean exit

**Hybrid Mode (partial args):**
- [ ] Run with --email only → prompts for password only
- [ ] Run with --password only → prompts for email only

**Scripted Mode (full args):**
- [ ] Run with --email + --password → no prompts, creates admin
- [ ] Run with --help → displays usage help
- [ ] Run with invalid email format → validation error
- [ ] Run with weak password → validation error

**Edge Cases:**
- [ ] Admin already exists → duplicate prevention error
- [ ] No database connection → clear error
- [ ] Missing .env → clear error

### Project Structure Notes

- Follow backend file naming: `kebab-case` (e.g., `create-admin.ts`)
- Place script in `backend/src/scripts/` directory (consistent with migration scripts)
- Use same Prettier config as rest of backend

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story-1.9-CLI-Superadmin-Creation]
- [Source: _bmad-output/planning-artifacts/architecture.md#Decision-Impact-Analysis]
- [Source: _bmad-output/planning-artifacts/architecture.md#Database-Naming-Conventions]
- [Source: _bmad-output/project-context.md#Critical-Implementation-Rules]

## Dev Agent Record

### Agent Model Used

Claude 3.5 Sonnet (via OpenCode)

### Debug Log References

### Completion Notes List

- Created CLI script `backend/src/scripts/create-admin.ts` with full argument parsing, interactive prompts, email/password validation
- Added `create-admin` npm script to `backend/package.json`
- Installed `inquirer@^8.x` as runtime dependency (and `@types/inquirer` as devDependency)
- Used bcrypt with 10 rounds for password hashing
- Implemented duplicate admin prevention by querying for existing admin role
- Implemented proper error handling with dotenv/config as first import
- All unit tests pass (75 → 103 with new tests)
- Added 28 new tests (23 unit + 5 E2E)

### File List

- `backend/package.json` - Added npm script: `"create-admin": "ts-node -r tsconfig-paths/register src/scripts/create-admin.ts"`, added dependencies: `inquirer@^8.2.7`, `@types/inquirer@^9.0.9`
- `backend/src/scripts/create-admin.ts` - CLI entry point (NEW)
- `backend/src/scripts/create-admin.spec.ts` - Unit tests for validateEmail, validatePassword, parseArgs (23 tests)
- `backend/src/scripts/create-admin.e2e.spec.ts` - E2E tests for CLI execution (5 tests)
- `backend/src/scripts/E2E-TEST-MANUAL.md` - Manual E2E test documentation
- `backend/src/users/entities/user.entity.ts` - Referenced for entity definition
- `backend/src/projects/entities/project.entity.ts` - Referenced for DataSource entities

### Change Log

- Initial implementation: CLI admin creation script with interactive and scripted modes
- Added unit tests: 23 tests covering validation functions and argument parsing
- Added E2E tests: 5 tests covering CLI execution, validation errors, duplicate prevention
