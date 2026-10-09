# Story 1.2: Database Schema Setup

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a developer,
I want the database schema initialized with TypeORM,
so that the application can persist user and project data.

## Acceptance Criteria

1. **Given** a MySQL database is available, **When** I configure the `.env` file with database credentials, **Then** the NestJS ConfigModule loads configuration with validation
2. **And** TypeORM connects to MySQL with `synchronize: false`
3. **And** a `data-source.ts` file exists for TypeORM CLI
4. **And** initial migration creates `users` and `projects` tables
5. **And** `users` table has: id, email, password, role, created_at, updated_at
6. **And** `projects` table has: id, name, user_id (FK), created_at, updated_at
7. **And** `npm run migration:run` executes successfully

## Tasks / Subtasks

- [x] Task 1: Install backend dependencies (AC: 1, 2)
  - [x] Install `@nestjs/config` for ConfigModule
  - [x] Install `@nestjs/typeorm` and `typeorm` for ORM
  - [x] Install `mysql2` driver for MySQL connectivity
  - [x] Install `class-validator` and `class-transformer` for .env validation
  - [x] Install `dotenv` for data-source.ts standalone loading
- [x] Task 2: Modify `.env` file with database configuration (AC: 1)
  - [x] Add DB vars to existing `backend/.env`: DB_HOST=localhost, DB_PORT=3306, DB_USERNAME=root, DB_PASSWORD=password, DB_NAME=kanbanflow_dev
  - [x] Add NODE_ENV=development
  - [x] Create `backend/.env.example` with placeholder values for documentation (never commit real .env)
- [x] Task 3: Create configuration module (AC: 1)
  - [x] Create `backend/src/config/configuration.ts` using `registerAs('database', ...)`
  - [x] Validate config with class-validator decorators (IsString, IsNumber, IsOptional)
  - [x] Export typed config interface
- [x] Task 4: Create TypeORM data-source for CLI (AC: 3)
  - [x] Create `backend/src/data-source.ts` as standalone DataSource
  - [x] Use `dotenv/config` import to load .env (NOT ConfigService — runs outside NestJS)
  - [x] Read DB values via `process.env.DB_HOST` etc.
  - [x] Set `synchronize: false`, `migrationsRun: false`
  - [x] Point entities to `dist/**/*.entity.js` and migrations to `dist/migrations/*.js`
- [x] Task 5: Configure TypeORM in app.module.ts (AC: 2)
  - [x] Add `ConfigModule.forRoot({ isGlobal: true, validate })` where `validate` uses class-validator (see Dev Notes)
  - [x] Add `TypeOrmModule.forRootAsync()` with inject ConfigService
  - [x] Configure: type mysql, entities auto-load, migrations path, synchronize false
- [x] Task 6: Create User entity (AC: 5)
  - [x] Create `backend/src/users/entities/user.entity.ts`
  - [x] Columns: id (PK auto-increment), email (unique, max 255), password (min 8 chars stored as bcrypt hash), role (default 'user'), created_at, updated_at
  - [x] Add OneToMany relation to Project
- [x] Task 7: Create Project entity (AC: 6)
  - [x] Create `backend/src/projects/entities/project.entity.ts`
  - [x] Columns: id (PK auto-increment), name, user_id (FK integer), created_at, updated_at
  - [x] Add ManyToOne relation to User with onDelete CASCADE
- [x] Task 8: Generate and run initial migration (AC: 4, 7)
  - [x] Run `npm run build` to compile entities
  - [x] Run `npm run migration:generate -- src/migrations/CreateUsersProjects`
  - [x] Review generated migration SQL — creates `users`, `projects` tables + FK constraint
  - [x] Run `npm run migration:run` — executed successfully
  - [x] Verify with `npm run migration:show` — shows `[X] 1 CreateUsersProjects1773977509530`
- [x] Task 9: Add migration scripts to package.json (AC: 7)
  - [x] Add `typeorm`, `migration:generate`, `migration:create`, `migration:run`, `migration:revert`, `migration:show`, `db:sync` scripts
  - [x] Run `npm run migration:show` to verify scripts work — shows `[X] 1 CreateUsersProjects1773977509530`
- [x] Task 10: Create unit test stubs
  - [x] Create `backend/src/users/entities/user.entity.spec.ts` with placeholder test for User entity
  - [x] Create `backend/src/projects/entities/project.entity.spec.ts` with placeholder test for Project entity

## Dev Notes

### Previous Story Intelligence

**From Story 1.1 (Project Initialization):**
- Monorepo structure created: `frontend/` and `backend/` directories at project root
- Backend initialized with NestJS CLI — `backend/src/main.ts`, `backend/src/app.module.ts` exist
- Frontend uses Vite + React + TypeScript with shadcn/ui initialized
- Root `.prettierrc` configured (semi: true, singleQuote: true, trailingComma: "all", printWidth: 100, tabWidth: 2)
- Root `.gitignore` covers node_modules, dist, .env patterns
- TypeScript strict mode enabled in both frontend and backend
- Backend has `nest-cli.json`, `tsconfig.json`, `tsconfig.build.json`
- Backend `.prettierrc` removed (using root config)

**Patterns established:**
- Backend files: `kebab-case` (e.g., `board.entity.ts`)
- NestJS modules use `@Module()` decorators with DI via constructor
- Separate tsconfig for frontend (Vite) and backend (NestJS)
- Tests co-located with implementation files (`.spec.ts` for backend)

### Architecture Compliance

**TypeORM + MySQL Configuration:**
- `synchronize: false` ALWAYS — no exceptions. Every schema change goes through migrations [Source: architecture.md#Database Migration Strategy]
- Entities auto-discovered from `dist/**/*.entity.js` (compiled) or `src/**/*.entity.ts` (dev)
- Migrations stored in `src/migrations/`, compiled to `dist/migrations/`
- `migrationsRun: false` — always explicit manual migration runs

**data-source.ts Structure:**
```typescript
import 'dotenv/config';
import { DataSource } from 'typeorm';

export default new DataSource({
  type: 'mysql',
  host: process.env.DB_HOST,
  port: Number(process.env.DB_PORT),
  username: process.env.DB_USERNAME,
  password: process.env.DB_PASSWORD,
  database: process.env.DB_NAME,
  entities: ['dist/**/*.entity.js'],
  migrations: ['dist/migrations/*.js'],
  migrationsTableName: 'typeorm_migrations',
  migrationsRun: false,
  synchronize: false,
});
```
Note: Uses `dotenv/config` and `process.env` directly — NOT ConfigService. ConfigService requires NestJS DI which is unavailable in CLI context. [Source: architecture.md#Database Migration Strategy - Configuration]

**app.module.ts TypeORM Config:**
```typescript
TypeOrmModule.forRootAsync({
  inject: [ConfigService],
  useFactory: (config: ConfigService) => ({
    type: 'mysql',
    host: config.get('DB_HOST'),
    port: config.get<number>('DB_PORT'),
    username: config.get('DB_USERNAME'),
    password: config.get('DB_PASSWORD'),
    database: config.get('DB_NAME'),
    entities: [__dirname + '/**/*.entity{.ts,.js}'],
    migrations: [__dirname + '/migrations/*{.ts,.js}'],
    migrationsTableName: 'typeorm_migrations',
    migrationsRun: false,
    synchronize: false,
  }),
}),
```
[Source: architecture.md#Database Migration Strategy - Configuration]

### Database Naming Conventions

- Tables: `snake_case` plural (e.g., `users`, `projects`)
- Columns: `snake_case` (e.g., `created_at`, `user_id`)
- Foreign keys: `{referenced_table}_id` (e.g., `user_id`)
- Primary keys: `id` (auto-increment integer)
- Indexes: `idx_{table}_{columns}` (e.g., `idx_projects_user_id`)
[Source: architecture.md#Implementation Patterns - Naming Patterns]

### Entity Specifications

**User Entity (`backend/src/users/entities/user.entity.ts`):**
```typescript
import { Entity, PrimaryGeneratedColumn, Column, CreateDateColumn, UpdateDateColumn, OneToMany } from 'typeorm';
import { Project } from '../../projects/entities/project.entity';

@Entity('users')
export class User {
  @PrimaryGeneratedColumn()
  id: number;

  @Column({ unique: true, length: 255 })
  email: string;

  @Column({ length: 255 })
  password: string;

  @Column({ default: 'user' })
  role: string;

  @CreateDateColumn()
  created_at: Date;

  @UpdateDateColumn()
  updated_at: Date;

  @OneToMany(() => Project, (project) => project.user)
  projects: Project[];
}
```
Field constraints: email unique + max 255 chars, password max 255 (stores bcrypt hash), role defaults to 'user'. Email format validation happens in DTOs/auth layer, not in entity.

**Project Entity (`backend/src/projects/entities/project.entity.ts`):**
```typescript
import { Entity, PrimaryGeneratedColumn, Column, CreateDateColumn, UpdateDateColumn, ManyToOne, JoinColumn } from 'typeorm';
import { User } from '../../users/entities/user.entity';

@Entity('projects')
export class Project {
  @PrimaryGeneratedColumn()
  id: number;

  @Column()
  name: string;

  @Column()
  user_id: number;

  @ManyToOne(() => User, (user) => user.projects, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'user_id' })
  user: User;

  @CreateDateColumn()
  created_at: Date;

  @UpdateDateColumn()
  updated_at: Date;
}
```

### .env Configuration

Add these DB variables to existing `backend/.env` (created in story 1.1):
```env
# Database
DB_HOST=localhost
DB_PORT=3306
DB_USERNAME=root
DB_PASSWORD=password
DB_NAME=kanbanflow_dev

# App
NODE_ENV=development
```
Note: Modify the existing .env file — do NOT overwrite it. Story 1.1 may have added other variables. [Source: architecture.md#Database Migration Strategy - Configuration]

### Migration Scripts (package.json)

Add to `backend/package.json` scripts:
```json
{
  "typeorm": "typeorm-ts-node-commonjs",
  "migration:generate": "npm run typeorm migration:generate -- -d src/data-source.ts",
  "migration:create": "npm run typeorm migration:create",
  "migration:run": "npm run typeorm migration:run -- -d src/data-source.ts",
  "migration:revert": "npm run typeorm migration:revert -- -d src/data-source.ts",
  "migration:show": "npm run typeorm migration:show -- -d src/data-source.ts",
  "db:sync": "npm run migration:run"
}
```
[Source: architecture.md#Database Migration Strategy - Package Scripts]

### Migration Naming Convention

```
{timestamp}-{Action}{Table}.ts
```
Actions: `Create`, `Add`, `Remove`, `Rename`, `Alter`, `Seed`

Examples:
- `1710825600000-CreateUsers.ts`
- `1710825600001-CreateProjects.ts`
- `1710825600002-AddIndexOnProjectsUserId.ts`
[Source: architecture.md#Database Migration Strategy - Migration Naming Convention]

### ConfigModule Setup

Create `backend/src/config/configuration.ts`:
```typescript
import { registerAs } from '@nestjs/config';

export default registerAs('database', () => ({
  host: process.env.DB_HOST,
  port: parseInt(process.env.DB_PORT ?? '3306', 10),
  username: process.env.DB_USERNAME,
  password: process.env.DB_PASSWORD,
  name: process.env.DB_NAME,
}));
```

Create `backend/src/config/env.validation.ts`:
```typescript
import { plainToInstance } from 'class-transformer';
import { IsString, IsNumber, IsOptional, validateSync } from 'class-validator';

class EnvironmentVariables {
  @IsString()
  DB_HOST: string;

  @IsNumber()
  DB_PORT: number;

  @IsString()
  DB_USERNAME: string;

  @IsString()
  DB_PASSWORD: string;

  @IsString()
  DB_NAME: string;

  @IsOptional()
  @IsString()
  NODE_ENV: string;
}

export function validate(config: Record<string, unknown>) {
  const validatedConfig = plainToInstance(EnvironmentVariables, config, {
    enableImplicitConversion: true,
  });
  const errors = validateSync(validatedConfig, { skipMissingProperties: false });
  if (errors.length > 0) throw new Error(errors.toString());
  return validatedConfig;
}
```

In `app.module.ts`:
```typescript
import { ConfigModule } from '@nestjs/config';
import { validate } from './config/env.validation';
import configuration from './config/configuration';

@Module({
  imports: [
    ConfigModule.forRoot({
      isGlobal: true,
      load: [configuration],
      validate,
    }),
    TypeOrmModule.forRootAsync({
      inject: [ConfigService],
      useFactory: (config: ConfigService) => ({
        type: 'mysql',
        host: config.get('DB_HOST'),
        port: config.get<number>('DB_PORT'),
        username: config.get('DB_USERNAME'),
        password: config.get('DB_PASSWORD'),
        database: config.get('DB_NAME'),
        entities: [__dirname + '/**/*.entity{.ts,.js}'],
        migrations: [__dirname + '/migrations/*{.ts,.js}'],
        migrationsTableName: 'typeorm_migrations',
        migrationsRun: false,
        synchronize: false,
      }),
    }),
  ],
})
export class AppModule {}
```

### Critical Anti-Patterns

- ❌ NEVER set `synchronize: true` — no exceptions [Source: architecture.md#Non-Negotiable Migration Rules]
- ❌ NEVER return raw database errors to frontend — use NestJS exceptions [Source: architecture.md#Implementation Patterns - Error Handling]
- ❌ NEVER use `any` type — strict TypeScript [Source: architecture.md#Anti-Patterns to Avoid]
- ❌ NEVER auto-run migrations on app start — explicit runs only [Source: architecture.md#Non-Negotiable Migration Rules]
- ❌ NEVER modify applied migrations — create new one to fix [Source: architecture.md#Non-Negotiable Migration Rules]

### Migration Workflow (for reference)

1. Create/modify entity file
2. Run `npm run migration:generate -- src/migrations/{timestamp}-{Action}{Table}`
3. Review generated migration SQL
4. Run `npm run migration:run`
5. Verify with `npm run migration:show`
6. Commit entity + migration together
[Source: architecture.md#Database Migration Strategy - Schema Change Workflow]

### Project Structure After This Story

```
backend/
├── .env                              (modified - added DB vars)
├── .env.example                      (created)
├── package.json                      (modified - migration scripts)
└── src/
    ├── app.module.ts                 (modified - ConfigModule + TypeORM)
    ├── config/
    │   ├── configuration.ts          (created)
    │   └── env.validation.ts         (created)
    ├── data-source.ts                (created)
    ├── migrations/
    │   └── {timestamp}-CreateUsersProjects.ts  (generated)
    ├── users/
    │   └── entities/
    │       ├── user.entity.ts        (created)
    │       └── user.entity.spec.ts   (created - test stub)
    └── projects/
        └── entities/
            ├── project.entity.ts     (created)
            └── project.entity.spec.ts (created - test stub)
```

### References

- [Source: architecture.md#Database Migration Strategy] — Complete migration workflow, configuration, naming conventions, package scripts
- [Source: architecture.md#Implementation Patterns - Naming Patterns] — DB table/column naming conventions
- [Source: architecture.md#Project Structure & Boundaries] — Backend directory structure
- [Source: architecture.md#Core Architectural Decisions - Data Architecture] — TypeORM + MySQL decisions
- [Source: architecture.md#Infrastructure & Deployment - Config management] — NestJS ConfigModule with validation
- [Source: project-context.md#Technology Stack & Versions] — Backend tech stack
- [Source: project-context.md#Critical Implementation Rules] — Language and framework rules
- [Source: project-context.md#Testing Rules] — Co-located .spec.ts files, Jest for backend
- [Source: project-context.md#Development Workflow Rules] — Database workflow rules
- [Source: epics.md#Story 1.2] — Original story acceptance criteria
- [Source: implementation-artifacts/1-1-project-initialization.md] — Previous story context and file list

## Dev Agent Record

### Agent Model Used

Amelia (Dev Agent) — opencode/mimo-v2-pro-free

### Debug Log References

- Build failed initially due to TypeScript strict mode (`strictPropertyInitialization`). Fixed by adding `!` definite assignment assertions to all entity properties and `EnvironmentVariables` class properties in `env.validation.ts`.
- `data-source.ts` originally used `dist/**/*.entity.js` / `dist/migrations/*.js` paths. Changed to `__dirname + '/**/*.entity{.ts,.js}'` / `__dirname + '/migrations/*{.ts,.js}'` so CLI works via `ts-node`.
- First migration generation used filename `1710825600000-CreateUsersProjects` which produced invalid TS class name starting with a number. Regenerated as `CreateUsersProjects` — valid class name.

### Completion Notes List

- All 10 tasks implemented and verified
- `npm run build` passes (0 errors)
- `npm test` passes (3/3 tests)
- All 7 ACs satisfied — `npm run migration:run` executes successfully, tables verified in MySQL
- Migration generated via TypeORM CLI, not manual
- Used `!` (non-null assertion) on entity fields for strict TS compatibility — standard TypeORM pattern

### Change Log

- 2026-03-20: Initial implementation — all tasks complete (Amelia)

### File List

- `backend/.env` (created)
- `backend/.env.example` (created)
- `backend/package.json` (modified — added migration scripts + dependencies)
- `backend/src/app.module.ts` (modified — added ConfigModule + TypeOrmModule)
- `backend/src/config/configuration.ts` (created)
- `backend/src/config/env.validation.ts` (created)
- `backend/src/data-source.ts` (created)
- `backend/src/migrations/1773977509530-CreateUsersProjects.ts` (generated via CLI)
- `backend/src/users/entities/user.entity.ts` (created)
- `backend/src/users/entities/user.entity.spec.ts` (created)
- `backend/src/projects/entities/project.entity.ts` (created)
- `backend/src/projects/entities/project.entity.spec.ts` (created)
