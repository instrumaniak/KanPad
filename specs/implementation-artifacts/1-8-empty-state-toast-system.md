# Story 1.8: Empty State & Toast System

Status: done

## Story

As a user,
I want helpful empty states and consistent toast notifications across the app,
so that I know what to do next and get clear feedback on my actions.

## Acceptance Criteria

1. **Given** I have no projects, **When** I view the projects page, **Then** I see an empty state with illustration, "Start organizing" headline, and "Create your first project" CTA
2. **Given** I have no boards in a project, **When** I view the project page, **Then** I see an empty state with illustration, "This project is empty" headline, and "Create a board" CTA
3. **Given** I have no cards on a board, **When** I view the board, **Then** I see an inline prompt "Add your first task" with input pre-focused
4. **Given** I perform a create or update action, **When** the action completes, **Then** a success toast appears in the bottom-right and auto-dismisses after 3 seconds
5. **Given** I perform a destructive action (delete), **When** the action completes, **Then** a destructive toast appears with "Undo" button and auto-dismisses after 5 seconds
6. **Given** I click "Undo" on a destructive toast, **When** within the 5-second window, **Then** the action is reversed and a success toast confirms restoration
7. **Given** an operation fails, **When** the error occurs, **Then** a red error toast appears and requires manual dismiss
8. **Given** I view any page that loads data, **When** data is fetching, **Then** skeleton screens appear matching the expected layout

## Tasks / Subtasks

- [x] Task 1: Extract reusable EmptyState component (AC: 1, 2)
  - [x] Create `frontend/src/components/empty-state.tsx`
  - [x] Props: `icon` (ReactNode), `headline` (string), `description` (string), `action` (CTA button props)
  - [x] Centered layout: icon circle + headline + description + primary CTA button
  - [x] Use CSS variables for theming (dark mode support)
  - [x] Follow existing pattern from `project-list.tsx` EmptyState component (lines 264-280)

- [x] Task 2: Extract reusable LoadingSkeleton component (AC: 8)
  - [x] Create `frontend/src/components/loading-skeleton.tsx`
  - [x] Props: `count` (number of skeleton rows, default 3), `className` (optional)
  - [x] Render `count` animated pulse bars matching card/row height
  - [x] Follow existing pattern from `project-list.tsx` LoadingSkeleton component (lines 282-293)

- [x] Task 3: Fix toast auto-dismiss timing (AC: 4, 5, 7)
  - [x] Update `frontend/src/components/ui/use-toast.tsx`
  - [x] Change `setTimeout` logic: success/default → 3000ms, destructive/error → manual (no auto-dismiss or 5000ms for destructive with action)
  - [x] Success + default: 3s auto-dismiss
  - [x] Destructive with action (Undo): 5s auto-dismiss
  - [x] Error: no auto-dismiss (manual dismiss only)
  - [x] Keep existing action button support for Undo

- [x] Task 4: Create toast helper function (AC: 4, 5, 7)
  - [x] Add convenience functions to `use-toast.tsx` or create `frontend/src/lib/toast-helpers.ts`
  - [x] `showSuccess(title, description?)` — success type, 3s auto-dismiss
  - [x] `showError(title, description?)` — error type, manual dismiss
  - [x] `showDestructive(title, onUndo)` — destructive type with Undo action, 5s auto-dismiss
  - [x] All functions wrap the existing `toast()` call with correct defaults

- [x] Task 5: Refactor project-list.tsx to use shared components (AC: 1)
  - [x] Replace inline `EmptyState` with imported `EmptyState` from `components/empty-state`
  - [x] Replace inline `LoadingSkeleton` with imported `LoadingSkeleton` from `components/loading-skeleton`
  - [x] Update toast calls to use helper functions where appropriate
  - [x] Verify existing tests still pass

- [x] Task 6: Add tests for EmptyState component (AC: 1, 2)
  - [x] Create `frontend/src/components/empty-state.test.tsx`
  - [x] Test: renders icon, headline, description, and CTA button
  - [x] Test: CTA button click fires callback
  - [x] Test: renders correctly in dark mode (CSS variable check)

- [x] Task 7: Add tests for toast helpers (AC: 4, 5, 7)
  - [x] Create `frontend/src/lib/toast-helpers.test.ts` or inline in `use-toast.test.tsx`
  - [x] Test: showSuccess creates toast with type 'success'
  - [x] Test: showError creates toast with type 'error'
  - [x] Test: showDestructive creates toast with action and type 'destructive'

## Dev Notes

### Previous Story Intelligence

**From Story 1.7 (Project CRUD):**
- `use-toast.tsx` is a custom React context-based toast system — NOT shadcn/ui's toast. It already supports `type` (default, success, error, destructive) and `action` (for Undo button).
- Current auto-dismiss is hardcoded to 5000ms for ALL types — this needs to be type-aware.
- `EmptyState` component exists inline in `project-list.tsx` (lines 264-280): centered layout with icon circle, headline, description, and CTA button. Uses `FolderKanban` icon from lucide-react.
- `LoadingSkeleton` component exists inline in `project-list.tsx` (lines 282-293): renders 3 animated pulse bars.
- Delete undo flow: `recreateProject()` in `projects.api.ts` re-creates project by name after delete — this pattern will be reused for future destructive actions.
- 51 frontend tests pass — new tests must not break them.

[Source: 1-7-project-crud.md]

**From Story 1.6 (App Shell & Navigation):**
- AppLayout wraps pages with `<Outlet />` — empty states render inside page components, not layout.
- Sidebar shows project list — empty state affects what sidebar displays too.

[Source: 1-6-app-shell-navigation.md]

**From Story 1.5 (Design System Foundation):**
- CSS variables define Soft Teal palette with light/dark tokens
- `use-theme` hook at `frontend/src/hooks/use-theme.ts`
- shadcn/ui components available: Button, Input, Dialog, Badge, DropdownMenu
- Toast system is custom (not shadcn/ui's) — built as React context provider

[Source: 1-5-design-system-foundation.md]

### Architecture Compliance

**Frontend Organization (from architecture.md):**
- Feature-based folders under `src/features/`
- Shared components go in `src/components/` (NOT `components/ui/` which is shadcn-owned)
- Co-located tests: `.test.tsx` next to components
- Shared utilities in `lib/`

[Source: architecture.md#Frontend Organization]

**Files to create:**
```
frontend/src/components/
├── empty-state.tsx              # Reusable empty state component
├── empty-state.test.tsx         # Tests
├── loading-skeleton.tsx         # Reusable skeleton component
└── ui/
    └── use-toast.tsx            # MODIFY: fix timing, add helpers
```

**Optional helper file:**
```
frontend/src/lib/
└── toast-helpers.ts             # Convenience toast functions
```

**Files to modify:**
```
frontend/src/features/projects/project-list.tsx  # Use shared components
```

[Source: architecture.md#Complete Project Directory Structure]

**API Response Format (from architecture.md):**
```typescript
// Error response
{ statusCode: number, message: string | string[], error: string }
```

Error toasts should display `message` field from API error responses.

[Source: architecture.md#API Response Formats]

**Naming Conventions (from architecture.md):**
- Frontend files: `kebab-case` (`empty-state.tsx`)
- Components: `PascalCase` (`EmptyState`)
- Co-located tests: `.test.tsx` suffix

[Source: architecture.md#Naming Patterns]

### UX Requirements

**From UX Design Specification:**

**Empty States (UX-DR10):**
| Context | Headline | CTA |
|---------|----------|-----|
| No projects | "Start organizing" | "Create your first project" |
| No boards in project | "This project is empty" | "Create a board" |
| No cards on board | "Add your first task" | Inline input pre-focused |
| No search results | "No cards found" | "Clear search" |
| No filter matches | "No cards match your filters" | "Clear all filters" |

Rules:
- Every empty state has: illustration + headline + ONE clear CTA
- Empty states are inviting, not intimidating
- CTAs are contextual (not generic "Get started")

[Source: ux-design-specification.md#Empty States]

**Toast Notifications (UX-DR17):**
| Type | Visual | Duration | Example |
|------|--------|----------|---------|
| Success | Green toast, bottom-right | 3s auto-dismiss | "Card created" |
| Error | Red toast, bottom-right | Manual dismiss | "Failed to save. Retry?" |
| Warning | Amber toast, bottom-right | 5s auto-dismiss | "Unsaved changes" |
| Info | Neutral toast, bottom-right | 3s auto-dismiss | "3 cards moved" |
| Undo | Toast with "Undo" button | 5s auto-dismiss | "Card deleted. Undo?" |

Rules:
- Toasts never block interaction
- Undo available for all destructive actions for 5 seconds
- Error toasts require manual dismiss
- Success toasts auto-dismiss quickly

[Source: ux-design-specification.md#Feedback Patterns]

**Loading States:**
| Context | Pattern |
|---------|---------|
| Initial page load | Skeleton screens (gray shapes matching expected layout) |
| Saving changes | Spinner on save button, "Saving..." text |

[Source: ux-design-specification.md#Loading States]

### File Structure Requirements

**New shared component files:**
```
frontend/src/components/
├── empty-state.tsx              # NEW: Reusable EmptyState
├── empty-state.test.tsx         # NEW: Tests
├── loading-skeleton.tsx         # NEW: Reusable LoadingSkeleton
```

**Files to modify:**
```
frontend/src/components/ui/use-toast.tsx              # Fix timing, add helpers
frontend/src/features/projects/project-list.tsx       # Refactor to use shared components
```

**Existing files (reference only, DO NOT MODIFY):**
```
frontend/src/components/ui/button.tsx                 # shadcn — never modify
frontend/src/components/ui/dialog.tsx                 # shadcn — never modify
frontend/src/components/ui/input.tsx                  # shadcn — never modify
```

### Testing Requirements

**Framework:** Vitest + React Testing Library, co-located `.test.tsx` files

**EmptyState tests:**
- Renders icon, headline, description
- Renders CTA button with correct label
- CTA click fires callback
- Dark mode: uses CSS variables (no hardcoded colors)

**LoadingSkeleton tests:**
- Renders correct number of skeleton bars
- Has animate-pulse class

**Toast timing tests:**
- Success toast auto-dismisses after 3s
- Error toast does NOT auto-dismiss
- Destructive toast with action auto-dismisses after 5s

**Existing tests must continue to pass:**
- `project-list.test.tsx` — 7 tests
- `app-layout.test.tsx` — existing tests
- Total: 51+ frontend tests

[Source: project-context.md#Testing Rules]

### Critical Anti-Patterns

- ❌ NEVER use `any` type — strict TypeScript
- ❌ NEVER modify shadcn/ui components directly in `components/ui/`
- ❌ NEVER use hardcoded color values — use CSS variables / Tailwind tokens
- ❌ NEVER create custom Button when shadcn Button exists
- ❌ NEVER fetch data in layout components
- ❌ NEVER override Prettier formatting — shared config at root

[Source: project-context.md#Critical Don't-Miss Rules]

### Performance Considerations

- Toast auto-dismiss uses `setTimeout` — clear timeout on unmount to prevent memory leaks
- EmptyState is a static component — no performance concerns
- Skeleton uses CSS animation (animate-pulse) — GPU-accelerated, no JS cost
- React Query handles loading states via `isLoading` — no extra state management needed

[Source: architecture.md#Performance Requirements]

### Accessibility

- EmptyState: headline is semantic `<h2>`, CTA is a `<Button>` (keyboard accessible)
- Toast: close button has `aria-label="Close"`, Undo button is keyboard accessible
- Skeleton: `aria-busy="true"` on parent container, `aria-label="Loading"` on skeleton
- Error toast: focus is NOT stolen (toasts are non-blocking)

[Source: ux-design-specification.md#Accessibility Strategy]

### References

- [Source: epics.md#Story 1.8] — User story and acceptance criteria
- [Source: prd.md#User Interface] — FR40 (loading states), FR41 (error messages)
- [Source: architecture.md#Frontend Organization] — Feature-based structure
- [Source: architecture.md#Process Patterns] — Error handling, loading states
- [Source: ux-design-specification.md#Empty States] — Empty state design specs
- [Source: ux-design-specification.md#Feedback Patterns] — Toast notification specs
- [Source: ux-design-specification.md#Loading States] — Loading state patterns
- [Source: project-context.md#Framework-Specific Rules] — React rules
- [Source: project-context.md#Critical Don't-Miss Rules] — Anti-patterns
- [Source: 1-7-project-crud.md] — Previous story, existing toast/empty state implementations

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List
- Task 1: Extracted EmptyState component with icon, headline, description, and action props. Uses CSS variables for theming.
- Task 2: Extracted LoadingSkeleton component with count and className props. Includes aria-busy and aria-label for accessibility.
- Task 3: Updated toast auto-dismiss timing: success/default 3s, destructive with action 5s, error manual dismiss only.
- Task 4: Created useToastHelpers hook with showSuccess, showError, and showDestructive convenience functions.
- Task 5: Refactored project-list.tsx to use shared EmptyState and LoadingSkeleton components.
- Task 6: Added EmptyState tests covering rendering, CTA callback, no-button state, and dark mode CSS variables.
- Task 7: Added toast helper tests covering success, error, and destructive toast creation.

### File List
- frontend/src/components/empty-state.tsx (NEW)
- frontend/src/components/empty-state.test.tsx (NEW)
- frontend/src/components/loading-skeleton.tsx (NEW)
- frontend/src/components/ui/use-toast.tsx (MODIFIED)
- frontend/src/components/ui/toast-helpers.test.tsx (NEW)
- frontend/src/lib/toast-helpers.ts (NEW)
- frontend/src/features/projects/project-list.tsx (MODIFIED)
