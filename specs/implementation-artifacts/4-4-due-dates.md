# Story 4.4: Due Dates

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a user,
I want to set due dates on cards,
So that I can track deadlines and prioritize time-sensitive tasks.

## Acceptance Criteria

1. **Given** I am viewing the card detail panel
   **When** I click on the Due Date section
   **Then** a date picker appears allowing me to select a date

2. **Given** I select a due date
   **When** the date is saved
   **Then** the due date appears as a badge on the card
   **And** the badge displays the formatted date (e.g., "Jun 15, 2026")

3. **Given** a card has a due date in the past
   **When** I view the card
   **Then** the due date badge shows a warning color (red/orange)

4. **Given** a card has a due date
   **When** I want to remove it
   **Then** I can clear the due date
   **And** the badge disappears from the card

5. **Given** I view the board
   **When** cards have due dates
   **Then** due date badges are visible on the card face for quick visual scanning

## Tasks / Subtasks

- [ ] Frontend: Install `date-fns` dependency
- [ ] Frontend: Add shadcn/ui `Calendar` component (`npx shadcn@latest add calendar`)
- [ ] Frontend: Create `DueDatePicker` component (`src/features/cards/due-date-picker.tsx`)
  - [ ] Uses shadcn Calendar + Popover for date selection
  - [ ] Integrates with `date-fns` for formatting
  - [ ] Handles date selection and clearing
  - [ ] Shows "Clear date" button when date is set
- [ ] Frontend: Integrate `DueDatePicker` into `CardDetailPanel` (`card-detail-panel.tsx`)
  - [ ] Replace inline `toLocaleDateString()` with `DueDatePicker`
  - [ ] Add "No due date" placeholder when date is null
- [ ] Frontend: Add due date badge to `CardPreview` (`card-preview.tsx`)
  - [ ] Show formatted date (e.g., "Jun 15, 2026") or "Today" if due today
  - [ ] Past-due: red/warning color
  - [ ] Due today: orange color with "Today" text
  - [ ] Future: default gray
- [ ] Frontend: Add `useUpdateCardDueDate` hook or extend existing `useUpdateCard`
- [ ] Tests: Frontend unit tests for `DueDatePicker` component
- [ ] Tests: Frontend unit tests for due date badge rendering
  - [ ] Test "Today" text for cards due today
  - [ ] Test formatted date for past/future dates
- [ ] Tests: E2E test for setting, displaying, and clearing due dates

## Dev Notes

### Architecture Compliance

- **No backend changes required**: The `due_date` column already exists in the `cards` table, the `Card` entity has `due_date!: Date | null`, and DTOs accept ISO 8601 strings with `@IsValidDate()` validation.
- **Feature-based frontend**: Create `DueDatePicker` component in `src/features/cards/components/` or `src/features/cards/`.
- **React Query**: Use existing `useUpdateCard()` hook pattern for updating due dates.
- **shadcn/ui**: Use `Calendar` component (built on `react-day-picker`) with `Popover` for the date picker trigger.

### Date Library Decision

**Use `date-fns` (latest)** for the following reasons:
1. **shadcn/ui requires it**: The Calendar component uses `react-day-picker` which has a hard dependency on `date-fns`
2. **Bundle size**: 3-5 KB gzipped (typical usage) vs 17+ KB for Luxon
3. **Tree-shaking**: ESM-first v4 with `sideEffects: false` works perfectly with Vite 8
4. **TypeORM compatibility**: Works directly with native `Date` objects used by TypeORM
5. **Community**: 86M weekly downloads, largest post-Moment ecosystem

**No date library needed on backend**: Backend uses native `Date` objects and ISO 8601 strings for serialization. No date manipulation required.

### No Database Migration Required

The `due_date` column was already added in migration `1710825600002-AddDueDateToCards.ts`. No schema changes needed.

### Backend (Already Complete)

The backend already supports due dates:
- **Card entity**: `due_date!: Date | null` field with `datetime` column type
- **DTOs**: Accept `due_date?: string` with `@IsValidDate()` ISO 8601 validation
- **Service**: Converts string to `Date` with `new Date(dto.due_date)`
- **Controller**: Serializes with `.toISOString()` to `CardResponse`

### Frontend Components to Modify/Create

1. **DueDatePicker** (new component):
   ```tsx
   // frontend/src/features/cards/due-date-picker.tsx
   interface DueDatePickerProps {
     dueDate: string | null;
     onDateChange: (date: string | null) => void;
     disabled?: boolean;
   }
   // Uses shadcn Calendar + Popover
   // Shows formatted date (e.g., "Jun 15, 2026") or "No due date" placeholder
   // Displays "Clear date" button when date is set
   ```

2. **CardDetailPanel** (modify existing):
   - Replace lines 142-148 (inline `toLocaleDateString()`) with `DueDatePicker`
   - Add loading state during update
   - Show "Today" text when due date is today

3. **CardPreview** (modify existing):
   - Add due date badge after label badges
   - Show "Today" text when due date is today
   - Show formatted date (e.g., "Jun 15, 2026") for past/future dates
   - Color coding based on date comparison

### Due Date Badge Logic

```typescript
import { isPast, isToday, format } from 'date-fns';

interface DueDateBadge {
  text: string;
  className: string;
}

function getDueDateBadge(dueDate: string | null): DueDateBadge | null {
  if (!dueDate) return null;
  
  const date = new Date(dueDate);
  
  if (isToday(date)) {
    return {
      text: 'Today',
      className: 'bg-orange-100 text-orange-700 dark:bg-orange-950 dark:text-orange-400'
    };
  }
  
  if (isPast(date)) {
    return {
      text: format(date, 'MMM d, yyyy'), // e.g., "Jun 10, 2026"
      className: 'bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-400'
    };
  }
  
  return {
    text: format(date, 'MMM d, yyyy'), // e.g., "Jun 15, 2026"
    className: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300'
  };
}
```

### Date Formatting

```typescript
import { isToday, format } from 'date-fns';

// Display format for card badge and detail panel (consistent)
function formatDueDate(dueDate: string | null): string | null {
  if (!dueDate) return null;
  const date = new Date(dueDate);
  return isToday(date) ? 'Today' : format(date, 'MMM d, yyyy'); // e.g., "Today" or "Jun 15, 2026"
}

// ISO format for API
new Date(dueDate).toISOString(); // "2026-06-15T00:00:00.000Z"
```

### State Management

- Use existing `useUpdateCard()` hook to update `due_date` field
- Optimistic update: Update card in cache immediately, rollback on error
- Query invalidation: Invalidate `['cards', columnId]` and `['card', cardId]` on success

### Accessibility

- DueDatePicker trigger button: `aria-label="Set due date"`
- Calendar: Standard shadcn Calendar accessibility (keyboard navigation, screen reader support)
- Clear button: `aria-label="Clear due date"`
- Due date badge: `aria-label="Due date: Today"` or `aria-label="Due date: Jun 15, 2026"`

### Previous Story Intelligence

- Story 4.3 (Color-Coded Labels) established patterns for:
  - Feature-based component structure
  - React Query mutations with optimistic updates
  - Badge components (`LabelBadge`) for card face display
  - Popover-based pickers (`LabelPicker`)
- Story 4.2 (Markdown Description) upgraded `CardDetailPanel` with Tabs and react-markdown
- Story 4.1 (Card Detail Panel) established the Sheet-based panel structure

### Git Intelligence

- Recent commits: Story 4.3 completed (color-coded labels)
- Frontend: React Query, shadcn/ui, Tailwind CSS v4, Vite 8
- Backend: NestJS, TypeORM, Fastify
- Code style: single quotes, trailing commas, printWidth 100
- Tests co-located with source files

## References

- [Source: epics.md#Story 4.4: Due Dates]
- [Source: architecture.md#Card Entity — due_date field]
- [Source: architecture.md#DTOs — UpdateCardDto with due_date]
- [Source: architecture.md#Database Migration Strategy]
- [Source: ux-design-specification.md#Component Strategy — Card component]
- [Source: project-context.md#Critical Implementation Rules]
- [Source: 4-1-card-detail-panel.md — CardDetailPanel structure with Due Date placeholder]
- [Source: 4-3-color-coded-labels.md — Badge and Picker patterns]

## Dev Agent Record

### Agent Model Used

_(To be filled by dev agent)_

### Debug Log References

_(To be filled by dev agent)_

### Completion Notes List

_(To be filled by dev agent)_

### File List

Frontend (to be created/modified):
- `frontend/src/features/cards/due-date-picker.tsx` — new
- `frontend/src/features/cards/card-detail-panel.tsx` — modified (integrate DueDatePicker)
- `frontend/src/features/cards/card-preview.tsx` — modified (add due date badge)
- `frontend/src/features/cards/due-date-picker.test.tsx` — new
- `frontend/src/features/cards/card-preview.test.tsx` — modified (add due date badge tests)
- `frontend/e2e/due-dates.spec.ts` — new
