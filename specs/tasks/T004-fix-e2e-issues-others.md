# T004: Fix E2E Issues & Frontend Code Quality

## Goal

Fix critical frontend code quality issues identified during codebase exploration: API type duplication, type incompatibilities, over-invalidation, and accessibility issues. These fixes improve maintainability, prevent runtime bugs, and ensure consistent patterns across the codebase.

## Depends On

None

## Phase

0

## Critical

Yes

## Spec References

- `_bmad-output/planning-artifacts/architecture.md`
- `_bmad-output/planning-artifacts/ux-design-specification.md`

## Files to Create/Modify

### Create
- `frontend/src/lib/api.ts` - Shared API types and utilities

### Modify
- `frontend/src/features/boards/boards.api.ts` - Import shared types
- `frontend/src/features/cards/cards.api.ts` - Import shared types, export shared Card type
- `frontend/src/features/columns/columns.api.ts` - Import shared types, fix Card type
- `frontend/src/features/labels/labels.api.ts` - Import shared types
- `frontend/src/features/projects/projects.api.ts` - Import shared types
- `frontend/src/features/tags/tags.api.ts` - Import shared types
- `frontend/src/features/notes/notes.api.ts` - Import shared types
- `frontend/src/features/checklists/checklists.api.ts` - Import shared types
- `frontend/src/features/auth/auth.api.ts` - Import shared types
- `frontend/src/features/labels/use-labels.ts` - Fix over-invalidation
- `frontend/src/features/cards/add-card-input.tsx` - Fix document.querySelector
- `frontend/src/features/columns/column-header.tsx` - Fix manual dropdown menus
- `frontend/vitest.config.ts` - Add coverage configuration

## Implementation Steps

### Step 1: Create shared API utilities (`frontend/src/lib/api.ts`)

Create a single source of truth for API types and utilities:

```typescript
export interface ApiResponse<T> {
  data: T;
  message?: string;
}

export interface ListResponse<T> {
  data: T[];
  total: number;
}

export interface ApiError {
  statusCode: number;
  message: string | string[];
  error: string;
}

export const FETCH_OPTIONS: RequestInit = { credentials: 'include' };

export async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let message = 'Request failed';
    try {
      const error: ApiError = await response.json();
      message = Array.isArray(error.message) ? error.message.join(', ') : error.message;
    } catch {
      message = response.statusText || 'Request failed';
    }
    throw new Error(message);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  try {
    return await response.json();
  } catch {
    throw new Error('Unexpected response format');
  }
}

export async function apiFetch<T>(url: string, options?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(url, { ...FETCH_OPTIONS, ...options });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<T>(response);
}
```

### Step 2: Update all API files to use shared utilities

For each API file:
1. Remove local `ApiResponse`, `ListResponse`, `ApiError` definitions
2. Remove local `FETCH_OPTIONS` and `handleResponse` functions
3. Import from `@/lib/api`
4. Replace `fetch()` calls with `apiFetch()` helper

### Step 3: Fix Card type incompatibility

The `Card` type in `columns.api.ts` is missing fields:
- Missing: `description`, `checklists`
- `Label.color` is `string` instead of `LabelColor`

**Fix:**
1. In `cards.api.ts`, export the full `Card` and `Label` types
2. In `columns.api.ts`, import `Card` and `Label` from `cards.api.ts`

### Step 4: Fix over-invalidation in labels

Current issue: `useUpdateLabel` and `useDeleteLabel` invalidate ALL cards and columns globally.

**Fix:** Add comment explaining why global invalidation is necessary (can't know which cards have the label without fetching). Consider future optimization with new API endpoint.

### Step 5: Fix `document.querySelector` in add-card-input.tsx

Current issue (line 88-89): Uses `document.querySelector` to find next column's textarea.

**Fix:** Use a callback prop `onTabToNextColumn?: (nextColumnId: number) => void` pattern.

### Step 6: Fix manual dropdown menus in column-header.tsx

Current issue (lines 164-238): Manual implementation lacks accessibility.

**Fix:** Replace with Radix UI `DropdownMenu` component for proper keyboard navigation and ARIA attributes.

### Step 7: Add vitest coverage configuration

Add coverage configuration to `vitest.config.ts` with v8 provider.

## Constraints

- Must not break existing functionality
- Must maintain backward compatibility
- Must run full test suite after changes

## Acceptance Criteria

- [ ] All API files import from shared `@/lib/api` module
- [ ] No duplicate `ApiResponse`, `handleResponse`, or `FETCH_OPTIONS` definitions
- [ ] `Card` type is consistent across `cards.api.ts` and `columns.api.ts`
- [ ] `useUpdateLabel` and `useDeleteLabel` have documented invalidation strategy
- [ ] `add-card-input.tsx` does not use `document.querySelector`
- [ ] `column-header.tsx` uses Radix UI `DropdownMenu`
- [ ] `vitest.config.ts` has coverage configuration
- [ ] All existing tests pass
- [ ] TypeScript compilation succeeds

## Notes

### API Duplication Summary

| Type/Function | Files Duplicating |
|---------------|-------------------|
| `ApiResponse<T>` | boards, cards, columns, projects, auth |
| `ListResponse<T>` | boards, projects |
| `ApiError` | boards, projects, auth |
| `handleResponse<T>` | ALL 9 API files |
| `FETCH_OPTIONS` | ALL 9 API files |

### Card Type Incompatibility

| Field | `cards.api.ts` | `columns.api.ts` |
|-------|----------------|------------------|
| `description` | `string \| null` | **MISSING** |
| `checklists` | `ChecklistData[]` | **MISSING** |
| `Label.color` | `LabelColor` (union) | `string` |

### Over-invalidation Impact

When a label is updated:
- Current: Invalidates ALL cards (potentially 100s) and ALL columns (potentially 10s) across ALL boards
- Ideal: Only invalidate cards that have the specific label (requires new API endpoint)

### Accessibility Issues in column-header.tsx

Manual dropdown menus lack:
- `aria-haspopup`, `aria-expanded` attributes
- Keyboard navigation (arrow keys, Home/End)
- Focus trapping
- Escape key to close
- Proper role attributes

Radix UI `DropdownMenu` provides all of these automatically.
