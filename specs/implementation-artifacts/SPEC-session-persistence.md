# Quick Tech Spec: Session DB Persistence

**Date:** 2026-05-03
**Feature:** Express-session DB store using connect-typeorm (MySQL)
**Story:** Sessions survive server restarts

---

## Problem Statement

- **Current:** express-session uses MemoryStore — sessions lost on server restart
- **Impact:** User logged out when backend restarts
- **Requirement:** Sessions persist to MySQL database

---

## Implementation Plan

1. Install `connect-typeorm` + `@types/express-session`
2. Define Session entity (implements `ISession` from connect-typeorm)
3. Register entity in `TypeOrmModule.forRootAsync()` factory
4. Create migration for `sessions` table WITH INDEX on `expiredAt`
5. Configure TypeormStore in `app.module.ts` (Express middleware)
6. Verify session persistence and test lifecycle

---

## Task Breakdown (TDD)

### Task 1: Install Dependencies

- [x] Install `connect-typeorm`
- [x] Install `@types/express-session` (dev) — already present
- [x] Verify install via `npm ls`

### Task 2: Create Session Entity

- [x] Create `backend/src/sessions/entities/session.entity.ts`
  - MUST implement `ISession` from connect-typeorm
  - `id`: string (primary key, varchar 255)
  - `expiredAt`: bigint with `@Index()` decorator (REQUIRED for cleanup)
  - `json`: text (session data)
  - `destroyedAt`: datetime (DeleteDateColumn)
- [x] Verify entity compiles with `ISession` interface
- [x] Write unit tests for entity properties

### Task 3: Register Entity in TypeORM

- [x] ~~Update `app.module.ts` TypeOrmModule.forRootAsync factory~~ — auto-discovered via `__dirname + '/**/*.entity{.ts,.js}'` glob
- [x] Verify DataSource includes Session in metadata

### Task 4: Create Migration

- [x] Create migration file `migrations/1778100000000-AddSessionsTable.ts`
- [x] Migration MUST include INDEX on `expiredAt` for cleanup performance
- [x] Migration MUST be reversible (up/down)
- [x] Execute migration manually, verify table created with DESCRIBE

### Task 5: Configure TypeormStore in AppModule

- [x] Update `backend/src/app.module.ts`
  - Import `TypeormStore` from connect-typeorm
  - Inject `DataSource` via constructor injection (instead of `app.get(DataSource)`)
  - Get session repository: `this.dataSource.getRepository(Session)`
  - Replace session middleware store with TypeormStore
- [x] Config: `ttl: 86400` (24h), `cleanupLimit: 10`, `limitSubquery: false` (MySQL compatibility), `onError: console.error`
- [x] Verify DataSource is available before configuring store — confirmed via debug logging
- [x] Integration test: session persists after server restart simulation

### Task 6: Test Session Lifecycle

- [x] Test: Login creates session in DB
- [x] Test: `/api/auth/me` returns user (session restored)
- [x] Test: Logout ends session (/me returns 401) — NOTE: connect-typeorm `destroy()` has a bug where `softDelete` promise is not awaited; functional behavior (cookie cleared → 401) works correctly
- [x] Test: Session survives app restart (new app instance with same DB)

---

## Edge Cases

| Scenario | Handling |
|----------|----------|
| MySQL connection fails | TypeormStore `onError` handler logs + falls back to treating request as unauthenticated |
| Expired session | TypeormStore auto-cleanup via indexed `expiredAt` column + `cleanupLimit` |
| Corrupted session data | Catch JSON parse error in store, call destroy() |
| Concurrent login (same user) | Each login = new session row (TypeormStore does NOT auto-replace) |
| Cookie not sent | Return 401, same as before |

---

## Technical Implementation Patterns

### Session Entity Pattern
```typescript
import { ISession } from "connect-typeorm";
import { Column, DeleteDateColumn, Entity, Index, PrimaryColumn } from "typeorm";

@Entity()
export class Session implements ISession {
    @Index()
    @Column("bigint")
    public expiredAt = Date.now();

    @PrimaryColumn("varchar", { length: 255 })
    public id = "";

    @Column("text")
    public json = "";

    @DeleteDateColumn()
    public destroyedAt?: Date;
}
```

### TypeormStore in AppModule Pattern
```typescript
// In app.module.ts - nestApplication.use() after NestFactory.create
// Current session middleware at lines 62-72 needs update:

// After await app.get(DataSource) or after database connection
const dataSource = app.get(DataSource);
const sessionRepo = dataSource.getRepository(Session);

// TypeormStore config replaces MemoryStore
app.use(session({
    store: new TypeormStore({
        ttl: 86400,
        cleanupLimit: 10,
        onError: (store, error) => console.error('Session store error:', error),
    }).connect(sessionRepo),
    secret: this.getSessionSecret(),
    resave: false,
    saveUninitialized: false,
    cookie: { /* existing cookie config */ }
}));
```

---

## Current Auth API Reference

| Endpoint | Method | Session Data |
|----------|--------|--------------|
| `/api/auth/login` | POST | Creates session with `userId`, `email`, `role` |
| `/api/auth/logout` | POST | Calls `session.destroy()` |
| `/api/auth/me` | GET | Returns current user from session |
| `/api/auth/register` | POST | Auto-login after registration |

**Cookie name:** `connect.sid`
**Session maxAge:** 86400000ms (1 day)

---

## Test Coverage Requirements

| Test Type | Coverage |
|-----------|----------|
| Unit | Session entity creation, serialization, expirationAt calculation |
| Integration | Session CRUD via API flows |
| E2E | Login → Session in DB → Restart → `/me` returns 200 |

All existing tests must pass 100% before story is ready for review.

---

## Files to Create/Modify

| File Status | Path |
|------------|------|
| CREATE | `backend/src/sessions/entities/session.entity.ts` |
| CREATE | `migrations/XXXXXXXXXXXXXX-add-sessions-table.sql` |
| MODIFY | `backend/src/app.module.ts` (entity + TypeormStore) |

---

## Rollback Plan

1. Revert `app.module.ts` to remove TypeormStore (use default MemoryStore)
2. Run down migration to drop sessions table
3. Uninstall `connect-typeorm`