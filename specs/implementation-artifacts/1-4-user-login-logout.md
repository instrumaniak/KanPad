# Story 1.4: User Login/Logout

Status: done

## Story

As a registered user,
I want to log in with my credentials and log out when done,
so that I can securely access my workspace.

## Acceptance Criteria

1. **Given** I am on the login page, **When** I enter valid email and password, **Then** I am authenticated and redirected to the projects page
2. **And** a session cookie is set (httpOnly, SameSite, 24h maxAge)
3. **Given** I am logged in, **When** I click the logout button, **Then** my session is cleared and I am redirected to the login page
4. **Given** I enter invalid credentials, **When** I submit the login form, **Then** I see an error message "Invalid email or password"
5. **And** rate limiting prevents brute force attempts (5 req/min)
6. **Given** I am not logged in, **When** I try to access a protected route, **Then** I am redirected to the login page
7. **Given** I enter an invalid email format or empty password, **When** I submit the form, **Then** I see inline validation errors before the request is sent

## Tasks / Subtasks

- [x] Task 1: Create login DTO (AC: 1, 4, 7)
  - [x] Create `backend/src/auth/dto/login.dto.ts` with `email` (email format, required) and `password` (required) validation
  - [x] Use `class-validator` decorators consistent with `register.dto.ts` pattern

- [x] Task 2: Implement login logic in AuthService (AC: 1, 2, 4)
  - [x] Add `login(dto: LoginDto)` method to `backend/src/auth/auth.service.ts`
  - [x] Call `UsersService.findByEmail(dto.email)` — throw `UnauthorizedException` with "Invalid email or password" if not found
  - [x] Compare password with `bcrypt.compare()` — throw `UnauthorizedException` with "Invalid email or password" if mismatch
  - [x] Set session cookie: `req.session.userId = user.id`, `req.session.email = user.email`, `req.session.role = user.role`
  - [x] Return `{ data: { id, email, role }, message: "Login successful" }`
  - [x] Use generic error message for both "user not found" and "wrong password" — never reveal which field is wrong

- [x] Task 3: Implement logout logic in AuthService (AC: 3)
  - [x] Add `logout(req)` method to `backend/src/auth/auth.service.ts`
  - [x] Call `req.session.destroy()` to clear the session
  - [x] Clear the session cookie via `res.clearCookie('connect.sid')`
  - [x] Return `{ message: "Logout successful" }`

- [x] Task 4: Add login and logout endpoints to AuthController (AC: 1, 3, 5)
  - [x] Add `POST /api/auth/login` — accepts `LoginDto`, calls `AuthService.login()`
  - [x] Add `POST /api/auth/logout` — calls `AuthService.logout()`
  - [x] Apply `@Throttle({ default: { limit: 5, ttl: 60000 } })` to login endpoint (same rate as register)
  - [x] Add Swagger decorators: `@ApiOperation`, `@ApiResponse` consistent with register endpoint

- [x] Task 5: Create session guard for protected routes (AC: 6)
  - [x] Create `backend/src/auth/guards/session.guard.ts` implementing `CanActivate`
  - [x] Check `req.session.userId` exists — return `true` if authenticated, throw `UnauthorizedException` if not
  - [x] Create `backend/src/auth/decorators/current-user.decorator.ts` to extract user from session
  - [x] Guard is NOT applied in this story (wiring to protected routes happens in later stories) — just create it

- [x] Task 6: Create frontend login form (AC: 1, 4, 7)
  - [x] Create `frontend/src/features/auth/login-form.tsx`
    - Email field: shadcn `Input` with `type="email"` + inline validation
    - Password field: shadcn `Input` with `type="password"` (no strength check on login — that's registration-only)
    - Submit button: shadcn `Button` with loading spinner state
    - Validation errors: inline `<p>` below each field with `text-destructive` class
    - Link to register page: "Don't have an account? Register"
  - [x] Use same layout/pattern as `register-form.tsx`

- [x] Task 7: Wire up auth provider with login method (AC: 1, 2, 3)
  - [x] Update `frontend/src/features/auth/auth.api.ts`:
    - Add `loginApi(data)` → `POST /api/auth/login`
    - Verify `logoutApi()` → `POST /api/auth/logout` exists (already stubbed in Story 1.3)
  - [x] Update `frontend/src/features/auth/use-auth.ts`:
    - Add `login` to the returned hook object
    - Add `login` mutation in the provider that calls `loginApi`, updates user state, and navigates to `/projects`
  - [x] Update `frontend/src/features/auth/login-form.tsx` to use `useAuth().login` and `useToast()`
  - [x] Wire `/login` route in `App.tsx` — replace placeholder with `LoginForm` component

- [x] Task 8: Add tests (AC: all)
  - [x] Create `backend/src/auth/auth.service.spec.ts` additions:
    - Test: login with valid credentials returns user and sets session
    - Test: login with wrong password throws UnauthorizedException
    - Test: login with non-existent email throws UnauthorizedException
    - Test: logout destroys session
  - [x] Create `backend/src/auth/auth.controller.spec.ts` additions:
    - Test: POST /api/auth/login returns correct response on success
    - Test: POST /api/auth/login returns 401 on invalid credentials
    - Test: POST /api/auth/logout returns success message
  - [x] Create `frontend/src/features/auth/login-form.test.tsx`:
    - Test: renders form with email and password fields
    - Test: shows validation errors for invalid email
    - Test: shows validation errors for empty password
    - Test: submits on valid input

- [x] Task 9: Update session check endpoint (AC: 6)
  - [x] Verify `GET /api/auth/me` works correctly with session set from login (should already work from Story 1.3)
  - [x] Add Swagger decorator to `/me` endpoint if missing

## Dev Notes

### API Contract

**`POST /api/auth/login`**

Request:
```typescript
interface LoginRequest {
  email: string;     // valid email format, max 255 chars
  password: string;  // required, min 1 char (no strength check on login)
}
```

Success Response (200):
```typescript
{
  data: {
    id: number;
    email: string;
    role: string;
  },
  message: "Login successful"
}
```

Error Responses:
- 400: `{ statusCode: 400, message: ["email must be an email", "password should not be empty"], error: "Bad Request" }`
- 401: `{ statusCode: 401, message: "Invalid email or password", error: "Unauthorized" }`
- 429: `{ statusCode: 429, message: "Too Many Requests", error: "Too Many Requests" }`

**`POST /api/auth/logout`**

Success Response (200):
```typescript
{
  message: "Logout successful"
}
```

### Session Data Shape

The session stores the following after successful login (same as registration):
```typescript
interface SessionData {
  userId: number;
  email: string;
  role: string;  // 'user' by default
}
```
This is read by `GET /api/auth/me` and cleared by `POST /api/auth/logout`.
[Source: architecture.md#Authentication Flow]

### Previous Story Intelligence

**From Story 1.3 (User Registration) — CRITICAL CONTEXT:**
- `AuthService` exists at `backend/src/auth/auth.service.ts` with `register()` and `registrationEnabled` flag
- `AuthController` exists at `backend/src/auth/auth.controller.ts` with `POST /api/auth/register` and `GET /api/auth/me`
- `UsersService` exists at `backend/src/users/users.service.ts` with `findByEmail()` and `findById()` methods
- Session middleware configured in `app.module.ts` via `express-session` — cookie: `httpOnly: true`, `sameSite: 'lax'`, `secure: process.env.NODE_ENV === 'production'`, `maxAge: 86400000`
- Rate limiting: `@nestjs/throttler` configured, 5 req/min on auth endpoints
- `register.dto.ts` exists with class-validator decorators — use same pattern for `login.dto.ts`
- Frontend: `auth-provider.tsx` context exists, `useAuth()` hook returns `{ user, isLoading, register, logout }` — add `login`
- Frontend: `auth.api.ts` has `registerApi()`, `meApi()`, `logoutApi()` — add `loginApi()`
- Frontend: `App.tsx` has `/login` route rendering placeholder — replace with `LoginForm`
- Tests: Jest (backend), Vitest (frontend) — co-located `.spec.ts` and `.test.tsx`

**From Story 1.2 (Database Schema Setup):**
- User entity at `backend/src/users/entities/user.entity.ts`: id (PK), email (unique, 255), password (255, bcrypt hash), role (default 'user'), created_at, updated_at
- TypeORM with `synchronize: false`, MySQL 8.x

**From Story 1.1 (Project Initialization):**
- Monorepo: `frontend/` and `backend/` at project root
- Frontend: Vite + React + TypeScript with shadcn/ui
- Backend: NestJS CLI scaffolded
- Shared `.prettierrc`: semi, singleQuote, trailingComma: "all", printWidth: 100, tabWidth: 2

**Patterns established:**
- Backend files: `kebab-case` (e.g., `login.dto.ts`, `session.guard.ts`)
- Frontend files: `kebab-case` (e.g., `login-form.tsx`)
- NestJS modules use `@Module()` with DI via constructor
- Tests co-located: `.spec.ts` (backend), `.test.tsx` (frontend)
- DTOs use `class-validator` decorators for input validation

### Architecture Compliance

**Authentication Flow (from architecture.md):**
```
Login: POST /api/auth/login → validate credentials → set httpOnly cookie → return user data
Logout: POST /api/auth/logout → clear cookie
Session check: GET /api/auth/me → returns current user or 401
```
[Source: architecture.md#Authentication Flow]

**Auth Module Structure (from architecture.md):**
```
backend/src/auth/
├── auth.module.ts          (exists)
├── auth.controller.ts      (exists — add login/logout endpoints)
├── auth.service.ts         (exists — add login/logout methods)
├── auth.controller.spec.ts (exists — add login/logout tests)
├── auth.service.spec.ts    (exists — add login/logout tests)
├── dto/
│   ├── login.dto.ts        (NEW)
│   └── register.dto.ts     (exists)
├── guards/
│   └── session.guard.ts    (NEW)
└── decorators/
    └── current-user.decorator.ts (NEW)
```
[Source: architecture.md#Complete Project Directory Structure]

**Security Requirements:**
- Passwords hashed with bcrypt — compare with `bcrypt.compare()` (NFR6)
- Session tokens: cookie-based httpOnly with SameSite CSRF protection (NFR7, NFR8)
- Rate limiting on login endpoint via `@nestjs/throttler` — 5 req/min (NFR10)
- Generic error message "Invalid email or password" — never reveal whether email exists or password is wrong
- Never return raw database errors to frontend — use NestJS exceptions
[Source: architecture.md#Authentication & Security]

**Cookie Configuration (already set in Story 1.3):**
```typescript
{
  httpOnly: true,
  sameSite: 'lax',    // CSRF protection
  secure: process.env.NODE_ENV === 'production',
  maxAge: 86400000,   // 24 hours in ms
}
```
[Source: architecture.md#Authentication & Security]

### API Naming Conventions

- Endpoints: RESTful `kebab-case` (e.g., `/api/auth/login`)
- Controllers: `AuthController` (exists)
- Services: `AuthService` (exists)
- DTOs: `login.dto.ts`
- Guards: `session.guard.ts`
[Source: architecture.md#Implementation Patterns - Naming Patterns]

### API Response Format

```typescript
// Success response
{ data: T, message?: string }

// Error response
{ statusCode: number, message: string | string[], error: string }
```
[Source: architecture.md#Implementation Patterns - Format Patterns]

### Error Handling

- Backend: Use NestJS built-in exceptions (`UnauthorizedException` for invalid credentials, `BadRequestException` for validation)
- Frontend: React Query `onError` handlers display toast notifications via `useToast()`
- Form validation errors shown inline below fields with `text-destructive` class
[Source: architecture.md#Implementation Patterns - Process Patterns - Error Handling]

### Frontend Feature Organization

```
frontend/src/features/auth/
├── register-form.tsx       (exists)
├── register-form.test.tsx  (exists)
├── login-form.tsx          (NEW)
├── login-form.test.tsx     (NEW)
├── auth-provider.tsx       (exists — add login)
├── use-auth.ts             (exists — add login)
├── auth.api.ts             (exists — add loginApi)
```
Features are self-contained — each owns its components, hooks, and API calls.
[Source: architecture.md#Project Structure & Boundaries - Component Boundaries]

### shadcn/ui Component Mapping

| Form Element | shadcn/ui Component | Notes |
|---|---|---|
| Email input | `Input` with `type="email"` | shadcn/ui Input primitive |
| Password input | `Input` with `type="password"` | shadcn/ui Input primitive |
| Submit button | `Button` with `disabled` + loading state | shadcn/ui Button, use `Loader2` icon for spinner |
| Validation error text | `<p className="text-sm text-destructive">` | Tailwind classes, not a separate component |
| Error toast | `useToast()` hook | shadcn/ui toast hook, NOT a custom component |

Do NOT build custom Input, Button, or Toast components — use shadcn/ui from `components/ui/`.
[Source: ux-design-specification.md#Component Strategy - Design System Components]

### Critical Anti-Patterns

- ❌ NEVER use `synchronize: true` in TypeORM
- ❌ NEVER return raw database errors to frontend — use NestJS exceptions
- ❌ NEVER use `any` type — strict TypeScript
- ❌ NEVER reveal whether email exists or password is wrong — always use "Invalid email or password"
- ❌ NEVER build custom UI components when shadcn/ui equivalent exists
- ❌ NEVER fetch data without React Query (no raw `fetch` in components)
- ❌ NEVER modify shadcn/ui components directly in `components/ui/`
- ❌ NEVER add password strength validation to login form (registration-only feature)

### Project Structure After This Story

```
backend/src/auth/
├── auth.module.ts              (no changes)
├── auth.controller.ts          (modified — add login/logout endpoints)
├── auth.controller.spec.ts     (modified — add login/logout tests)
├── auth.service.ts             (modified — add login/logout methods)
├── auth.service.spec.ts        (modified — add login/logout tests)
├── dto/
│   ├── login.dto.ts            (NEW)
│   └── register.dto.ts         (no changes)
├── guards/
│   └── session.guard.ts        (NEW)
└── decorators/
    └── current-user.decorator.ts (NEW)

frontend/src/features/auth/
├── register-form.tsx           (no changes)
├── register-form.test.tsx      (no changes)
├── login-form.tsx              (NEW)
├── login-form.test.tsx         (NEW)
├── auth-provider.tsx           (modified — add login)
├── use-auth.ts                 (modified — add login)
├── auth.api.ts                 (modified — add loginApi)
```

### References

- [Source: architecture.md#Authentication Flow] — Auth endpoint patterns
- [Source: architecture.md#Authentication & Security] — Security decisions and cookie config
- [Source: architecture.md#Complete Project Directory Structure] — File locations
- [Source: architecture.md#Implementation Patterns - Naming Patterns] — Naming conventions
- [Source: architecture.md#Implementation Patterns - Error Handling] — Error patterns
- [Source: architecture.md#Implementation Patterns - Format Patterns] — API response format
- [Source: project-context.md#Technology Stack & Versions] — Stack details
- [Source: project-context.md#Framework-Specific Rules] — NestJS and React rules
- [Source: project-context.md#Critical Implementation Rules] — Language rules
- [Source: ux-design-specification.md#Component Strategy] — UI components to use
- [Source: epics.md#Story 1.4] — Original story acceptance criteria
- [Source: prd.md#User Authentication] — FR2 (login), FR3 (logout)

## Dev Agent Record

### Agent Model Used

OpenCode Dev Agent (Story 1.4 Implementation)

### Debug Log References

No significant debugging required. Implementation followed established patterns from Story 1.3.

### Completion Notes List

- Created `LoginDto` with email and password validation using class-validator
- Added `login()` method to `AuthService` with bcrypt password comparison and generic error messages
- Added `logout()` method to `AuthService` with session destroy
- Created `POST /api/auth/login` endpoint with rate limiting (5 req/min) and Swagger decorators
- Updated `POST /api/auth/logout` endpoint to use AuthService.logout() and return success message
- Created `SessionGuard` for protecting routes (wiring in later stories)
- Created `CurrentUser` decorator for extracting user from session
- Created `LoginForm` component with inline validation and link to register page
- Added `loginApi()` to auth.api.ts
- Added `login` mutation to AuthProvider
- Updated App.tsx to use LoginForm instead of placeholder
- Added comprehensive tests for login/logout in both backend and frontend
- All acceptance criteria satisfied

### File List

**New Files:**
- backend/src/auth/dto/login.dto.ts
- backend/src/auth/guards/session.guard.ts
- backend/src/auth/decorators/current-user.decorator.ts
- frontend/src/features/auth/login-form.tsx
- frontend/src/features/auth/login-form.test.tsx
- frontend/src/components/ui/dropdown-menu.tsx

**Modified Files:**
- backend/src/auth/auth.service.ts (added login/logout methods)
- backend/src/auth/auth.controller.ts (added login endpoint, updated logout with clearCookie + throttle)
- backend/src/auth/auth.service.spec.ts (added login/logout tests)
- backend/src/auth/auth.controller.spec.ts (added login/logout tests)
- frontend/src/features/auth/auth.api.ts (added loginApi with network error handling)
- frontend/src/features/auth/auth-provider.tsx (added login mutation, optimistic user data)
- frontend/src/App.tsx (wired LoginForm, user dropdown, / redirect)
