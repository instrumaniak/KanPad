# Story 4.1: Card Detail Panel

Status: done

---

## Story

As a user,
I want to open a detailed view of a card,
so that I can edit all card properties in one place.

---

## Acceptance Criteria

1. **Given** I am viewing a card on the board  
   **When** I click on the card  
   **Then** a sheet panel opens from the right side  
   **And** the sheet shows the card title (editable inline)  
   **And** the sheet shows sections for: Description, Labels, Due Date, Checklist  
   **And** the sheet is scrollable for long content  
   **And** clicking outside the sheet or pressing Escape closes it  
   **And** changes are saved automatically on blur

2. **Given** I am editing the card title in the detail panel  
   **When** I modify the text and leave the field  
   **Then** the title is saved via PATCH `/api/cards/:id`  
   **And** the board view updates to show the new title

3. **Given** I am viewing the card detail panel  
   **When** I look at the Description section  
   **Then** I see a textarea for entering plain-text description  
   **And** the description saves on blur via PATCH `/api/cards/:id`

4. **Given** I am viewing the card detail panel  
   **When** I look at the Labels section  
   **Then** I see a placeholder area indicating labels will be added in a future update

5. **Given** I am viewing the card detail panel  
   **When** I look at the Due Date section  
   **Then** I see the due date displayed if set, or "No due date" placeholder

6. **Given** I am viewing the card detail panel  
   **When** I look at the Checklist section  
   **Then** I see a placeholder area indicating checklists will be added in a future update

---

## Tasks / Subtasks

- [x] Database: Add `description` (TEXT, nullable) and `due_date` (DATETIME, nullable) to `cards` table via TypeORM migration
- [x] Backend: Update `Card` entity with new fields
- [x] Backend: Update `CreateCardDto` and `UpdateCardDto` with new fields
- [x] Backend: Update `CardsController` to include new fields in responses
- [x] Backend: Run migration locally and verify
- [x] Frontend: Update `Card` interface in `cards.api.ts` with new fields
- [x] Frontend: Create `CardDetailPanel` component using shadcn `Sheet`
- [x] Frontend: Add title editing in detail panel (inline input, auto-save on blur)
- [x] Frontend: Add description textarea section in detail panel (auto-save on blur)
- [x] Frontend: Add Due Date display section (show formatted date or placeholder)
- [x] Frontend: Add Labels placeholder section
- [x] Frontend: Add Checklist placeholder section
- [x] Frontend: Integrate detail panel into `Card` component (click to open)
- [x] Frontend: Ensure panel is scrollable for long content
- [x] Frontend: Handle Escape key and outside-click to close panel
- [x] Tests: Add backend unit tests for updated DTOs and entity
- [x] Tests: Add frontend tests for `CardDetailPanel` rendering and interactions

---

## Dev Notes

### Architecture Compliance
- **NestJS module pattern**: Extend existing `src/cards/` module. No new modules needed.
- **Feature-based frontend**: Create `frontend/src/features/cards/card-detail-panel.tsx`
- **API pattern**: Use existing `PATCH /api/cards/:id` endpoint. Extend `UpdateCardData` interface.
- **React Query**: Use `useUpdateCard()` hook with `onSuccess` invalidation of `['cards', columnId]`
- **shadcn/ui**: Use `Sheet` component for right-side panel. Use `Textarea` for description. Use `ScrollArea` for scrollable content. Use `Separator` between sections.

### Database Migration Required
```bash
# Generate migration after updating Card entity
npm run migration:generate -- src/migrations/1710825600010-AddDescriptionAndDueDateToCards
```

Migration must add:
- `description` TEXT NULL to `cards` table
- `due_date` DATETIME NULL to `cards` table

### Card Entity Extension
Current `Card` entity has: `id`, `title`, `column_id`, `position`, `created_at`, `updated_at`
Add:
- `description` (Column, type: 'text', nullable: true)
- `due_date` (Column, type: 'datetime', nullable: true)

### DTO Updates
`UpdateCardDto`:
- Add `description?: string` (IsOptional, IsString)
- Add `due_date?: Date | string` (IsOptional, IsDateString or use transformer)

`CreateCardDto`:
- Add `description?: string` (IsOptional, IsString)
- Add `due_date?: Date | string` (IsOptional)

### Frontend Card Interface Update
```typescript
export interface Card {
  id: number;
  title: string;
  column_id: number;
  position: number;
  description: string | null;
  due_date: string | null;
  created_at: string;
  updated_at: string;
}
```

### CardDetailPanel Component Structure
```
<Sheet>
  <SheetContent side="right" className="w-[400px] sm:w-[540px]">
    <ScrollArea className="h-full">
      {/* Title Section */}
      <input value={title} onChange={...} onBlur={saveTitle} />
      
      <Separator />
      
      {/* Description Section */}
      <h3>Description</h3>
      <Textarea value={description || ''} onChange={...} onBlur={saveDescription} />
      
      <Separator />
      
      {/* Labels Section (placeholder) */}
      <h3>Labels</h3>
      <p className="text-muted-foreground text-sm">Labels will be available in a future update.</p>
      
      <Separator />
      
      {/* Due Date Section */}
      <h3>Due Date</h3>
      {dueDate ? <Badge>{formattedDate}</Badge> : <p className="text-muted-foreground text-sm">No due date set</p>}
      
      <Separator />
      
      {/* Checklist Section (placeholder) */}
      <h3>Checklist</h3>
      <p className="text-muted-foreground text-sm">Checklists will be available in a future update.</p>
    </ScrollArea>
  </SheetContent>
</Sheet>
```

### Integration Point
Modify `frontend/src/features/cards/card.tsx`:
- Change `handleClick` to open detail panel instead of inline title editing
- Remove inline title editing logic from card face (keep it for later if needed, or move entirely to detail panel)
- The card face should now just show title (and any existing metadata)

**Important**: Currently `card.tsx` has inline title editing triggered by click. For this story, clicking a card should open the detail panel. The inline editing on the board view should be removed or changed to only trigger on a specific action (e.g., double-click or edit button). Per UX spec, "Click → open detail panel. Drag → move between columns/reorder."

### State Management
- Use local state in `CardDetailPanel` for form fields
- Auto-save on blur: call `updateCard.mutate()` with changed fields only
- Show subtle "Saving..." indicator during mutation
- On success, invalidate card queries so board view updates

### Accessibility
- Sheet should have `aria-label="Card details"`
- Title input should have `aria-label="Card title"`
- Description textarea should have `aria-label="Card description"`
- Focus should return to the card that opened the panel when closed

### File Structure
Backend:
- `backend/src/cards/entities/card.entity.ts` — add fields
- `backend/src/cards/dto/create-card.dto.ts` — add fields
- `backend/src/cards/dto/update-card.dto.ts` — add fields
- `backend/src/cards/cards.controller.ts` — update response mapping
- `backend/src/cards/cards.service.ts` — may need minor updates if DTO changes
- `backend/src/migrations/...-AddDescriptionAndDueDateToCards.ts` — new migration

Frontend:
- `frontend/src/features/cards/card-detail-panel.tsx` — new component
- `frontend/src/features/cards/card.tsx` — modify click handler
- `frontend/src/features/cards/cards.api.ts` — update interfaces
- `frontend/src/features/cards/use-cards.ts` — update types if needed

### Previous Story Intelligence
- Story 3.5 (Card Deletion) established the card menu pattern with DropdownMenu and AlertDialog
- The card component currently has inline editing on click — this must change to open detail panel
- The `useUpdateCard` hook supports optimistic updates and query invalidation
- API error handling follows the pattern: `onError` shows toast with error message
- The `CardDraggable` wrapper must not interfere with click-to-open panel

### Git Intelligence
- Recent commits show Fastify platform migration completed
- Backend uses NestJS with TypeORM, strict TypeScript
- Frontend uses React Query, shadcn/ui, Tailwind CSS
- Code style: single quotes, trailing commas, printWidth 100
- Tests co-located with source files

---

## Dev Agent Record

### Agent Model Used

(opencodelm)

### Completion Notes List

- [x] Migration generated and tested locally (`1778200000000-AddDescriptionAndDueDateToCards`)
- [x] Backend entity and DTOs updated with validation (`@IsOptional`, `@IsString`, `@IsDateString`)
- [x] Controller response mapping updated to include `description` and `due_date` in all endpoints
- [x] Frontend Card interface updated with `description: string | null` and `due_date: string | null`
- [x] CardDetailPanel component created with Sheet, ScrollArea, Textarea, Separator
- [x] Title editing implemented with auto-save on blur (reverts to original if emptied)
- [x] Description textarea implemented with auto-save on blur (saves `null` when cleared)
- [x] Due Date display section implemented with `Badge` and `toLocaleDateString` formatting
- [x] Labels and Checklist placeholder sections added
- [x] Card click handler updated to open detail panel; drag distance check preserved
- [x] Inline editing removed from card face (moved entirely to detail panel)
- [x] Escape and outside-click close handlers implemented via Radix Dialog/Sheet primitives
- [x] Accessibility attributes added (`aria-label="Card details"`, `aria-label="Card title"`, `aria-label="Card description"`)
- [x] Backend tests updated/added (20 tests passing in cards module, 179 total backend tests passing)
- [x] Frontend tests added for CardDetailPanel (14 tests) and card.test.tsx updated (existing tests pass)
- [x] All existing tests pass (backend: 179/179; frontend: 231/233 — 2 pre-existing auth form failures unchanged)
- [x] New shadcn components created: `Sheet`, `Textarea`, `ScrollArea`, `Separator`
- [x] `ResizeObserver` mock added to `frontend/src/test-setup.ts` for jsdom compatibility
- [x] Code review completed — 16 patch findings fixed, 3 deferred, 6 rejected
- [x] DTOs: Added `@MaxLength(10000)` on description, `@ValidateIf` for null-safe validation, custom `@IsValidDate()` decorator
- [x] Controller: Extracted `toCardResponse()` helper to eliminate duplicated mapping logic
- [x] Service: Wrapped `update()` in `dataSource.transaction()` for atomicity; helpers accept optional `EntityManager`
- [x] Detail panel: `latestCardRef` for stale closure fix, `isDirtyRef` dirty guard, `isMountedRef` for unmount safety, blur guards, `onError` with toast
- [x] Card component: Delete closes panel, undo restores `description`/`due_date`, NaN gate fixed for null `pointerDownPos`
- [x] Placeholder text corrected to "No due date" per AC5
- [x] All tests passing after review fixes (backend: 20/20 cards module; frontend: 25/25 cards module)

### File List

Backend:
- `backend/src/cards/entities/card.entity.ts` — added `description` and `due_date` columns
- `backend/src/cards/dto/create-card.dto.ts` — added optional `description` and `due_date` fields with validation
- `backend/src/cards/dto/update-card.dto.ts` — added optional `description` and `due_date` fields with validation
- `backend/src/cards/cards.controller.ts` — updated `CardResponse` and all endpoint mappings to include new fields
- `backend/src/cards/cards.service.ts` — updated `create` and `update` to persist new fields
- `backend/src/database/typeorm-registry.ts` — registered new migration
- `backend/src/migrations/1778200000000-AddDescriptionAndDueDateToCards.ts` — new migration (ran successfully in MySQL)
- `backend/src/cards/cards.controller.spec.ts` — updated test fixtures with new fields
- `backend/src/cards/cards.service.spec.ts` — updated test fixtures with new fields; added `remove` tests

Frontend:
- `frontend/src/features/cards/card-detail-panel.tsx` — new component
- `frontend/src/features/cards/card-detail-panel.test.tsx` — new tests (14 passing)
- `frontend/src/features/cards/card.tsx` — click opens detail panel; removed inline editing
- `frontend/src/features/cards/card.test.tsx` — updated to test detail panel open instead of inline editing
- `frontend/src/features/cards/cards.api.ts` — updated `Card`, `CreateCardData`, `UpdateCardData` interfaces
- `frontend/src/components/ui/sheet.tsx` — new shadcn component
- `frontend/src/components/ui/textarea.tsx` — new shadcn component
- `frontend/src/components/ui/scroll-area.tsx` — new shadcn component
- `frontend/src/components/ui/separator.tsx` — new shadcn component
- `frontend/src/test-setup.ts` — added `ResizeObserver` mock

---

## References

- [Source: epics.md#Story 4.1: Card Detail Panel]
- [Source: architecture.md#Core Entities]
- [Source: architecture.md#Database Migration Strategy]
- [Source: ux-design-specification.md#Component Strategy]
- [Source: ux-design-specification.md#Form Patterns]
- [Source: project-context.md#Critical Implementation Rules]
