# Story 1.6: App Shell & Navigation

Status: done

## Story

As a user,
I want a consistent app layout with navigation,
so that I can easily move between projects and boards.

## Acceptance Criteria

1. **Given** I am logged in, **When** I view the application, **Then** I see a header with the app name "KanbanFlow" and user menu
2. **And** I see a collapsible sidebar (240px expanded, 0px collapsed) with smooth 300ms transition
3. **And** the sidebar is collapsed by default
4. **And** the sidebar shows my projects list
5. **And** clicking a project navigates to it
6. **And** breadcrumbs show current location (Project > Board)
7. **And** React Router handles client-side routing with nested routes
8. **And** layout renders correctly in both light and dark mode
9. **And** sidebar toggle button is accessible via keyboard (Tab + Enter/Space)

## Tasks / Subtasks

- [x] Task 1: Create AppLayout shell component (AC: 1, 2, 7)
  - [x] Create `frontend/src/layouts/app-layout.tsx` — root layout wrapping `<Outlet />`
  - [x] Layout structure: fixed header (top) + flex row (sidebar + main content area)
  - [x] Header: app name "KanbanFlow" left-aligned, user menu right-aligned
  - [x] User menu: shadcn/ui `DropdownMenu` with user email and "Logout" action
  - [x] Main content area scrolls vertically, sidebar is fixed-height
  - [x] Apply Tailwind classes using design tokens (`bg-background`, `text-text-primary`, etc.)

- [x] Task 2: Create Sidebar component (AC: 2, 3, 4, 9)
  - [x] Create `frontend/src/layouts/sidebar.tsx`
  - [x] Width: 240px when expanded, 0px when collapsed, 300ms CSS transition on `width`
  - [x] Toggle button at top-left of sidebar (arrow icon, rotates on toggle)
  - [x] Sidebar collapsed by default — localStorage key `'sidebar-collapsed'` defaults to true
  - [x] Sidebar content: project list (text list, clickable items)
  - [x] Active project highlighted with `bg-sidebar-accent`
  - [x] When collapsed, sidebar content is hidden (use `overflow-hidden`)
  - [x] Toggle button accessible: `aria-label="Toggle sidebar"`, focus ring on keyboard nav
  - [x] Store collapsed state in localStorage so preference persists across sessions

- [x] Task 3: Create Breadcrumbs component (AC: 6)
  - [x] Create `frontend/src/layouts/breadcrumbs.tsx`
  - [x] Renders in header area, inline with header
  - [x] Format: "Project: {name}" > "Board: {name}" with clickable links
  - [x] Built with `<nav>` + `<ol>` semantic pattern
  - [x] Current location (last crumb) is not a link, styled as text
  - [x] Hidden when on projects list (no context to show)

- [x] Task 4: Update App.tsx with nested routing (AC: 5, 7)
  - [x] Wrap authenticated routes in `<AppLayout />` route element
  - [x] Use React Router `Outlet` pattern for nested routes
  - [x] Routes structure:
    - `/login` → Login (no layout)
    - `/register` → Register (no layout)
    - `/` → AppLayout wrapper
      - `/` → Projects list (default)
      - `/projects/:projectId` → Project page (future)
      - `/projects/:projectId/boards/:boardId` → Board view (future)
    - `/admin` → Admin layout (future, separate)
  - [x] Protected routes redirect to `/login` if not authenticated
  - [x] Use existing auth context from `features/auth/auth-provider.tsx`

- [x] Task 5: Create user menu dropdown (AC: 1)
  - [x] Use shadcn/ui `DropdownMenu` component
  - [x] Trigger: user email text in header with ChevronDown
  - [x] Menu items: user email (display only), "Log out"
  - [x] Logout calls existing auth API endpoint and redirects to `/login`
  - [x] Style with design tokens, support dark mode via theme toggle

- [x] Task 6: Add responsive behavior (AC: 2)
  - [x] Desktop (1024px+): Sidebar toggleable, full layout
  - [x] Tablet (640-1023px): Sidebar auto-collapsed, can be toggled
  - [x] Mobile (<640px): Sidebar hidden, hamburger menu in header to toggle
  - [x] Use Tailwind responsive prefixes (`sm:`)

- [x] Task 7: Add tests (AC: all)
  - [x] Create `frontend/src/layouts/app-layout.test.tsx`:
    - Test: renders header with app name
    - Test: renders sidebar toggle button
    - Test: sidebar collapsed by default
    - Test: toggle expands/collapses sidebar
    - Test: collapsed state persists to localStorage
  - [x] Create `frontend/src/layouts/sidebar.test.tsx`:
    - Test: renders project list items
    - Test: active project is highlighted
    - Test: clicking project triggers navigation
    - Test: keyboard accessible toggle button
  - [x] Create `frontend/src/layouts/breadcrumbs.test.tsx`:
    - Test: renders project and board names
    - Test: current location is not a link
    - Test: hidden when no context

## Dev Notes

### Previous Story Intelligence

**From Story 1.5 (Design System Foundation):**
- CSS variables define Soft Teal color palette with light/dark mode tokens
- `use-theme` hook available at `frontend/src/hooks/use-theme.ts` — returns `{ theme, toggleTheme, setTheme }`
- Inter font loaded via `@font-face` in `index.css`
- Tailwind v4 configured with `@tailwindcss/vite` plugin — CSS-based config in `index.css` `@theme inline` block
- shadcn/ui components available: `button.tsx`, `input.tsx`, `dialog.tsx`, `badge.tsx`, `dropdown-menu.tsx`
- Label color tokens defined (Red, Orange, Yellow, Green, Blue, Purple) — not needed here but pattern reference
- Code Review fixes applied: duplicate CSS vars removed, media-query listener race condition fixed, SSR FOUC fixed

[Source: 1-5-design-system-foundation.md]

**From Story 1.4 (User Login/Logout):**
- `App.tsx` has React Router setup with routes for `/login` and `/` (projects page)
- `features/auth/auth-provider.tsx` provides auth context with `useAuth()` hook
- `features/auth/use-auth.ts` exports auth hook
- Login/logout API endpoints: `POST /api/auth/login`, `POST /api/auth/logout`, `GET /api/auth/me`
- Protected route pattern: check auth state, redirect to `/login` if not authenticated
- shadcn/ui `Input`, `Button`, `Toast` components used in auth forms

[Source: 1-4-user-login-logout.md]

**From Story 1.3 (User Registration):**
- shadcn/ui initialized with `components.json` config
- `lib/utils.ts` has `cn()` helper (clsx + tailwind-merge)
- Feature-based folder structure under `features/`

[Source: 1-3-user-registration.md]

**From Story 1.1 (Project Initialization):**
- Vite + React + TypeScript frontend at `frontend/`
- Shared `.prettierrc` at root: `semi: true, singleQuote: true, trailingComma: "all", printWidth: 100, tabWidth: 2`
- `vite.config.ts` configured with `@tailwindcss/vite` plugin

[Source: 1-1-project-initialization.md]

### Architecture Compliance

**Frontend Organization (from architecture.md):**
- Feature-based folders under `src/features/`
- Layout components in `src/layouts/` — page structure, no data fetching
- Shared UI from `src/components/ui/` (shadcn/ui) — never modify directly
- Custom hooks in `src/hooks/` for shared logic

[Source: architecture.md#Frontend Organization]

**Layout Structure (from architecture.md):**
```
frontend/src/layouts/
├── app-layout.tsx     # Main layout with header + sidebar + content
├── sidebar.tsx        # Collapsible sidebar with project navigation
└── breadcrumbs.tsx    # Breadcrumb navigation component
```

[Source: architecture.md#Complete Project Directory Structure]

**Routing (from architecture.md):**
- React Router for client-side routing
- SPA behavior: client-side routing, API-based data loading
- Nested routes with `<Outlet />` pattern

[Source: architecture.md#Frontend Architecture]

**Sidebar Specs (from ux-design-specification.md):**
- Width: 240px when expanded, 0px when collapsed
- Smooth 300ms CSS transition
- Collapsed by default
- Shows projects + boards list
- Toggle via arrow button
- Current location highlighted

[Source: ux-design-specification.md#Navigation Patterns]

**Header Specs (from ux-design-specification.md):**
- App name in header
- User menu (logout, settings)
- Theme toggle (dark mode switch)
- Fixed header stays visible while scrolling

[Source: ux-design-specification.md#UX Consistency Patterns]

**Breadcrumb Specs (from ux-design-specification.md):**
- Format: Project > Board name
- Clickable to navigate up hierarchy
- Provides context, not primary navigation
- Hidden when on projects list

[Source: ux-design-specification.md#Navigation Patterns]

### File Structure Requirements

```
frontend/src/
├── layouts/
│   ├── app-layout.tsx          # NEW — Main layout shell
│   ├── sidebar.tsx             # NEW — Collapsible sidebar
│   ├── breadcrumbs.tsx         # NEW — Breadcrumb navigation
│   ├── app-layout.test.tsx     # NEW — Layout tests
│   ├── sidebar.test.tsx        # NEW — Sidebar tests
│   └── breadcrumbs.test.tsx    # NEW — Breadcrumbs tests
├── App.tsx                     # MODIFY — Add nested routing with layout
├── features/
│   └── auth/                   # EXISTING — Use auth context
├── hooks/
│   └── use-theme.ts            # EXISTING — Theme hook
├── components/ui/
│   ├── dropdown-menu.tsx       # EXISTING — Use for user menu
│   ├── button.tsx              # EXISTING — Use for toggle
│   └── ...                     # DO NOT MODIFY shadcn components
└── lib/
    ├── api.ts                  # EXISTING — API client
    └── utils.ts                # EXISTING — cn() utility
```

[Source: architecture.md#Complete Project Directory Structure]

### Testing Requirements

- **Framework:** Vitest with React Testing Library — co-located `.test.tsx` files
- **Test patterns:** Mock `localStorage` for sidebar state, mock auth context for protected routes
- **Coverage:** Layout rendering, sidebar toggle, breadcrumb navigation, routing
- **Existing tests:** 22 frontend tests pass — new tests must not break them

[Source: project-context.md#Testing Rules]

### Critical Anti-Patterns

- ❌ NEVER modify shadcn/ui components directly in `components/ui/`
- ❌ NEVER use hardcoded color values — use CSS variables / Tailwind tokens
- ❌ NEVER use `any` type — strict TypeScript
- ❌ NEVER fetch data in layout components — layouts are for structure only
- ❌ NEVER create custom Button or DropdownMenu when shadcn equivalents exist
- ❌ NEVER mix `snake_case` and `camelCase` in the same layer
- ❌ NEVER override Prettier formatting — shared config at root

[Source: project-context.md#Critical Don't-Miss Rules]

### Performance Considerations

- CSS transitions are GPU-accelerated — sidebar expand/collapse is smooth at 60fps
- No data fetching in layout — avoids unnecessary loading states
- Sidebar state in localStorage — instant restore on page load
- React Router handles route-level code splitting if needed later

[Source: architecture.md#Performance Requirements]

### Accessibility

- Sidebar toggle: `aria-label="Toggle sidebar"`, keyboard focusable
- Breadcrumbs: semantic `<nav aria-label="Breadcrumb">` with `<ol>` list
- User menu: shadcn DropdownMenu handles ARIA automatically
- Focus management: visible focus rings using `ring-primary` or `ring-2 ring-teal-600`
- All interactive elements minimum 44px touch target

[Source: ux-design-specification.md#Accessibility Strategy]

### Git Intelligence

**Recent commits:**
- `d61e129` — create & dev story: 1-5: design system foundation
- `ac2dd3a` — code review fixes for story 1-4: user login/logout
- `a5271c1` — dev story: 1-4: user login/logout
- `0fc73d3` — create-story: 1-4: user login/logout
- `ecc0393` — code review fixes: 1-3 user registration

**Pattern observations:**
- Stories follow: create-story → dev-story → code review → code review fixes
- Commits use conventional format: `feat:`, `fix:`, `chore:`
- Each story builds on previous — auth patterns from 1-3/1-4 must be reused

### References

- [Source: epics.md#Story 1.6] — User story and acceptance criteria
- [Source: ux-design-specification.md#Navigation Patterns] — Sidebar, breadcrumb specs
- [Source: ux-design-specification.md#UX Consistency Patterns] — Button hierarchy, feedback
- [Source: ux-design-specification.md#Responsive Design] — Breakpoint strategy
- [Source: ux-design-specification.md#Accessibility Strategy] — WCAG requirements
- [Source: architecture.md#Frontend Organization] — Feature-based structure
- [Source: architecture.md#Complete Project Directory Structure] — File locations
- [Source: 1-5-design-system-foundation.md] — Design tokens, use-theme hook, shadcn components
- [Source: 1-4-user-login-logout.md] — Auth context, routing patterns, API endpoints
- [Source: project-context.md#Framework-Specific Rules] — React frontend rules
- [Source: project-context.md#Critical Don't-Miss Rules] — Anti-patterns

## Dev Agent Record

### Agent Model Used
mimo-v2-pro-free

### Debug Log References
- Fixed `getStoredCollapsed` bug: `null === 'true'` was `false`, causing sidebar to default to expanded instead of collapsed. Changed to check `stored === 'false'` explicitly so default is collapsed.
- Fixed test issue: Two "Toggle sidebar" buttons exist (header mobile + sidebar). Used `within(aside)` to target the sidebar's toggle specifically.
- Added `window.matchMedia` mock to app-layout.test.tsx for jsdom compatibility.

### Completion Notes List
- All 7 tasks implemented and tested
- 44 frontend tests pass (22 existing + 22 new)
- AppLayout provides header, sidebar, breadcrumbs, and content area via `<Outlet />`
- Sidebar defaults to collapsed, persists state to localStorage
- Responsive behavior: mobile hamburger, tablet auto-collapse, desktop toggleable
- User menu uses existing shadcn/ui DropdownMenu + auth context
- Breadcrumbs hidden when no project context available

### File List
- `frontend/src/layouts/app-layout.tsx` (NEW)
- `frontend/src/layouts/sidebar.tsx` (NEW)
- `frontend/src/layouts/breadcrumbs.tsx` (NEW)
- `frontend/src/layouts/app-layout.test.tsx` (NEW)
- `frontend/src/layouts/sidebar.test.tsx` (NEW)
- `frontend/src/layouts/breadcrumbs.test.tsx` (NEW)
- `frontend/src/App.tsx` (MODIFIED — nested routing with AppLayout)

### Change Log
- 2026-03-21: Implemented app shell layout with header, collapsible sidebar, breadcrumbs, nested routing, user menu, responsive behavior, and tests (44 tests passing)
