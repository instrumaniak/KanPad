# Code Review: Story 3-5 Card Deletion

**Story**: 3-5 Card Deletion
**Date**: 2026-05-11
**Review Mode**: Full (with spec)
**Reviewer**: Amelia (Dev Agent)

---

## Fixable Issues

Issues are ordered by priority. Each issue includes exact file paths, line references, and concrete fix instructions.

---

### ISSUE-01: Missing `group` class — menu button invisible on hover

**Severity**: CRITICAL
**Category**: Bug
**File**: `frontend/src/features/cards/card.tsx`
**Location**: Parent `div` (role="button") wrapping the card content

**Problem**: The `MoreHorizontal` button uses `group-hover:opacity-100` but no parent element has the `group` CSS class. The menu button stays `opacity-0` permanently — delete action is undiscoverable via hover.

**Fix**: Add `group` to the parent `div` className:

```tsx
// BEFORE (missing 'group')
className={`rounded bg-card p-3 text-sm shadow-sm hover:bg-accent/50 cursor-pointer ...`}

// AFTER
className={`group rounded bg-card p-3 text-sm shadow-sm hover:bg-accent/50 cursor-pointer ...`}
```

**Verification**: Hover over a card in the board view — the three-dot menu button should fade in.

---

### ISSUE-02: AlertDialog says "cannot be undone" but Undo exists

**Severity**: HIGH
**Category**: Bug / UX
**File**: `frontend/src/features/cards/card.tsx`
**Location**: `AlertDialogDescription` text

**Problem**: Dialog text reads "This action cannot be undone" but the success toast provides an Undo action. This contradicts the actual behavior.

**Fix**: Replace the dialog description:

```tsx
// BEFORE
<AlertDialogDescription>
  This action cannot be undone.
</AlertDialogDescription>

// AFTER
<AlertDialogDescription>
  The card "{card.title}" will be deleted. You can restore it from the notification.
</AlertDialogDescription>
```

**Note**: If `card.title` can be empty (new card), use a fallback: `card.title || "this card"`.

---

### ISSUE-03: Undo doesn't restore card to original position

**Severity**: HIGH
**Category**: Intent Gap
**Files**:
- `frontend/src/features/cards/card.tsx` (undo handler, ~line 127)
- `frontend/src/features/cards/use-cards.ts` (optimistic update — need to snapshot position)
- `frontend/src/features/cards/cards.api.ts` (CreateCardData type, ~line 35)
- `backend/src/cards/dto/create-card.dto.ts` (if exists) or `backend/src/cards/cards.controller.ts`
- `backend/src/cards/cards.service.ts` and `backend/src/cards/cards.service.spec.ts`

**Problem**: The undo action calls `createCardMutation.mutateAsync({ title, column_id })` which creates a brand new card at the default position (end of column). AC2 requires the card to be restored to its original position. Additionally, the card gets a new ID — references to the old ID become stale.

**Fix** (multi-step):

1. **Extend `CreateCardData` to accept optional `position`**:
   ```ts
   // frontend/src/features/cards/cards.api.ts
   export interface CreateCardData {
     title: string;
     column_id: number;
     position?: number; // Added for undo/restore
   }
   ```

2. **Update backend to accept `position` in card creation**:
   - Add `position` as optional field in the DTO/entity
   - In `CardsService.create()`, if `position` is provided, use it instead of calculating default
   - Update `cards.service.spec.ts` to test card creation with position

3. **Capture position in the delete handler and pass it to undo**:
   ```tsx
   // card.tsx — handleDelete
   const deletedCard = { ...card }; // already captures card, which includes position

   // In undo action, include position:
   createCardMutation.mutateAsync({
     title: deletedCard.title,
     column_id: deletedCard.column_id,
     position: deletedCard.position,
   })
   ```

4. **Update undo toast message** from "Card restored" to "Card restored to original position"

**Verification**: Delete a card at position 2 in a column of 5 → click Undo → card should reappear at position 2, not at the bottom.

---

### ISSUE-04: Broken test — `waitFor` not awaited, no mutation assertion

**Severity**: HIGH
**Category**: Bug (Test)
**File**: `frontend/src/features/cards/card.test.tsx`
**Location**: Lines ~145-163 (the "calls delete mutation when confirming delete" test)

**Problem**:
1. `waitFor()` calls are not `await`ed — assertions inside may never execute
2. `fireEvent.click(deleteItem)` inside `waitFor` is an anti-pattern
3. No assertion that `mockDeleteMutate` was called
4. Test never clicks the AlertDialog confirm button, so `handleDelete` never runs

**Fix**: Rewrite the test:

```tsx
it('calls delete mutation when confirming delete', async () => {
  mockDeleteMutate.mockImplementation((id, opts) => {
    opts?.onSuccess?.();
  });

  renderWithProviders(<Card card={mockCard} index={0} />);

  // Open menu
  const menuButton = screen.getByRole('button', { name: /card menu/i });
  fireEvent.click(menuButton);

  // Click delete menu item
  const deleteItem = await screen.findByRole('menuitem', { name: /delete/i });
  fireEvent.click(deleteItem);

  // Confirm in AlertDialog
  const confirmBtn = await screen.findByRole('button', { name: /^delete$/i });
  fireEvent.click(confirmBtn);

  // Assert delete mutation called
  await waitFor(() => {
    expect(mockDeleteMutate).toHaveBeenCalledWith(
      mockCard.id,
      expect.objectContaining({
        onSuccess: expect.any(Function),
        onError: expect.any(Function),
      })
    );
  });
});
```

**Verification**: Test should pass and actually verify mutation invocation.

---

### ISSUE-05: Toast mock untestable — inline mock prevents assertions

**Severity**: MEDIUM
**Category**: Code Smell (Test)
**File**: `frontend/src/features/cards/card.test.tsx`
**Location**: Line ~10

**Problem**: `useToast: () => ({ toast: vi.fn() })` creates a new mock on every call. Cannot assert toast was called with specific arguments (e.g., verifying Undo action is offered).

**Fix**: Extract the mock to a named variable:

```tsx
const mockToast = vi.fn();

// In the mock setup:
useToast: () => ({ toast: mockToast }),
```

**Verification**: Can now assert `expect(mockToast).toHaveBeenCalledWith(expect.objectContaining({ title: 'Card deleted' }))`.

---

### ISSUE-06: `mockDeleteMutateObj` shares `mutateAsync` with other mocks

**Severity**: MEDIUM
**Category**: Code Smell (Test)
**File**: `frontend/src/features/cards/card.test.tsx`
**Location**: Line ~5

**Problem**: `mockDeleteMutateObj` reuses `mockMutateAsync` from the update/create mutation mock. Cannot distinguish which mutation's `mutateAsync` was called — undo-related assertions become unreliable.

**Fix**: Create a separate mock:

```tsx
const mockCreateMutateAsync = vi.fn();
const mockDeleteMutateAsync = vi.fn();

// In mock for useCreateCard:
mutateAsync: mockCreateMutateAsync,

// In mock for useDeleteCard return object:
mutateAsync: mockDeleteMutateAsync,
```

**Verification**: Can independently assert create vs delete `mutateAsync` calls.

---

### ISSUE-07: `async handleDelete` never awaits anything

**Severity**: MEDIUM
**Category**: Code Smell
**File**: `frontend/src/features/cards/card.tsx`
**Location**: ~line 81

**Problem**: `handleDelete` is declared `async` but `deleteCard.mutate()` is fire-and-forget with callbacks. The `async` keyword is misleading.

**Fix**: Remove the `async` keyword:

```tsx
// BEFORE
const handleDelete = async () => {

// AFTER
const handleDelete = () => {
```

**Verification**: No behavior change; function signature now matches actual usage.

---

### ISSUE-08: Redundant ternary — `isSaving ? editValue : editValue`

**Severity**: LOW
**Category**: Code Smell
**File**: `frontend/src/features/cards/card.tsx`
**Location**: ~line 107

**Problem**: Both branches of the ternary return `editValue`. Dead logic.

**Fix**: Replace with just `editValue`, or implement the intended differentiation (e.g., show a saving indicator).

```tsx
// BEFORE
isSaving ? editValue : editValue

// AFTER (simple)
editValue

// AFTER (if saving state display was intended)
isSaving ? editValue + "..." : editValue
```

---

### ISSUE-09: No loading/disabled state on delete confirm button

**Severity**: MEDIUM
**Category**: UX
**File**: `frontend/src/features/cards/card.tsx`
**Location**: AlertDialog confirm button, ~line 218-226

**Problem**: No loading indicator or disabled state during delete mutation. User can double-click and fire duplicate requests.

**Fix**: Track deleting state and disable the button:

```tsx
const { mutate: deleteCard, isPending: isDeleting } = useDeleteCard();

// In AlertDialogAction:
<AlertDialogAction
  onClick={handleDelete}
  disabled={isDeleting}
  className={isDeleting ? "opacity-50 cursor-not-allowed" : ""}
>
  {isDeleting ? "Deleting..." : "Delete"}
</AlertDialogAction>
```

**Verification**: Click delete → confirm button shows loading state and is disabled.

---

### ISSUE-10: Toast auto-dismiss may make Undo unreachable

**Severity**: MEDIUM
**Category**: UX
**File**: `frontend/src/features/cards/card.tsx`
**Location**: Toast duration in `handleDelete`, and `use-toast.tsx`

**Problem**: The destructive toast with Undo action auto-dismisses after 5 seconds. For a destructive action with undo capability, the toast should persist until manually dismissed.

**Fix**: Set the toast duration to a very long value or Infinity:

```tsx
toast({
  title: "Card deleted",
  // ... other props
  duration: 30000, // 30 seconds, or use Infinity for manual-dismiss only
});
```

**Note**: Check if the toast component supports `Infinity` as duration. If not, use a very long timeout (e.g., 30000ms).

**Verification**: Delete a card → toast should remain visible for at least 30 seconds or until manually dismissed.

---

### ISSUE-11: `queryKey[1] as number` — fragile hidden contract

**Severity**: MEDIUM
**Category**: Code Smell
**File**: `frontend/src/features/cards/use-cards.ts`
**Location**: ~line 8

**Problem**: Casting `queryKey[1]` assumes a specific query key structure. If the key format changes, this silently produces wrong behavior with no type error.

**Fix**: Use a typed query key factory:

```ts
const cardKeys = {
  byColumn: (columnId: number) => ['cards', columnId] as const,
  all: () => ['cards'] as const,
};

// Usage:
// queryClient.invalidateQueries({ queryKey: cardKeys.byColumn(columnId) })
// queryKey[1] → destructured properly from the factory
```

**Verification**: Type-safe query key usage across `use-cards.ts`.

---

### ISSUE-12: `onSettled` invalidation may cause visual flicker after undo

**Severity**: LOW
**Category**: Edge Case
**File**: `frontend/src/features/cards/use-cards.ts`
**Location**: ~line 181-184

**Problem**: After undo (which calls `createCardMutation`), the delete mutation's `onSettled` fires and invalidates queries, potentially causing a brief flicker where the re-created card disappears and reappears.

**Fix**: This is low severity since `onSettled` eventually converges to correct state. If flicker is observed, conditionally skip `onSettled` invalidation when an undo is in progress (e.g., set a flag in the undo handler).

**Verification**: Monitor for flicker after undo — only fix if observed.

---

### ISSUE-13: Missing test coverage — undo flow, error paths, optimistic rollback

**Severity**: HIGH
**Category**: Missing Test Coverage
**File**: `frontend/src/features/cards/card.test.tsx`

**Problem**: Zero tests for:
1. Undo toast action (clicking "Undo" calls `createCardMutation.mutateAsync` with correct data)
2. Delete error handling (`onError` callback)
3. Optimistic UI rollback in `useDeleteCard`
4. `onSettled` query invalidation

**Fix**: Add the following test cases:

```tsx
describe('Card deletion undo', () => {
  it('clicking Undo in toast calls createCardMutation with card data', async () => {
    // Setup: render card, trigger delete, confirm delete
    // Capture toast call arguments
    // Simulate clicking Undo action
    // Assert: createCardMutation.mutateAsync called with { title, column_id, position }
  });

  it('shows error toast when delete fails', async () => {
    // Setup: make mockDeleteMutate call onError
    // Trigger delete flow
    // Assert: error toast called with "Failed to delete card"
  });

  it('shows error toast when undo fails', async () => {
    // Setup: make mockDeleteMutate call onSuccess, mockCreateMutateAsync reject
    // Trigger delete, then undo
    // Assert: error toast called with "Failed to restore card"
  });
});
```

**Verification**: All new tests pass and provide meaningful assertions.

---

## Deferred Items

These items were identified but deferred for later consideration:

| # | Item | Reason |
|---|------|--------|
| D-01 | Delete dropdown menu has only one item ("Delete") | Acceptable if more menu items are planned in future stories |
| D-02 | SEC: No CSRF/confirmation token on delete | Acceptable for kanban app with proper auth; rate limiting is a future concern |
| D-03 | Undo creates card with new ID (vs restoring original) | Known limitation of current architecture; a proper restore API endpoint would be ideal but is out of scope for this story |
| D-04 | Delete while editing silently discards unsaved title | Behavior is arguably correct (saves snapshot, not draft); could add a warning in a future enhancement |
| D-05 | Toast undo closure may have stale mutation reference if component unmounts | Low-risk edge case; depends on toast rendering architecture |

---

## Summary

| Severity | Count | Key Issues |
|----------|-------|------------|
| CRITICAL | 1 | Missing `group` class (ISSUE-01) |
| HIGH | 4 | Contradictory dialog text (ISSUE-02), Position not restored (ISSUE-03), Broken test (ISSUE-04), Missing test coverage (ISSUE-13) |
| MEDIUM | 5 | Toast mock cleanup (ISSUE-05/06), async handleDelete (ISSUE-07), No loading state (ISSUE-09), Toast auto-dismiss (ISSUE-10), Fragile queryKey (ISSUE-11) |
| LOW | 3 | Redundant ternary (ISSUE-08), Flicker edge case (ISSUE-12), Deferred items |

**Recommended fix order**: ISSUE-01 → ISSUE-04 → ISSUE-03 → ISSUE-02 → ISSUE-13 → ISSUE-05/06 → ISSUE-07 → ISSUE-08 → ISSUE-09 → ISSUE-10 → ISSUE-11 → ISSUE-12