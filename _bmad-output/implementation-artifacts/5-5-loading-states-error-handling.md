# Story 5.5: Loading States & Error Handling

Status: done

## Quick Reference

**Components to Create:** `Spinner`, `ErrorState`, `RouteErrorBoundary`, `BoardListSkeleton`, `CardListSkeleton`, `FormSkeleton`, `error-messages.ts`

**Files to Modify:** `loading-skeleton.tsx`, `lazy-load-boundary.tsx`, `App.tsx`, `app-layout.tsx`, `board-list.tsx`, `project-list.tsx`, `archived-boards.tsx`, `toast-provider/index.tsx`, `use-toast/context.ts`

**Key Decisions:**
- Use `useToastHelpers` for simple toasts, `useToast()` for custom actions
- RouteErrorBoundary wraps INSIDE existing `<Suspense>` — do NOT replace Suspense
- Skeleton presets are separate files, exported from `loading-skeleton.tsx`

**DO NOT:**
- Create a new toast system — use existing ToastProvider
- Replace React Query error handling with RouteErrorBoundary
- Add loading spinners to drag-drop operations (use optimistic updates)

## Story

As a user,
I want clear feedback when the app is loading or encounters errors,
So that I always know what's happening.

## Acceptance Criteria

1. **Given** a page is loading
   **When** data is being fetched
   **Then** skeleton screens appear matching the expected layout
   **And** skeletons use gray shapes that approximate the content

2. **Given** I submit a form
   **When** the request is processing
   **Then** a spinner appears on the submit button
   **And** the button text changes to "Saving..."

3. **Given** an operation fails
   **When** an error occurs
   **Then** a red toast appears with a friendly error message
   **And** the toast requires manual dismiss
   **And** the message is actionable (e.g., "Failed to save. Retry?")

4. **Given** I perform a drag-drop
   **When** the move is in progress
   **Then** no loading indicator appears (optimistic update)
   **And** if the server request fails, a toast notifies me

## Implementation Order

**Dependencies must be respected:**
1. Create `Spinner` component (no dependencies)
2. Create `ErrorState` component (no dependencies)
3. Enhance `LoadingSkeleton` (no dependencies)
4. Create `RouteErrorBoundary` (depends on #1)
5. Enhance toast animations (no dependencies)
6. Refactor inline spinners (depends on #1)
7. Refactor inline error states (depends on #2)
8. Adopt `useToastHelpers` (depends on #5)
9. Add tests (depends on #1-#8)

## Error Message Templates

**Use these templates for consistent error messaging across the app:**

```typescript
// lib/error-messages.ts
type ErrorType = 'network' | 'timeout' | 'server' | 'validation' | 'unauthorized' | 'unknown';

interface ErrorMessage {
  title: string;
  message: string;
  retry: boolean;
  redirect?: string;
}

export const ERROR_MESSAGES: Record<ErrorType, ErrorMessage> = {
  network: {
    title: 'Connection Error',
    message: 'Unable to reach the server. Check your internet connection.',
    retry: true,
  },
  timeout: {
    title: 'Request Timeout',
    message: 'The request took too long. Please try again.',
    retry: true,
  },
  server: {
    title: 'Server Error',
    message: 'Something went wrong on our end. Please try again later.',
    retry: true,
  },
  validation: {
    title: 'Invalid Input',
    message: 'Please check your input and try again.',
    retry: false,
  },
  unauthorized: {
    title: 'Session Expired',
    message: 'Your session has expired. Please log in again.',
    retry: false,
    redirect: '/login',
  },
  unknown: {
    title: 'Something Went Wrong',
    message: 'An unexpected error occurred. Please try again.',
    retry: true,
  },
};

export function getErrorMessage(error: unknown): ErrorMessage {
  if (error instanceof TypeError && error.message.includes('Failed to fetch')) {
    return ERROR_MESSAGES.network;
  }
  if (error instanceof Error && error.name === 'AbortError') {
    return ERROR_MESSAGES.timeout;
  }
  // For Axios errors: error.response?.status
  // For fetch errors: check error instanceof Response
  if (error instanceof Response) {
    if (error.status === 401) return ERROR_MESSAGES.unauthorized;
    if (error.status >= 500) return ERROR_MESSAGES.server;
    if (error.status >= 400) return ERROR_MESSAGES.validation;
  }
  // For API errors with status: error.response?.status || error.status
  const status = (error as any)?.response?.status || (error as any)?.status;
  if (status === 401) return ERROR_MESSAGES.unauthorized;
  if (status >= 500) return ERROR_MESSAGES.server;
  if (status >= 400) return ERROR_MESSAGES.validation;
  return ERROR_MESSAGES.unknown;
}
```

## Loading State Decision Tree

**Use this to determine the correct loading indicator:**

```
Loading State Decision Tree:
├── Initial page load → Skeleton screen (matches expected layout)
├── Data refetch (existing data visible) → Background refresh (no skeleton)
├── Form submission → Button spinner + "Saving..." text
├── Single action (delete, archive) → Button spinner
├── Drag-drop → No indicator (optimistic update)
└── Navigation between routes → Suspense fallback (existing LoadingFallback)
```

## Error Boundary Reset Mechanism

**RouteErrorBoundary must support multiple reset triggers:**

```typescript
// components/route-error-boundary.tsx
interface RouteErrorBoundaryProps {
  children: React.ReactNode;
  fallback?: React.ComponentType<{ error: Error; reset: () => void }>;
  onReset?: () => void;
  resetKeys?: unknown[]; // Auto-reset when these values change
}

// Reset triggers:
// 1. User clicks "Try Again" button → calls reset()
// 2. Navigation to different route → resetKeys change (use useLocation().pathname)
// 3. Manual reset via onReset callback

// Implementation for resetKeys (auto-reset on navigation):
componentDidUpdate(prevProps: RouteErrorBoundaryProps) {
  if (this.state.hasError && prevProps.resetKeys !== this.props.resetKeys) {
    // Check if any resetKey changed
    const hasChanged = prevProps.resetKeys?.some(
      (key, i) => key !== this.props.resetKeys?.[i]
    );
    if (hasChanged) {
      this.setState({ hasError: false, error: null });
    }
  }
}

// Usage in App.tsx:
function App() {
  const location = useLocation();
  return (
    <RouteErrorBoundary resetKeys={[location.pathname]}>
      {/* routes */}
    </RouteErrorBoundary>
  );
}
```

### Error Boundary Logging Strategy

**For this story, implement `componentDidCatch` with `console.error` for development:**

```typescript
componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
  // Development logging
  console.error('RouteErrorBoundary caught:', error, errorInfo);

  // Error data structure for future production integration
  const errorData = {
    message: error.message,
    stack: error.stack,
    componentStack: errorInfo.componentStack,
    timestamp: new Date().toISOString(),
  };

  // TODO: Integrate with error reporting service (Sentry, LogRocket, etc.)
  // errorReportingService.captureException(error, errorData);
}
```

### Error Boundary Reset UX Specification

**Fallback UI behavior:**
1. Show error title, friendly message, and "Try Again" button
2. After clicking "Try Again", show brief loading state (spinner)
3. If error persists after 3 retries, show "Contact support" message
4. Never show raw error details to users (only in development mode)

```tsx
// Fallback UI component
function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  const [retryCount, setRetryCount] = useState(0);
  const maxRetries = 3;

  const handleRetry = () => {
    setRetryCount(prev => prev + 1);
    resetErrorBoundary();
  };

  return (
    <div className="rounded-lg border border-destructive bg-destructive/10 p-6 text-center">
      <h3 className="text-lg font-semibold text-destructive">Something went wrong</h3>
      <p className="mt-2 text-sm text-muted-foreground">
        An unexpected error occurred. Please try again.
      </p>
      {retryCount < maxRetries ? (
        <Button variant="outline" className="mt-4" onClick={handleRetry}>
          Try Again
        </Button>
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">
          If this persists, please contact support.
        </p>
      )}
    </div>
  );
}
```

## Tasks / Subtasks

- [x] Create `Spinner` component (AC: #2) — Task #1
  - [x] Centralized loading spinner with size variants
  - [x] Props: `size?: 'sm' | 'md' | 'lg'`, `className?`
  - [x] Consistent styling across app
  - [x] Use `animate-spin` with border styling
  - [x] Add `role="status"` and `aria-label="Loading"` for accessibility

- [x] Create `ErrorState` component (AC: #3) — Task #2
  - [x] Reusable error display component (like `EmptyState`)
  - [x] Props: `title`, `message`, `onRetry?`, `retryLabel?`
  - [x] Red border with destructive styling
  - [x] Optional retry button
  - [x] Accessible with `role="alert"` and `aria-live="assertive"`
  - [x] Use error message templates (see Error Message Templates section)

- [x] Create skeleton preset components (AC: #1) — Task #3
  - [x] Create `board-list-skeleton.tsx` — grid of card-shaped skeletons
  - [x] Create `card-list-skeleton.tsx` — list of shorter card skeletons
  - [x] Create `form-skeleton.tsx` — input field skeletons with button
  - [x] Export all presets from `loading-skeleton.tsx` as named exports
  - [x] Ensure `aria-busy="true"` and `aria-label` for accessibility

- [x] Create `RouteErrorBoundary` component — Task #4
  - [x] Top-level error boundary for lazy routes
  - [x] Add `componentDidCatch` for error logging (see Error Boundary Logging Strategy)
  - [x] Fallback UI with retry mechanism (see Error Boundary Reset UX)
  - [x] Wrap each route group INSIDE existing `<Suspense>` in `App.tsx`
  - [x] Implement reset mechanism (see Error Boundary Reset Mechanism section)

**Error Boundary vs Suspense Integration:**
```tsx
// App.tsx - Add RouteErrorBoundary INSIDE existing Suspense
// Target structure after this story:
<Suspense fallback={<LoadingFallback />}>
  <RouteErrorBoundary>
    <Routes>
      <Route path="/register" element={<RegisterForm />} />
      <Route path="/login" element={<LoginForm />} />
      <Route path="/forgot-password" element={<ForgotPasswordPage />} />

      <Route element={<AppLayoutRoute />}>
        <Route path="/" element={<BoardList />} />
        <Route path="/archived-boards" element={<ArchivedBoards />} />
        <Route path="/projects" element={<ProjectList />} />
        <Route path="/notes" element={<NotesPage />} />
        <Route path="/notes/:id" element={<NoteDetailPage />} />
        <Route path="/notes/:id/edit" element={<NoteEditPage />} />
        <Route path="/board/:boardId" element={<BoardView />} />
      </Route>

      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  </RouteErrorBoundary>
</Suspense>

// Suspense handles: loading fallback (initial load, lazy chunks)
// RouteErrorBoundary handles: error fallback (render errors, failed loads)
// KEEP existing <Suspense fallback={<LoadingFallback />}> - do NOT replace
// RouteErrorBoundary wraps INSIDE Suspense, NOT outside
```

- [x] Enhance toast system (AC: #3) — Task #5
  - [x] Add CSS transition animations for enter/exit
  - [x] Add `slide-in` animation from right
  - [x] Add `fade-out` animation on dismiss
  - [x] Ensure `prefers-reduced-motion` disables animations

- [x] Refactor inline spinners (AC: #2) — Task #6
  - [x] Replace inline spinner in `app-layout.tsx` with `Spinner`
  - [x] Replace `LoadingFallback` in `App.tsx` with `Spinner`
  - [x] Standardize spinner sizes

- [x] Refactor inline error states (AC: #3) — Task #7
  - [x] Replace copy-pasted error UI in `board-list.tsx`
  - [x] Replace copy-pasted error UI in `project-list.tsx`
  - [x] Replace copy-pasted error UI in `archived-boards.tsx`
  - [x] Use new `ErrorState` component

- [x] Adopt `useToastHelpers` (AC: #3) — Task #8
  - [x] Replace direct `useToast()` calls with `useToastHelpers` (see Migration Plan below)
  - [x] Use `showSuccess()`, `showError()`, `showDestructive()` helpers
  - [x] Keep `useToast()` for custom action patterns (see Decision Matrix)
  - [x] Remove unused direct `useToast()` imports where possible

**useToastHelpers Migration Plan:**

| File | Current Pattern | Action |
|------|-----------------|--------|
| `board-list.tsx` | `useToast()` with custom Undo action | KEEP `useToast()` for custom action |
| `project-list.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `archived-boards.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `card.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `card-detail-panel.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `column-header.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `drag-drop-context.tsx` | `useToast()` for error with retry | KEEP `useToast()` for complex patterns |
| `add-card-input.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `tag-picker.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `login-form.tsx` | `useToast()` for error | MIGRATE to `useToastHelpers` |
| `register-form.tsx` | `useToast()` for error | MIGRATE to `useToastHelpers` |
| `board-notes-sidebar.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `note-detail-page.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `note-list.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `note-editor.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `board-list-view.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `board-view.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `create-board-modal.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `board-card.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `checklist-item.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `add-checklist-form.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `add-checklist-item-form.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `checklist.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |
| `label-picker.tsx` | `useToast()` for success/error | MIGRATE to `useToastHelpers` |

- [x] Add tests for new components — Task #9
  - [x] Test `ErrorState` rendering and retry
  - [x] Test `Spinner` size variants
  - [x] Test `LoadingSkeleton` accessibility
  - [x] Test `RouteErrorBoundary` error catching and reset

## Dev Notes

### Architecture Compliance

Follow `architecture.md` patterns for stack (React 19 + Tailwind CSS v4 + shadcn/ui), state management (React Query), error handling (React Query `onError` → toast), loading patterns (skeletons, spinners, optimistic updates), and accessibility (`role="alert"`, `aria-live`, `aria-busy`, `prefers-reduced-motion`).

### Key Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `frontend/src/components/error-state.tsx` | CREATE | Reusable error display component |
| `frontend/src/components/error-state.test.tsx` | CREATE | Tests for ErrorState |
| `frontend/src/components/spinner.tsx` | CREATE | Centralized loading spinner |
| `frontend/src/components/spinner.test.tsx` | CREATE | Tests for Spinner |
| `frontend/src/components/route-error-boundary.tsx` | CREATE | Top-level error boundary |
| `frontend/src/components/route-error-boundary.test.tsx` | CREATE | Tests for RouteErrorBoundary |
| `frontend/src/components/loading-skeleton.tsx` | MODIFY | Add skeleton presets |
| `frontend/src/components/loading-skeleton.test.tsx` | CREATE | Tests for skeleton presets |
| `frontend/src/components/lazy-load-boundary.tsx` | MODIFY | Add error logging |
| `frontend/src/App.tsx` | MODIFY | Wrap routes in RouteErrorBoundary |
| `frontend/src/features/boards/board-list.tsx` | MODIFY | Use ErrorState component |
| `frontend/src/features/projects/project-list.tsx` | MODIFY | Use ErrorState component |
| `frontend/src/features/boards/archived-boards.tsx` | MODIFY | Use ErrorState component |
| `frontend/src/layouts/app-layout.tsx` | MODIFY | Use Spinner component |
| `frontend/src/components/ui/toast-provider/index.tsx` | MODIFY | Add animations |
| `frontend/src/components/ui/toast-provider/toast-provider.css` | CREATE | Toast animation styles |
| `frontend/src/components/ui/use-toast/context.ts` | MODIFY | Add `isExiting?: boolean` to Toast interface |
| `frontend/src/lib/toast-helpers.ts` | VERIFY | Already exists, adopt usage |
| `frontend/src/lib/error-messages.ts` | CREATE | Error message templates |
| `frontend/src/lib/error-messages.test.ts` | CREATE | Tests for error messages |
| `frontend/src/components/toast-provider.test.tsx` | CREATE | Tests for toast animations |
| `frontend/src/components/board-list-skeleton.tsx` | CREATE | Board list skeleton preset |
| `frontend/src/components/card-list-skeleton.tsx` | CREATE | Card list skeleton preset |
| `frontend/src/components/form-skeleton.tsx` | CREATE | Form skeleton preset |

### Existing Components Analysis

- **LoadingSkeleton:** Renders N identical rows, props: `count`, `className`, `ariaLabel`
- **Skeleton:** Atomic placeholder `animate-pulse rounded-md bg-primary/10`
- **EmptyState:** Props: `icon`, `headline`, `description?`, `action?` — pattern for ErrorState
- **ToastProvider:** `useState<Toast[]>`, auto-dismiss 3s/5s — add CSS animations
- **LazyLoadBoundary:** Class component with `getDerivedStateFromError()` — add logging

### Skeleton Preset Specifications

**BoardListSkeleton:** Grid of card-shaped skeletons matching board card grid layout
```tsx
function BoardListSkeleton({ count = 6 }: { count?: number }) {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="h-32 animate-pulse rounded-lg bg-primary/10" />
      ))}
    </div>
  );
}
```

**CardListSkeleton:** Horizontal list of shorter card skeletons matching column card layout
```tsx
function CardListSkeleton({ count = 4 }: { count?: number }) {
  return (
    <div className="space-y-2">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="h-16 animate-pulse rounded-lg bg-primary/10" />
      ))}
    </div>
  );
}
```

**FormSkeleton:** Input field skeletons with button skeleton
```tsx
function FormSkeleton({ fields = 3 }: { fields?: number }) {
  return (
    <div className="space-y-4">
      {Array.from({ length: fields }).map((_, i) => (
        <div key={i} className="space-y-2">
          <div className="h-4 w-24 animate-pulse rounded bg-primary/10" />
          <div className="h-10 w-full animate-pulse rounded-md bg-primary/10" />
        </div>
      ))}
      <div className="h-10 w-24 animate-pulse rounded-md bg-primary/10" />
    </div>
  );
}
```

### Toast Animation Integration Details

**Modified ToastProvider with animation classes:**
```tsx
// components/ui/toast-provider/index.tsx
export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const toast = useCallback((toast: Omit<Toast, 'id'>) => {
    const id = Math.random().toString(36).slice(2);
    setToasts(prev => [...prev, { ...toast, id, isExiting: false }]);
  }, []);

  const dismiss = useCallback((id: string) => {
    // Add exit class before removing
    setToasts(prev =>
      prev.map(t => (t.id === id ? { ...t, isExiting: true } : t))
    );
    // Remove after animation completes (200ms matches CSS fadeOut animation)
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 200);
  }, []);

  return (
    <ToastContext.Provider value={{ toasts, toast, dismiss }}>
      {children}
      <div className="fixed bottom-4 right-4 z-50 space-y-2">
        {toasts.map(toast => (
          <div
            key={toast.id}
            className={`toast-enter ${toast.isExiting ? 'toast-exit' : ''}`}
            // ... rest of toast rendering
          />
        ))}
      </div>
    </ToastContext.Provider>
  );
}
```

### usePrefersReducedMotion Integration

**CSS media query is sufficient for this story.** The `@media (prefers-reduced-motion: reduce)` in the toast CSS handles animation disabling. The `usePrefersReducedMotion` hook from Story 5-4 is available if React-level control is needed, but CSS-only approach is simpler and performant.

### ErrorState Import Guidance

**Named export with standard import path:**
```tsx
// Import pattern
import { ErrorState } from '@/components/error-state';
import { Spinner } from '@/components/spinner';
import { RouteErrorBoundary } from '@/components/route-error-boundary';

// No barrel export needed - direct imports preferred
```

### Missing Test File Paths

**Additional tests to create:**
| Test File | Purpose |
|-----------|---------|
| `lib/error-messages.test.ts` | Test `getErrorMessage()` function |
| `components/toast-provider.test.tsx` | Test toast animations (snapshot) |
| `components/board-list-skeleton.test.tsx` | Test BoardListSkeleton preset |
| `components/card-list-skeleton.test.tsx` | Test CardListSkeleton preset |
| `components/form-skeleton.test.tsx` | Test FormSkeleton preset |

### Error Classification

Uses the `getErrorMessage()` function from `lib/error-messages.ts` (see Error Message Templates section above).

### Error Boundary vs React Query Error Handling

**Important distinction:**
- **RouteErrorBoundary** catches JavaScript **render errors** only (component throws during render)
- **React Query `onError`** catches **API/data errors** (failed fetch, 4xx/5xx responses)
- **Do NOT** use RouteErrorBoundary for API error handling
- **Do NOT** use React Query onError for render errors

```tsx
// React Query handles API errors → show toast
const { data, isError, error } = useQuery({
  queryKey: ['boards'],
  queryFn: fetchBoards,
  onError: (err) => {
    toast({ title: 'Failed to load boards', type: 'error' });
  },
});

// RouteErrorBoundary handles render errors → show fallback UI
<RouteErrorBoundary fallback={ErrorFallback}>
  <BoardList />
</RouteErrorBoundary>
```

### Error Boundary Placement Strategy

**Conceptual architecture (this story implements single top-level boundary):**
```
App
└── Suspense (existing LoadingFallback)
    └── RouteErrorBoundary (THIS STORY)
        ├── / → BoardList (ErrorState on React Query error)
        ├── /projects/:projectId → ProjectDetail
        ├── /projects/:projectId/boards/:boardId → BoardView
        │   ├── Column (Suspense for lazy cards)
        │   └── CardDetailPanel (LazyLoadBoundary for markdown)
        └── /notes → NoteList
```

**Note:** Per-route boundaries can be added in future stories for finer-grained error isolation.

### Toast Animation CSS

```css
@keyframes slideInRight {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes fadeOut {
  from {
    opacity: 1;
  }
  to {
    opacity: 0;
  }
}

.toast-enter {
  animation: slideInRight 0.3s ease-out;
}

.toast-exit {
  animation: fadeOut 0.2s ease-in forwards;
}

@media (prefers-reduced-motion: reduce) {
  .toast-enter,
  .toast-exit {
    animation: none;
  }
}
```

### Spinner Component API

```tsx
interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const sizeMap = {
  sm: 'h-4 w-4 border-2',
  md: 'h-6 w-6 border-2',
  lg: 'h-8 w-8 border-4',
};

export function Spinner({ size = 'md', className }: SpinnerProps) {
  return (
    <div
      className={`animate-spin rounded-full border-primary border-t-transparent ${sizeMap[size]} ${className}`}
      role="status"
      aria-label="Loading"
    >
      <span className="sr-only">Loading...</span>
    </div>
  );
}
```

### ErrorState Component API

```tsx
interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
}

export function ErrorState({
  title = 'Something went wrong',
  message,
  onRetry,
  retryLabel = 'Retry',
}: ErrorStateProps) {
  return (
    <div
      className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive"
      role="alert"
      aria-live="assertive"
    >
      <p className="text-sm font-medium">{title}</p>
      <p className="text-sm opacity-80">{message}</p>
      {onRetry && (
        <Button
          variant="outline"
          size="sm"
          className="mt-3"
          onClick={onRetry}
        >
          {retryLabel}
        </Button>
      )}
    </div>
  );
}
```

### Refactoring Examples

**BEFORE: board-list.tsx lines 43-57 (inline error state with page wrapper)**
```tsx
if (isError) {
  return (
    <div className="mx-auto max-w-6xl">
      <h1 className="mb-6 text-2xl font-bold">My Boards</h1>
      <div className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive">
        <p className="text-sm font-medium">Failed to load boards</p>
        <p className="text-sm opacity-80">
          {error instanceof Error ? error.message : 'Something went wrong'}
        </p>
        <Button variant="outline" size="sm" className="mt-3" onClick={() => refetch()}>
          Retry
        </Button>
      </div>
    </div>
  );
}
```

**AFTER: board-list.tsx (using ErrorState, keep page wrapper)**
```tsx
if (isError) {
  return (
    <div className="mx-auto max-w-6xl">
      <h1 className="mb-6 text-2xl font-bold">My Boards</h1>
      <ErrorState
        title="Failed to load boards"
        message={error instanceof Error ? error.message : 'Something went wrong'}
        onRetry={() => refetch()}
      />
    </div>
  );
}
```

**BEFORE: archived-boards.tsx lines 82-95 (inline error state with page wrapper)**
```tsx
if (isError) {
  return (
    <div className="mx-auto max-w-6xl">
      <h1 className="mb-6 text-2xl font-bold">Archived Boards</h1>
      <div className="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive">
        <p className="text-sm font-medium">Failed to load archived boards</p>
        <p className="text-sm opacity-80">{error instanceof Error ? error.message : 'Something went wrong'}</p>
        <Button variant="outline" size="sm" className="mt-3" onClick={() => refetch()}>
          Retry
        </Button>
      </div>
    </div>
  );
}
```

**AFTER: archived-boards.tsx (using ErrorState, keep page wrapper)**
```tsx
if (isError) {
  return (
    <div className="mx-auto max-w-6xl">
      <h1 className="mb-6 text-2xl font-bold">Archived Boards</h1>
      <ErrorState
        title="Failed to load archived boards"
        message={error instanceof Error ? error.message : 'Something went wrong'}
        onRetry={() => refetch()}
      />
    </div>
  );
}
```

**BEFORE: app-layout.tsx lines 82-88 (inline spinner)**
```tsx
if (isLoading) {
  return (
    <div className="flex h-dvh items-center justify-center bg-background">
      <div className="h-6 w-6 animate-spin rounded-full border-2 border-border border-t-primary" />
    </div>
  );
}
```

**AFTER: app-layout.tsx (using Spinner)**
```tsx
if (isLoading) {
  return (
    <div className="flex h-dvh items-center justify-center bg-background">
      <Spinner size="md" />
    </div>
  );
}
```

### useToastHelpers vs useToast Decision Matrix

| Pattern | Use When | Example |
|---------|----------|---------|
| `useToastHelpers.showError()` | Simple error toast | API failure, validation error |
| `useToastHelpers.showSuccess()` | Simple success toast | CRUD success, save complete |
| `useToastHelpers.showDestructive()` | Destructive action with Undo | Delete card, archive board |
| `useToast()` direct | Custom action, complex toast | Board archived with custom Undo logic, toast with multiple actions |

**Rule:** If the toast only needs title + message + optional undo, use `useToastHelpers`. If it needs custom action callbacks or complex UI, use `useToast()` directly.

### Anti-Pattern Warnings

**DO NOT:**
- Create a new toast system — use existing ToastProvider and useToastHelpers
- Replace React Query error handling with RouteErrorBoundary — they serve different purposes
- Add loading spinners to drag-drop operations — use optimistic updates instead
- Remove existing `<Suspense>` from App.tsx — RouteErrorBoundary wraps INSIDE it
- Show raw error details to users — only log to console in development
- Create skeleton components without `aria-busy="true"` and `aria-label`
- Add animations without `prefers-reduced-motion` support
- Use `useToast()` for simple success/error toasts — use `useToastHelpers` instead

### Cross-Story Dependencies

**Dependencies (stories this builds upon):**
- Story 1.8 (empty-state-toast-system): EmptyState component pattern to follow
- Story 5.4 (responsive-layout): usePrefersReducedMotion hook for animations
- Story 5.3 (dark-mode): CSS variables for theming

**Available from previous work:**
| Component | File | Purpose |
|-----------|------|---------|
| EmptyState | `components/empty-state.tsx` | Pattern to follow for ErrorState |
| usePrefersReducedMotion | `hooks/use-prefers-reduced-motion.ts` | Animation accessibility |
| Toast system | `components/ui/toast-provider/` | Toast notifications |
| useToastHelpers | `lib/toast-helpers.ts` | Convenience wrappers |
| LoadingSkeleton | `components/loading-skeleton.tsx` | Existing skeleton component |
| Skeleton | `components/ui/skeleton.tsx` | Atomic skeleton primitive |
| LazyLoadBoundary | `components/lazy-load-boundary.tsx` | Existing error boundary |

### Previous Story Learnings (5-4-responsive-layout)

- Created `useBreakpoint` hook with matchMedia API
- Created `usePrefersReducedMotion` hook - extend for toast animations
- CSS variables work well for theming
- Tailwind v4 responsive prefixes work as expected
- shadcn/ui components have good responsive support
- All 607 tests passing - maintain test coverage

### Testing Strategy

- **Unit tests:** Co-located `.test.tsx` files
- **Accessibility tests:** `role="alert"`, `aria-live`, `aria-busy`
- **Animation tests:** Mock `prefers-reduced-motion`
- **Error boundary tests:** Simulate render errors
- **Integration tests:** Toast notifications on API errors

### Accessibility Requirements

- `role="alert"` on error states
- `aria-live="assertive"` for error announcements
- `aria-busy="true"` on loading skeletons
- `aria-label` on spinners
- `prefers-reduced-motion` disables animations
- Keyboard navigation for retry buttons
- **Focus management after error recovery:** After clicking "Try Again" and error clears, move focus to the first interactive element in the recovered content

### Test Coverage by Acceptance Criteria

```
Test Coverage by AC:
├── AC #1 (Skeleton screens) → loading-skeleton.test.tsx
│   ├── Test BoardListSkeleton preset
│   ├── Test CardListSkeleton preset
│   ├── Test FormSkeleton preset
│   └── Test aria-busy="true" and aria-label
├── AC #2 (Button spinner) → spinner.test.tsx, form integration tests
│   ├── Test size variants (sm, md, lg)
│   ├── Test aria-label="Loading"
│   └── Test button text changes to "Saving..."
├── AC #3 (Error toast) → toast-provider.test.tsx, error-state.test.tsx
│   ├── Test ErrorState renders title and message
│   ├── Test ErrorState retry button calls onRetry
│   ├── Test toast animations (slide-in, fade-out)
│   ├── Test prefers-reduced-motion disables animations
│   └── Test error message templates
└── AC #4 (Optimistic update) → drag-drop-context.test.tsx (existing)
    └── Verify no loading indicator during drag-drop
```

### Performance Budget

| Metric | Target | How to Verify |
|--------|--------|---------------|
| Skeleton screen render | < 16ms (60fps) | React DevTools Profiler |
| Toast animation | 300ms enter, 200ms exit | Browser DevTools Performance |
| Error boundary fallback render | < 50ms | React DevTools Profiler |
| Spinner component render | < 8ms | React DevTools Profiler |

**Optimization notes:**
- CSS animations preferred over JS for performance
- Avoid heavy computation in skeleton components
- Debounce rapid retry attempts (300ms minimum)
- Skeleton screens reduce perceived loading time
- Optimistic updates for drag-drop (no spinner)

## References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.5]
- [Source: _bmad-output/planning-artifacts/architecture.md#Loading States]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Emotional Design]
- [Source: frontend/src/components/loading-skeleton.tsx]
- [Source: frontend/src/components/empty-state.tsx]
- [Source: frontend/src/components/lazy-load-boundary.tsx]
- [Source: frontend/src/components/ui/toast-provider/index.tsx]
- [Source: frontend/src/lib/toast-helpers.ts]
- [Source: frontend/src/features/boards/board-list.tsx:43-57]
- [Source: frontend/src/features/projects/project-list.tsx:293-306]
- [Source: frontend/src/features/boards/archived-boards.tsx:82-95]
- [Source: MDN Loading States Best Practices 2026]
- [Source: React Error Boundaries Complete Guide 2026]
- [Source: Discriminated Unions TypeScript Pattern]

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List

**Created:**
- `frontend/src/components/spinner.tsx`
- `frontend/src/components/spinner.test.tsx`
- `frontend/src/components/error-state.tsx`
- `frontend/src/components/error-state.test.tsx`
- `frontend/src/components/route-error-boundary.tsx`
- `frontend/src/components/route-error-boundary.test.tsx`
- `frontend/src/components/board-list-skeleton.tsx`
- `frontend/src/components/board-list-skeleton.test.tsx`
- `frontend/src/components/card-list-skeleton.tsx`
- `frontend/src/components/card-list-skeleton.test.tsx`
- `frontend/src/components/form-skeleton.tsx`
- `frontend/src/components/form-skeleton.test.tsx`
- `frontend/src/lib/error-messages.ts`
- `frontend/src/lib/error-messages.test.ts`
- `frontend/src/components/ui/toast-provider/toast-provider.css`
- `frontend/src/components/ui/toast-provider/toast-provider.test.tsx`

**Modified:**
- `frontend/src/components/loading-skeleton.tsx`
- `frontend/src/App.tsx`
- `frontend/src/layouts/app-layout.tsx`
- `frontend/src/features/boards/board-list.tsx`
- `frontend/src/features/projects/project-list.tsx`
- `frontend/src/features/boards/archived-boards.tsx`
- `frontend/src/components/ui/toast-provider/index.tsx`
- `frontend/src/components/ui/use-toast/context.ts`
- `frontend/src/components/ui/use-toast.test.tsx`
- 22 files migrated to useToastHelpers

