# Story 4.5: Checklists with Progress

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a user,
I want to add checklists to cards with visual progress tracking,
So that I can break down tasks into subtasks and see completion status.

## Acceptance Criteria

1. **Given** I am viewing the card detail panel
   **When** I click "Add Checklist"
   **Then** a new checklist section appears
   **And** the checklist has an editable title prefilled with "Checklist"
   **And** the checklist shows an input for the first item

2. **Given** I type a checklist item and press Enter
   **When** the item is added
   **Then** it appears as a checkbox with label text
   **And** the input stays focused for adding more items

3. **Given** I edit a checklist title
   **When** I blur the title input or press Enter
   **Then** the title is saved automatically
   **And** empty titles are rejected and revert to the previous value
   **And** the updated title persists when I reopen the card

4. **Given** I edit a checklist item label
   **When** I blur the item text field or press Enter
   **Then** the item text is saved automatically
   **And** empty item text is rejected and reverts to the previous value
   **And** the updated text persists when I reopen the card

5. **Given** I check or uncheck a checklist item
   **When** the state changes
   **Then** the checklist progress updates immediately
   **And** the progress bar appears on the card face on the board
   **And** the card face progress reflects the aggregate completion across all checklist items on the card

6. **Given** a checklist is complete (100%)
   **When** viewing the card
   **Then** the progress bar shows full green fill with a checkmark

7. **Given** a checklist is partially complete
   **When** viewing the card
   **Then** the progress bar shows teal fill proportional to completion

8. **Given** I want to delete a checklist item
   **When** I click the delete icon on an item
   **Then** the item is removed and progress recalculates
   **And** an undo toast notification appears for 5 seconds

9. **Given** a checklist has no items or is newly created
   **When** viewing the card
   **Then** the progress bar shows empty gray bar at 0%
   **And** no division-by-zero occurs (0/0 renders as 0%)

10. **Given** I want to delete the entire checklist
   **When** I click the delete checklist button
   **Then** the checklist and all its items are removed

## Tasks / Subtasks

- [ ] Backend: Create `checklists` module structure
  - [ ] Create `backend/src/checklists/checklists.module.ts`
  - [ ] Create `backend/src/checklists/entities/checklist.entity.ts`
  - [ ] Create `backend/src/checklists/entities/checklist-item.entity.ts`
  - [ ] Create `backend/src/checklists/dto/create-checklist.dto.ts`
  - [ ] Create `backend/src/checklists/dto/update-checklist.dto.ts`
  - [ ] Create `backend/src/checklists/dto/create-checklist-item.dto.ts`
  - [ ] Create `backend/src/checklists/dto/update-checklist-item.dto.ts`
  - [ ] Create `backend/src/checklists/checklists.service.ts`
  - [ ] Create `backend/src/checklists/checklists.controller.ts`
- [ ] Backend: Create database migration
  - [ ] Create migration for `checklists` and `checklist_items` tables
- [ ] Backend: Update Card entity
  - [ ] Add `@OneToMany` relation to Checklist entity
  - [ ] Update card detail response to include checklist titles and items
  - [ ] Update board-level card responses to include checklist progress summary for CardPreview
- [ ] Backend: Run migrations
  - [ ] Apply the hand-written migration with `typeorm migration:run`
  - [ ] Verify rollback works: `typeorm migration:revert`
- [ ] Backend: Register module in app.module.ts
  - [ ] Import `ChecklistsModule` after `CardsModule`
- [ ] Backend: Implement checklist-level delete endpoint (`DELETE /api/checklists/:id`) with CASCADE
  - [ ] Implement checklist title update endpoint (`PATCH /api/checklists/:id`)
  - [ ] Implement checklist item create/update/delete endpoints
- [ ] Frontend: Create `checklists` feature folder
  - [ ] Create `frontend/src/features/checklists/checklist-section.tsx`
  - [ ] Create `frontend/src/features/checklists/checklist.tsx`
  - [ ] Create `frontend/src/features/checklists/checklist-item.tsx`
  - [ ] Create `frontend/src/features/checklists/progress-bar.tsx`
  - [ ] Create `frontend/src/features/checklists/checklists.api.ts`
  - [ ] Create `frontend/src/features/checklists/use-checklists.ts`
- [ ] Frontend: Integrate into CardDetailPanel
  - [ ] Add "Add Checklist" button inside the Checklist section wrapper
  - [ ] Render ChecklistSection when a card has one or more checklists
  - [ ] Support inline editing for checklist titles and item text
  - [ ] Load full checklist data for the detail panel
  - [ ] Add a dedicated card detail query hook for the panel data shape
  - [ ] Update card data types to include checklist detail payloads and board progress summary
- [ ] Frontend: Update CardPreview
  - [ ] Add progress badge showing aggregated completion percentage
  - [ ] Show green checkmark when 100% complete
- [ ] Tests: Backend unit tests for checklists service
  - [ ] Test create checklist, rename checklist, add item, edit item, toggle item, delete item, delete checklist
  - [ ] Test CASCADE delete: deleting card removes its checklists
  - [ ] Test CASCADE delete: deleting checklist removes its items
  - [ ] Verify persistence after each POST/PATCH/DELETE with a follow-up API call or DB query
- [ ] Tests: Frontend unit tests for checklist components
  - [ ] ProgressBar renders correctly at 0%, partial, 100%
  - [ ] ProgressBar handles `total=0` without division error
  - [ ] ChecklistSection renders "Add Checklist" button when empty and when populated
  - [ ] Checklist title input auto-saves on blur/Enter
  - [ ] Adding item focuses input for next entry
  - [ ] Checklist item text auto-saves on blur/Enter
  - [ ] Deleting item recalculates progress and shows undo toast
  - [ ] CardPreview badge shows aggregated progress from all checklists
- [ ] Tests: E2E test for checklist CRUD operations
  - [ ] Full flow: add card → add checklist → rename checklist → add items → edit item text → toggle → verify progress on card face
  - [ ] Delete checklist item → verify progress updates on card face
  - [ ] Delete checklist → verify all items removed
  - [ ] Verify persistence after each mutation with a second API call or DB query

## Dev Notes

### Architecture Compliance

- **New NestJS Module:** Create `checklists/` module following existing patterns (boards, cards, columns)
- **Module template:** `@Module({ imports: [TypeOrmModule.forFeature([Checklist, ChecklistItem])], controllers: [ChecklistsController], providers: [ChecklistsService], exports: [ChecklistsService] })`
- **Registration:** Import `ChecklistsModule` in `app.module.ts` after `CardsModule`
- **Entity Relationships:** Checklist belongs to Card (CASCADE delete), ChecklistItem belongs to Checklist (CASCADE delete)
- **Feature-based Frontend:** Create `features/checklists/` following existing patterns (boards, cards, labels)
- **React Query:** Use mutations with optimistic updates for checklist title, item text, toggle, and delete operations

### Migration Strategy (MUST follow existing hand-written pattern)

**Existing convention** (from `backend/src/migrations/`): All migrations are **hand-written raw SQL** via `queryRunner.query()`, NOT auto-generated. Pattern:

```typescript
import { MigrationInterface, QueryRunner } from 'typeorm';

export class CreateChecklists<timestamp> implements MigrationInterface {
  name = 'CreateChecklists<timestamp>';

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`CREATE TABLE \`checklists\` (...) ENGINE=InnoDB`);
    await queryRunner.query(`CREATE TABLE \`checklist_items\` (...) ENGINE=InnoDB`);
    // Add FK constraints as separate ALTER TABLE statements
    await queryRunner.query(`ALTER TABLE \`checklists\` ADD CONSTRAINT \`FK_checklists_card_id\` FOREIGN KEY (\`card_id\`) REFERENCES \`cards\`(\`id\`) ON DELETE CASCADE`);
    await queryRunner.query(`ALTER TABLE \`checklist_items\` ADD CONSTRAINT \`FK_checklist_items_checklist_id\` FOREIGN KEY (\`checklist_id\`) REFERENCES \`checklists\`(\`id\`) ON DELETE CASCADE`);
    // Add indexes
    await queryRunner.query(`CREATE INDEX \`IDX_checklists_card_id\` ON \`checklists\`(\`card_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_checklist_items_checklist_id\` ON \`checklist_items\`(\`checklist_id\`)`);
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`ALTER TABLE \`checklist_items\` DROP FOREIGN KEY \`FK_checklist_items_checklist_id\``);
    await queryRunner.query(`ALTER TABLE \`checklists\` DROP FOREIGN KEY \`FK_checklists_card_id\``);
    await queryRunner.query(`DROP INDEX \`IDX_checklist_items_checklist_id\` ON \`checklist_items\``);
    await queryRunner.query(`DROP INDEX \`IDX_checklists_card_id\` ON \`checklists\``);
    await queryRunner.query(`DROP TABLE \`checklist_items\``);
    await queryRunner.query(`DROP TABLE \`checklists\``);
  }
}
```

**Migration registration:** Import and add to both `entities` and `migrations` arrays in `backend/src/database/typeorm-registry.ts`:

```typescript
// Add imports:
import { Checklist } from '../checklists/entities/checklist.entity';
import { ChecklistItem } from '../checklists/entities/checklist-item.entity';
import { CreateChecklists<timestamp> } from '../migrations/<timestamp>-CreateChecklists';

// Add to entities array:
export const entities = [User, Project, Board, BoardColumn, Card, CardLabel, Label, Session, Checklist, ChecklistItem];

// Add to migrations array:
export const migrations = [
  // ... existing migrations ...,
  CreateChecklists<timestamp>,
];
```

**Timestamp format:** Use Unix milliseconds matching existing pattern (e.g., `1779000000000`).

**SQL conventions:**
- FK naming: `FK_<table>_<column>` (e.g., `FK_checklists_card_id`)
- Index naming: `IDX_<table>_<column>` (e.g., `IDX_checklist_items_checklist_id`)
- `datetime(6)` for timestamps (NOT `TIMESTAMP`)
- `ENGINE=InnoDB` on all tables
- FK constraints as separate `ALTER TABLE` statements (not inline)

### Backend Implementation Details

**Card entity** (`backend/src/cards/entities/card.entity.ts`):
- Add `@OneToMany(() => Checklist, (checklist) => checklist.card) checklists!: Checklist[];`
- No explicit `@JoinColumn` on OneToMany side

**DTO patterns** (follow existing conventions):
- `@IsString()`, `@IsNotEmpty()`, `@MaxLength(255)`, `@IsOptional()`, `@IsNumber()` from `class-validator`
- `@ApiProperty({ example: ... })` on every field for Swagger
- `@ValidateIf((_object, value) => value !== null)` for nullable optional fields
- File naming: `create-checklist.dto.ts`, `update-checklist.dto.ts`, `create-checklist-item.dto.ts`, `update-checklist-item.dto.ts`
- `update-checklist.dto.ts` handles editable checklist titles
- `update-checklist-item.dto.ts` handles editable item text plus completion state

**Controller patterns** (follow existing conventions):
- Route prefix: `api/checklists` / `api/checklist-items` (flat pattern, like `api/labels`)
- Endpoints:
  - `POST /api/cards/:cardId/checklists` — Create checklist (nested, like `api/columns/:columnId/cards`)
  - `PATCH /api/checklists/:id` — Rename checklist
  - `DELETE /api/checklists/:id` — Delete checklist
  - `POST /api/checklists/:checklistId/items` — Create item
  - `PATCH /api/checklist-items/:id` — Update item text and/or completion
  - `DELETE /api/checklist-items/:id` — Delete item
- Class decorator: `@UseGuards(SessionGuard)`, `@ApiTags('checklists')`
- `@Session() session: SessionData` for userId extraction
- `ParseIntPipe` on all `:id` params
- `@UsePipes(new ValidationPipe({ transform: true }))` on POST/PATCH
- Response shape: `{ data: ..., message: 'Checklist created' }` for mutations

**Service patterns:**
- Ownership verification via private `findChecklistById(id, userId)` / `findChecklistItemById(id, userId)` that check `card.column.board.user_id`
- Chain: `checklist → card → column → board → user_id`
- `NotFoundException('Checklist not found')` for missing resources
- `ForbiddenException('Access denied')` for cross-user access
- `@InjectRepository(Checklist)` / `@InjectRepository(ChecklistItem)`
- Transactions if needed via `dataSource.transaction()`

### Frontend Implementation Details

**Feature folder** (`frontend/src/features/checklists/`):
```
checklists/
  checklists.api.ts         -- API functions (fetch, create, delete checklist/item)
  use-checklists.ts         -- React Query hooks
  checklist-section.tsx     -- Container component for all checklists on a card
  checklist.tsx             -- Single checklist with title + items list
  checklist-item.tsx        -- Single item row with checkbox + delete
  progress-bar.tsx          -- Progress bar component
  checklist-section.test.tsx
  checklist.test.tsx
  checklist-item.test.tsx
  progress-bar.test.tsx
```

**API file conventions** (follow `cards.api.ts`):
- `FETCH_OPTIONS: RequestInit = { credentials: 'include' }`
- `handleResponse<T>(response)` helper
- All paths: `/api/...`
- Every function has try/catch with `'Network error — please check your connection'`
- Types exported for hook usage

**Hook conventions** (follow `use-cards.ts`):
```typescript
const checklistKeys = {
  byCard: (cardId: number) => ['checklists', cardId] as const,
  all: () => ['checklists'] as const,
};
```
- Invalidation: `queryClient.invalidateQueries({ queryKey: checklistKeys.byCard(cardId) })`
- Also invalidate `['cards']` and `['columns']` so CardPreview updates on the board
- Use `useMutation` with `mutationFn`, `onSuccess`, `onError`, and optimistic updates for snappy checklist interactions

**Component patterns:**
- Import via `@/` alias (e.g., `@/features/checklists/checklist-section`)
- shadcn/ui: No `checkbox.tsx` or `progress.tsx` exists yet — must create following existing shadcn/ui patterns (cva variants, radix primitive or native)
  - `CheckboxItem`: Use native `<input type="checkbox">` styled with Tailwind (shadcn checkbox is heavy for this use case) OR add `checkbox.tsx` to `components/ui/`
  - `ProgressBar`: Pure Tailwind div-based (width %, bg color), no radix dependency needed
- Toast for undo: Import `useToast()` from `@/components/ui/use-toast`
- AlertDialog for checklist delete confirmation: Already exists at `components/ui/alert-dialog.tsx`

**CardDetailPanel integration** (at `card-detail-panel.tsx:285`):
- Replace the placeholder `<p>Checklists will be available...</p>` with actual component
- Follow existing section pattern:
```tsx
<Separator />
<div className="space-y-2">
  <h3 className="text-sm font-medium">Checklist</h3>
  <ChecklistManager cardId={card.id} checklists={card.checklists ?? []} />
</div>
```

**CardPreview integration** (at `card-preview.tsx`):
- Card previews should not need full checklist item payloads if a progress summary is present on the board card response
- Board card responses should expose `checklist_progress` with `completed`, `total`, and `percent` fields
- Add progress badge after labels, before dueDateBadge:
```tsx
{card.checklist_progress && (
  /* aggregated progress across all checklists */
)}
```

**Progress colors (Tailwind v4 compatibility):**
- Theme has `--success: #10B981` (emerald) — use for 100% complete
- For partial: use `bg-emerald-500` (matches existing `--success` color)
- For 0% empty: `bg-zinc-200` (matches existing `--border` lightness)
- Background track: `bg-zinc-100`
- Use `inline` Tailwind classes (Tailwind v4 has all built-in colors)

### Testing Patterns

**Backend tests** (co-located):
- `checklists.service.spec.ts` — test all service methods, including authorization checks
- `checklists.controller.spec.ts` — test route handling, param validation
- Follow existing spec patterns (Vitest, NestJS testing utilities)

**Frontend tests** (co-located, follow existing patterns):
- Vitest + @testing-library/react + userEvent
- QueryClient wrapper: `new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })`
- `vi.mock()` for dependencies (use-checklists, use-toast)
- Test progress bar states: 0%, partial, 100%, and `total=0` edge case
- Test undo toast appears on item delete

**E2E tests** (`frontend/e2e/checklists.spec.ts`):
- Follow `due-dates.spec.ts` pattern:
  - `test.beforeAll` — register test user
  - Login via API, setup test data (board + column + card)
  - UI login → navigate → interact → assert → reload → verify persistence
  - Use `monitoringTest` from `test-utils.ts`

### "Add Checklist" button placement

- **Not in CardDetailPanel header** — follow the section pattern used by Labels and DueDate sections
- Place "Add Checklist" button inside the checklist section wrapper, as the trigger to create a new checklist
- When no checklists exist, show the button as the primary action in the section
- When checklists exist, show the button above/below the checklist list

### Future-proofing

- `position` column on `checklist_items` for eventual reorder — no reorder UI in this story
- Delete confirmation: Toast with undo for item deletion (shadcn `useToast`), AlertDialog for checklist-level delete (shadcn `AlertDialog`)

### React Query key convention

- Use `['checklists', cardId]` — matching `['cards', columnId]` pattern
- After mutations, invalidate:
  - `checklistKeys.byCard(cardId)` — refreshes checklist data
  - `['cards']` — refreshes CardPreview badges on the board
  - Do NOT invalidate `['board', boardId]` unless Card data is embedded in board query

### Database Schema

**checklists table:**
```sql
CREATE TABLE `checklists` (
  `id` int NOT NULL AUTO_INCREMENT,
  `title` varchar(255) NOT NULL,
  `card_id` int NOT NULL,
  `created_at` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB;
```

**checklist_items table:**
```sql
CREATE TABLE `checklist_items` (
  `id` int NOT NULL AUTO_INCREMENT,
  `text` varchar(255) NOT NULL,
  `is_completed` tinyint NOT NULL DEFAULT 0,
  `checklist_id` int NOT NULL,
  `position` int NOT NULL DEFAULT 0,
  `created_at` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  `updated_at` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB;
```

**FK and Index statements** (separate ALTER TABLE, matching existing conventions):
```sql
ALTER TABLE `checklists` ADD CONSTRAINT `FK_checklists_card_id` FOREIGN KEY (`card_id`) REFERENCES `cards`(`id`) ON DELETE CASCADE ON UPDATE NO ACTION;
ALTER TABLE `checklist_items` ADD CONSTRAINT `FK_checklist_items_checklist_id` FOREIGN KEY (`checklist_id`) REFERENCES `checklists`(`id`) ON DELETE CASCADE ON UPDATE NO ACTION;
CREATE INDEX `IDX_checklists_card_id` ON `checklists`(`card_id`);
CREATE INDEX `IDX_checklist_items_checklist_id` ON `checklist_items`(`checklist_id`);
```

### Backend Implementation

**Checklist Entity:**
```typescript
// backend/src/checklists/entities/checklist.entity.ts
import {
  Entity, PrimaryGeneratedColumn, Column, CreateDateColumn, UpdateDateColumn,
  ManyToOne, OneToMany, JoinColumn,
} from 'typeorm';
import { ApiProperty } from '@nestjs/swagger';
import { Card } from '../../cards/entities/card.entity';
import { ChecklistItem } from './checklist-item.entity';

@Entity('checklists')
export class Checklist {
  @ApiProperty({ example: 1 })
  @PrimaryGeneratedColumn()
  id!: number;

  @ApiProperty({ example: 'Setup steps' })
  @Column({ length: 255 })
  title!: string;

  @ApiProperty({ example: 1 })
  @Column()
  card_id!: number;

  @ManyToOne(() => Card, (card) => card.checklists, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'card_id' })
  card!: Card;

  @OneToMany(() => ChecklistItem, (item) => item.checklist, { cascade: true })
  items!: ChecklistItem[];

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @CreateDateColumn()
  created_at!: Date;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @UpdateDateColumn()
  updated_at!: Date;
}
```

**ChecklistItem Entity:**
```typescript
// backend/src/checklists/entities/checklist-item.entity.ts
import {
  Entity, PrimaryGeneratedColumn, Column, CreateDateColumn, UpdateDateColumn,
  ManyToOne, JoinColumn,
} from 'typeorm';
import { ApiProperty } from '@nestjs/swagger';
import { Checklist } from './checklist.entity';

@Entity('checklist_items')
export class ChecklistItem {
  @ApiProperty({ example: 1 })
  @PrimaryGeneratedColumn()
  id!: number;

  @ApiProperty({ example: 'Buy milk' })
  @Column({ length: 255 })
  text!: string;

  @ApiProperty({ example: false })
  @Column({ default: false })
  is_completed!: boolean;

  @ApiProperty({ example: 1 })
  @Column()
  checklist_id!: number;

  @ManyToOne(() => Checklist, (checklist) => checklist.items, { onDelete: 'CASCADE' })
  @JoinColumn({ name: 'checklist_id' })
  checklist!: Checklist;

  @ApiProperty({ example: 0 })
  @Column({ default: 0 })
  position!: number;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @CreateDateColumn()
  created_at!: Date;

  @ApiProperty({ example: '2026-01-01T00:00:00.000Z' })
  @UpdateDateColumn()
  updated_at!: Date;
}
```

### API Endpoints

**Checklists:**
- `POST /api/cards/:cardId/checklists` — Create checklist
- `DELETE /api/checklists/:id` — Delete checklist

**Checklist Items:**
- `POST /api/checklists/:checklistId/items` — Create item
- `PATCH /api/checklist-items/:id` — Update item (toggle completion)
- `DELETE /api/checklist-items/:id` — Delete item

### Frontend Components

**ProgressBar Component:**
```typescript
// frontend/src/features/checklists/progress-bar.tsx
interface ProgressBarProps {
  completed: number;
  total: number;
  showLabel?: boolean;
}

// Returns:
// - 0% (empty checklist / total=0): Empty bar (gray), no division error
// - 1-99%: Teal bar with percentage label
// - 100%: Green bar with checkmark icon
// Edge case: total=0 → pct=0 (guard before division)
```

**CardPreview Progress Badge:**
```typescript
// Show aggregated progress across all checklists on the card
// Format: "3/5" or "100%" with checkmark
// Color: teal for partial, green for complete
```

### State Management

- Use React Query mutations for all checklist operations
- Optimistic updates: Update cache immediately, rollback on error
- Query invalidation: Use `['checklists', cardId]` for checklist data, `['cards']` for board cards, and `['columns']` for column/card counts. Invalidate all relevant keys after mutations so the detail panel and board stay in sync.
- Progress calculation: Client-side aggregation from checklist items for the detail panel. Guard against `total === 0` returning `NaN`.
- Board cards should receive a checklist progress summary so the board view does not need to fetch every checklist item for every card.
- Keep board progress summary in sync with the detail payload so a checklist edit immediately updates the card face.
- The detail panel should fetch the rich card response shape that includes full checklist titles and items.

### Accessibility

- Checklist items: Native `<input type="checkbox">` with associated label (handles `role="checkbox"`, `aria-checked`, keyboard natively)
- Or use custom `role="checkbox"` with `aria-checked` if styling requires it
- Checklist title and item text fields must have clear ARIA labels
- Progress bar: `role="progressbar"` with `aria-valuenow`, `aria-valuemin`, `aria-valuemax`
- Delete item button: `aria-label="Delete item"` with undo toast on action
- Delete checklist button: `aria-label="Delete checklist"` with confirmation dialog
- Keyboard navigation: Tab through items, Space to toggle native checkbox

### Previous Story Intelligence

- Story 4.4 (Due Dates) established patterns for:
  - CardDetailPanel integration: `<Separator />` + `<div className="space-y-2">` + `<h3>` section pattern
  - Badge components for card face display in CardPreview
  - React Query mutations with per-feature hook file
  - DueDatePicker popover placement below section heading
- Story 4.3 (Color-Coded Labels) established:
  - Feature-based component structure under `features/labels/`
  - API file (`labels.api.ts`) + hooks (`use-labels.ts`) pattern
  - Popover-based picker component
- Story 4.1 (Card Detail Panel) established:
  - Sheet-based panel structure with ScrollArea
  - Section organization with `<Separator />` dividers
  - Auto-save on blur for inline edits

### Git Intelligence

- Recent commits: Story 4.4 completed (due dates)
- Frontend: React 19, React Query v5, shadcn/ui (radix primitives + cva), Tailwind CSS v4 (@tailwindcss/vite plugin), Vite 8, Vitest + @testing-library/react, Playwright E2E
- Backend: NestJS 11, TypeORM 0.3, MySQL 8 (mysql2), Fastify adapter, class-validator, bcryptjs, @fastify/session
- Path aliases: `@/` → `src/` (both Vite + tsconfig)
- Code style: single quotes, trailing commas, printWidth 100, explicit semicolons
- Tests co-located with source files (`.spec.ts` / `.test.tsx`)
- E2E tests in `frontend/e2e/<feature>.spec.ts` with shared `test-utils.ts`
- For this feature, keep the board response light and the detail response rich to avoid unnecessary card-face payload growth.

## References

- [Source: epics.md#Story 4.5: Checklists with Progress]
- [Source: architecture.md#Checklists Module — entities, endpoints]
- [Source: architecture.md#Card Entity — checklists relation]
- [Source: architecture.md#Database Migration Strategy]
- [Source: ux-design-specification.md#Component Strategy — Checklist with progress bar]
- [Source: project-context.md#Critical Implementation Rules]
- [Source: 4-4-due-dates.md — CardDetailPanel integration patterns]
- [Source: 4-3-color-coded-labels.md — Badge and feature patterns]

## Dev Agent Record

### Agent Model Used

Codex GPT-5

### Debug Log References

Frontend checklist-focused Vitest run passed after replacing the stale card-detail placeholder test and adding shared progress-bar coverage.
Backend `checklists.service.spec.ts` passed with `--watchman=false`.
Checklist e2e run was blocked by environment DB access: `connect EPERM 127.0.0.1:3306`.

### Completion Notes List

- Replaced the stale checklist placeholder flow in the card detail panel with a real checklist section wrapper and conditional "Add Checklist" trigger.
- Added a shared progress bar component that handles `0/0`, partial, and complete states, including the 100% checkmark.
- Fixed item deletion undo so it restores the checklist item instead of attempting to update a deleted row.
- Updated checklist and card preview rendering to use the shared progress component and show aggregate progress consistently.
- Updated tests to cover the new checklist section toggle and progress-bar states.

### File List

Backend:
- `backend/src/checklists/checklists.service.spec.ts` — existing tests verified

Frontend:
- `frontend/src/features/checklists/checklist-section.tsx` — new
- `frontend/src/features/checklists/progress-bar.tsx` — new
- `frontend/src/features/checklists/checklist.tsx` — modified
- `frontend/src/features/checklists/checklist-item.tsx` — modified
- `frontend/src/features/checklists/index.ts` — modified
- `frontend/src/features/cards/card-detail-panel.tsx` — modified
- `frontend/src/features/cards/card-detail-panel.test.tsx` — modified
- `frontend/src/features/cards/card-preview.tsx` — modified
- `frontend/src/features/cards/card-preview.test.tsx` — modified
- `frontend/src/features/checklists/progress-bar.test.tsx` — new
