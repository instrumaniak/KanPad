# Story 1.3: User Registration

Status: review

## Story

As a new user,
I want to register with my email and password,
so that I can create an account and start using KanbanFlow.

## Acceptance Criteria

1. **Given** I am on the registration page, **When** I enter a valid email and password, **Then** my account is created and I am automatically logged in
2. **And** the password is hashed with bcrypt before storage
3. **And** a session cookie is set (httpOnly, SameSite)
4. **And** I am redirected to the projects page
5. **Given** I enter an email that already exists, **When** I submit the registration form, **Then** I see an error message "Email already registered"
6. **Given** registration is disabled by admin, **When** I try to access the registration page, **Then** I see a message "Registration is currently closed"
7. **Given** I enter an invalid email or weak password, **When** I submit the form, **Then** I see inline validation errors before the request is sent

### Password Validation Rules

- Minimum 8 characters
- At least 1 number OR 1 special character
- Enforced on both client (inline errors) and server (BadRequestException)

## Tasks / Subtasks

- [x] Task 1: Create Users service and module (AC: 1, 2, 5)
  - [x] Create `backend/src/users/users.module.ts`
  - [x] Create `backend/src/users/users.service.ts` with `findByEmail()` and `create()` methods
  - [x] Create `backend/src/users/users.controller.ts` (stub — auth routes live in AuthController)
  - [x] Export `UsersModule` for use by `AuthModule`
  - [x] This module MUST exist before AuthModule imports it
- [x] Task 2: Create Auth backend module (AC: 1, 2, 3, 4)
  - [x] Create `backend/src/auth/auth.module.ts` — imports `UsersModule`
  - [x] Create `backend/src/auth/auth.service.ts` with `register()` method
  - [x] Create `backend/src/auth/auth.controller.ts` with `POST /api/auth/register` endpoint
  - [x] Create `backend/src/auth/dto/register.dto.ts` with email format + password rules validation
  - [x] Install `bcrypt` and `@types/bcrypt` for password hashing
- [x] Task 3: Implement registration logic (AC: 1, 2, 3, 5)
  - [x] Check for duplicate email via `UsersService.findByEmail()` — throw `ConflictException` with "Email already registered"
  - [x] Hash password with bcrypt (salt rounds: 10) before storing
  - [x] Create user in database via `UsersService.create()`
  - [x] `register()` method sets the session cookie directly (auto-login) — NOT a separate login call
  - [x] Return `{ data: { id, email, role }, message: "Registration successful" }`
- [x] Task 4: Implement session management (AC: 3, 4)
  - [x] Install `cookie-parser` and `express-session` with `@types/express-session`
  - [x] Configure cookie-based session in `app.module.ts` or `main.ts`
  - [x] Cookie options: `httpOnly: true`, `sameSite: 'lax'`, `secure: process.env.NODE_ENV === 'production'`, `maxAge: 86400000` (24h)
  - [x] Session stores: `{ userId: number, email: string, role: string }`
  - [x] On successful registration, set session with user data and return user object
  - [x] Create `GET /api/auth/me` endpoint — reads session, returns user or 401
- [x] Task 5: Implement registration toggle check (AC: 6)
  - [x] Use in-memory boolean `registrationEnabled` in AuthService (MVP: resets on restart, acceptable for single-user self-hosted)
  - [x] Check flag before processing registration — throw `ForbiddenException` with "Registration is currently closed"
  - [x] Admin toggle endpoint added in Story 6.3 (do NOT build admin toggle here)
- [x] Task 6: Create frontend auth feature (AC: 1, 4, 7)
  - [x] Create `frontend/src/features/auth/register-form.tsx`
    - Email field: shadcn `Input` with `type="email"` + email format validation
    - Password field: shadcn `Input` with `type="password"` + strength validation (min 8, 1 number/special)
    - Confirm password field: shadcn `Input` with `type="password"` + match validation
    - Submit button: shadcn `Button` with loading spinner state
    - Validation errors: inline `<p>` below each field with `text-destructive` class
  - [x] Create `frontend/src/features/auth/auth-provider.tsx` — React context for auth state
  - [x] Create `frontend/src/features/auth/use-auth.ts` — `useAuth()` hook returns `{ user, isLoading, register, logout }`
  - [x] Create `frontend/src/features/auth/auth.api.ts` — API calls via React Query mutations
    - `registerApi(data)` → `POST /api/auth/register`
    - `meApi()` → `GET /api/auth/me`
    - `logoutApi()` → `POST /api/auth/logout`
  - [x] On success, redirect to `/projects` using React Router `useNavigate()`
  - [x] On error, display toast via shadcn `useToast()` with server error message
- [x] Task 7: Wire up routing and API proxy (AC: 4)
  - [x] Configure Vite proxy in `frontend/vite.config.ts` to forward `/api/*` to `http://localhost:3000`
  - [x] Add `/register` route in `App.tsx` — renders `RegisterForm`
  - [x] Add `/login` route stub in `App.tsx` — renders placeholder (full impl in Story 1.4)
  - [x] Add `/forgot-password` route stub in `App.tsx` — renders placeholder
- [x] Task 8: Add rate limiting to auth endpoints (AC: all)
  - [x] Install `@nestjs/throttler` if not already installed
  - [x] Configure `ThrottlerModule.forRoot()` in `app.module.ts` — default: 10 req/min
  - [x] Apply `@Throttle({ default: { limit: 5, ttl: 60000 } })` to registration endpoint
- [x] Task 9: Add Swagger decorators to auth endpoints (AC: all)
  - [x] Add `@ApiTags('auth')` to AuthController
  - [x] Add `@ApiOperation({ summary: 'Register new user' })` + `@ApiResponse()` to register endpoint
  - [x] Add `@ApiOperation({ summary: 'Get current user' })` + `@ApiResponse()` to `/api/auth/me`
  - [x] Use same Swagger decorator pattern as any existing controllers from story 1.2
- [x] Task 10: Write tests (AC: all)
  - [x] Create `backend/src/auth/auth.service.spec.ts` — test: registration success, duplicate email throws ConflictException, password is hashed (not plain text), session is set
  - [x] Create `backend/src/auth/auth.controller.spec.ts` — test: register endpoint returns correct response, validation errors return 400
  - [x] Create `backend/src/users/users.service.spec.ts` — test: findByEmail returns user, create persists to DB
  - [x] Create `frontend/src/features/auth/register-form.test.tsx` — test: renders form, shows validation errors for weak password, shows validation errors for invalid email, submits on valid input

## Dev Notes

### API Contract

**`POST /api/auth/register`**

Request:
```typescript
interface RegisterRequest {
  email: string;      // valid email format, max 255 chars
  password: string;   // min 8 chars, at least 1 number or special char
}
```

Success Response (201):
```typescript
{
  data: {
    id: number;
    email: string;
    role: string;
  },
  message: "Registration successful"
}
```

Error Responses:
- 400: `{ statusCode: 400, message: ["email must be an email", "password is too short"], error: "Bad Request" }`
- 409: `{ statusCode: 409, message: "Email already registered", error: "Conflict" }`
- 403: `{ statusCode: 403, message: "Registration is currently closed", error: "Forbidden" }`
- 429: `{ statusCode: 429, message: "Too Many Requests", error: "Too Many Requests" }`

**`GET /api/auth/me`**

Success Response (200):
```typescript
{
  data: {
    id: number;
    email: string;
    role: string;
  }
}
```

Error Response (401):
```typescript
{ statusCode: 401, message: "Unauthorized", error: "Unauthorized" }
```

### Session Data Shape

The session stores the following after successful registration:
```typescript
interface SessionData {
  userId: number;
  email: string;
  role: string;  // 'user' by default
}
```
This is read by `GET /api/auth/me` and cleared by `POST /api/auth/logout` (Story 1.4).

### Previous Story Intelligence

**From Story 1.2 (Database Schema Setup):**
- User entity exists at `backend/src/users/entities/user.entity.ts` with: id (PK), email (unique, 255), password (255, stores bcrypt hash), role (default 'user'), created_at, updated_at
- Project entity exists at `backend/src/projects/entities/project.entity.ts`
- ConfigModule configured in `app.module.ts` with validation
- TypeORM configured with `synchronize: false`, MySQL 8.x
- `data-source.ts` exists for CLI migrations
- Migration scripts in `backend/package.json`: `migration:run`, `migration:generate`, etc.
- `.env` has DB_HOST, DB_PORT, DB_USERNAME, DB_PASSWORD, DB_NAME, NODE_ENV

**From Story 1.1 (Project Initialization):**
- Monorepo structure: `frontend/` and `backend/` at project root
- Frontend: Vite + React + TypeScript with shadcn/ui initialized
- Backend: NestJS CLI scaffolded
- Root `.prettierrc` configured (semi, singleQuote, trailingComma: "all", printWidth: 100, tabWidth: 2)
- TypeScript strict mode in both projects

**Patterns established:**
- Backend files: `kebab-case` (e.g., `board.entity.ts`, `create-board.dto.ts`)
- Frontend files: `kebab-case` (e.g., `register-form.tsx`)
- NestJS modules use `@Module()` with DI via constructor
- Tests co-located: `.spec.ts` (backend), `.test.tsx` (frontend)
- Entity properties use `!` definite assignment assertions for strict TS

### Architecture Compliance

**Authentication Flow (from architecture.md):**
```
Register: POST /api/auth/register → creates user → auto-login → sets httpOnly cookie → returns user data
Login: POST /api/auth/login → sets httpOnly cookie → returns user data
Logout: POST /api/auth/logout → clears cookie
Session check: GET /api/auth/me → returns current user or 401
```
[Source: architecture.md#Authentication Flow]

**Key implementation detail:** The `register()` method in AuthService sets the session cookie directly — it does NOT call a separate `login()` method. The registration endpoint IS the auto-login. [Source: architecture.md#Authentication Flow]

**Auth Module Structure (from architecture.md):**
```
backend/src/auth/
├── auth.module.ts
├── auth.controller.ts
├── auth.service.ts
├── dto/
│   ├── login.dto.ts
│   └── register.dto.ts
└── guards/
    └── session.guard.ts
```
[Source: architecture.md#Complete Project Directory Structure]

**Security Requirements:**
- Passwords hashed with bcrypt (NFR6)
- Session tokens: cookie-based httpOnly with SameSite CSRF protection (NFR7, NFR8)
- Rate limiting on auth endpoints via `@nestjs/throttler` (NFR10)
- Never return raw database errors to frontend — use NestJS exceptions
[Source: architecture.md#Authentication & Security]

**Cookie Configuration:**
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

- Endpoints: RESTful `kebab-case` (e.g., `/api/auth/register`)
- Controllers: `{Resource}Controller` (e.g., `AuthController`)
- Services: `{Resource}Service` (e.g., `AuthService`)
- DTOs: `register.dto.ts`, `login.dto.ts`
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

- Backend: Use NestJS built-in exceptions (`ConflictException` for duplicate email, `ForbiddenException` for disabled registration, `BadRequestException` for validation)
- Frontend: React Query `onError` handlers display toast notifications
- Form validation errors shown inline below fields
[Source: architecture.md#Implementation Patterns - Process Patterns - Error Handling]

### Frontend Feature Organization

```
frontend/src/features/auth/
├── register-form.tsx      # Registration form component
├── register-form.test.tsx # Component tests
├── auth-provider.tsx      # Auth context provider
├── use-auth.ts            # Auth hook (useAuth)
├── auth.api.ts            # API calls (register, login, logout, me)
```
Features are self-contained — each owns its components, hooks, and API calls.
[Source: architecture.md#Project Structure & Boundaries - Component Boundaries]

### shadcn/ui Component Mapping

| Form Element | shadcn/ui Component | Notes |
|---|---|---|
| Email input | `Input` with `type="email"` | shadcn/ui Input primitive |
| Password input | `Input` with `type="password"` | shadcn/ui Input primitive |
| Confirm password | `Input` with `type="password"` | shadcn/ui Input primitive |
| Submit button | `Button` with `disabled` + loading state | shadcn/ui Button, use `Loader2` icon for spinner |
| Validation error text | `<p className="text-sm text-destructive">` | Tailwind classes, not a separate component |
| Success/error toast | `useToast()` hook | shadcn/ui toast hook, NOT a custom component |

Do NOT build custom Input, Button, or Toast components — use shadcn/ui from `components/ui/`.
[Source: ux-design-specification.md#Component Strategy - Design System Components]

### Vite Proxy Configuration

Add to `frontend/vite.config.ts`:
```typescript
server: {
  proxy: {
    '/api': {
      target: 'http://localhost:3000',
      changeOrigin: true,
    },
  },
},
```
This routes all `/api/*` requests to the NestJS backend during development.

### Registration Toggle Persistence Note

The `registrationEnabled` flag is stored in-memory in AuthService. This is acceptable for MVP because:
- KanbanFlow is single-user focused (self-hosted)
- The flag defaults to `true` (registration open by default)
- Admin can disable via Story 6.3 (which will persist to a settings table)
- Server restart resets to default — users can re-disable after restart

This avoids creating a premature settings table before Story 6.3 defines the admin requirements.

### Project Structure After This Story

```
backend/src/auth/
├── auth.module.ts
├── auth.controller.ts
├── auth.controller.spec.ts
├── auth.service.ts
├── auth.service.spec.ts
├── dto/
│   └── register.dto.ts
backend/src/users/
├── users.module.ts
├── users.service.ts
├── users.service.spec.ts
├── users.controller.ts
├── entities/
│   └── user.entity.ts (exists, no changes)
frontend/src/features/auth/
├── register-form.tsx
├── register-form.test.tsx
├── auth-provider.tsx
├── use-auth.ts
├── auth.api.ts
```

### Critical Anti-Patterns

- ❌ NEVER use `synchronize: true` in TypeORM
- ❌ NEVER return raw database errors to frontend — use NestJS exceptions
- ❌ NEVER use `any` type — strict TypeScript
- ❌ NEVER store passwords in plain text — always bcrypt hash
- ❌ NEVER build custom UI components when shadcn/ui equivalent exists
- ❌ NEVER fetch data without React Query (no raw `fetch` in components)
- ❌ NEVER modify shadcn/ui components directly in `components/ui/`

### References

- [Source: architecture.md#Authentication Flow] — Auth endpoint patterns and auto-login behavior
- [Source: architecture.md#Authentication & Security] — Security decisions and cookie config
- [Source: architecture.md#Complete Project Directory Structure] — File locations
- [Source: architecture.md#Implementation Patterns - Naming Patterns] — Naming conventions
- [Source: architecture.md#Implementation Patterns - Error Handling] — Error patterns
- [Source: architecture.md#Implementation Patterns - Format Patterns] — API response format
- [Source: project-context.md#Technology Stack & Versions] — Stack details
- [Source: project-context.md#Framework-Specific Rules] — NestJS and React rules
- [Source: project-context.md#Critical Implementation Rules] — Language rules
- [Source: ux-design-specification.md#Component Strategy] — UI components to use
- [Source: epics.md#Story 1.3] — Original story acceptance criteria
- [Source: prd.md#User Authentication] — FR1, FR2, FR3, FR4

## Dev Agent Record

### Agent Model Used

Amelia (Developer Agent) - Senior Software Engineer

### Debug Log References

- All tests passing: backend (19 tests), frontend (3 tests)
- Installed packages: bcrypt, @types/bcrypt, cookie-parser, express-session, @types/cookie-parser, @types/express-session, @nestjs/throttler, @nestjs/swagger, @tanstack/react-query, react-router-dom
- Added vitest config and test-setup.ts for frontend testing

### Completion Notes List

- Implemented full auth registration flow: backend (NestJS) + frontend (React)
- Created Users module with findByEmail, findById, create methods
- Created Auth module with register, me endpoints
- Session management via express-session middleware in AppModule
- Registration toggle using in-memory flag (MVP approach)
- Rate limiting via @nestjs/throttler (5 req/min on register endpoint)
- Swagger decorators on all auth endpoints
- Frontend: RegisterForm with inline validation, auth-provider context, useAuth hook
- Tests: 19 backend tests (Jest), 3 frontend tests (Vitest)

### File List

**Backend:**
- `backend/src/users/users.module.ts` (new)
- `backend/src/users/users.service.ts` (new)
- `backend/src/users/users.controller.ts` (new)
- `backend/src/users/dto/create-user.dto.ts` (new)
- `backend/src/users/users.service.spec.ts` (new)
- `backend/src/auth/auth.module.ts` (new)
- `backend/src/auth/auth.service.ts` (new)
- `backend/src/auth/auth.controller.ts` (new)
- `backend/src/auth/dto/register.dto.ts` (new)
- `backend/src/auth/auth.service.spec.ts` (new)
- `backend/src/auth/auth.controller.spec.ts` (new)
- `backend/src/app.module.ts` (modified - added AuthModule, UsersModule, ThrottlerModule, session middleware)

**Frontend:**
- `frontend/src/features/auth/register-form.tsx` (new)
- `frontend/src/features/auth/register-form.test.tsx` (new)
- `frontend/src/features/auth/auth-provider.tsx` (new)
- `frontend/src/features/auth/use-auth.ts` (new)
- `frontend/src/features/auth/auth.api.ts` (new)
- `frontend/src/components/ui/input.tsx` (new)
- `frontend/src/components/ui/use-toast.tsx` (new)
- `frontend/src/App.tsx` (modified - added routing, AuthProvider, QueryClientProvider)
- `frontend/vitest.config.ts` (new)
- `frontend/src/test-setup.ts` (new)
