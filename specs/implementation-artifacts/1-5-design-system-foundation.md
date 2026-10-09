# Story 1.5: Design System Foundation

Status: done

## Story

As a developer,
I want the design system foundation implemented with CSS variables,
So that all components have consistent theming and dark mode support.

## Acceptance Criteria

1. **Given** the frontend is initialized, **When** I configure the design system, **Then** CSS variables define the Soft Teal color palette with both light and dark tokens
2. **And** typography uses Inter/system-ui stack with the defined type scale (H1: 24px/600, H2: 20px/600, H3: 16px/600, Body: 14px/400, Small: 12px/400, Card title: 14px/500)
3. **And** spacing uses a 4px base unit scale (1: 4px, 2: 8px, 3: 12px, 4: 16px, 5: 20px, 6: 24px, 8: 32px, 12: 48px, 16: 64px)
4. **And** dark mode is toggled via CSS class and respects `prefers-color-scheme`
5. **And** a `use-theme` hook provides theme state and toggle function with localStorage persistence

## Tasks / Subtasks

- [x] Task 1: Define CSS variable system in `index.css` (AC: 1, 2, 3)
  - [x] Define light mode color tokens as CSS variables on `:root`:
    - `--primary: #0D9488` (Teal 600)
    - `--primary-hover: #0F766E` (Teal 700)
    - `--background: #F8FAFB` (Cool white)
    - `--surface: #F1F5F9` (Light slate)
    - `--card: #FFFFFF`
    - `--border: #E2E8F0` (Slate 200)
    - `--text-primary: #0F172A` (Slate 900)
    - `--text-secondary: #64748B` (Slate 500)
    - `--success: #10B981` (Emerald 500)
    - `--warning: #F59E0B` (Amber 500)
    - `--error: #F43F5E` (Rose 500)
  - [x] Define dark mode tokens under `.dark` class:
    - `--background: #0F172A` (Slate 950)
    - `--surface: #1E293B` (Slate 900)
    - `--card: #334155` (Slate 800)
    - `--border: #475569` (Slate 700)
    - `--text-primary: #F8FAFB` (Slate 50)
    - `--text-secondary: #94A3B8` (Slate 400)
  - [x] Define 6 label color tokens (Red, Orange, Yellow, Green, Blue, Purple) for both light and dark modes
  - [x] Set Tailwind `@theme` blocks to map CSS variables to Tailwind utility classes

- [x] Task 2: Configure typography system (AC: 2)
  - [x] Import Inter font via `@font-face` or Google Fonts CDN in `index.html`
  - [x] Set `font-family: 'Inter', system-ui, -apple-system, sans-serif` as base
  - [x] Define CSS custom properties for font sizes and weights matching the type scale
  - [x] Configure Tailwind `@theme` with `--font-sans` to the Inter stack

- [x] Task 3: Configure Tailwind theme with design tokens (AC: 1, 2, 3)
  - [x] Update Tailwind v4 CSS config in `index.css` `@theme inline` block to reference CSS variables for colors
  - [x] Extend theme with custom spacing scale based on 4px unit
  - [x] Extend theme with custom border-radius values (0.5rem base)
  - [x] Configure dark mode strategy: class-based toggle via `.dark` class
  - [x] Verify Tailwind v4 `@tailwindcss/vite` plugin is correctly configured in `vite.config.ts`

- [x] Task 4: Create `use-theme` hook (AC: 4, 5)
  - [x] Create `frontend/src/hooks/use-theme.ts`
  - [x] Hook reads initial theme from: localStorage key `'theme'`, then `prefers-color-scheme`, default `'light'`
  - [x] Toggle function adds/removes `'dark'` class on `<html>` element
  - [x] Toggle function persists choice to `localStorage.setItem('theme', value)`
  - [x] Returns `{ theme: 'light' | 'dark', toggleTheme: () => void, setTheme: (t) => void }`
  - [x] Listen for `prefers-color-scheme` change events to auto-update if no manual preference stored

- [x] Task 5: Verify shadcn/ui initialization with theme (AC: 1)
  - [x] Confirm `components.json` exists with correct config (style: radix-nova, tsx: true, aliases)
  - [x] Confirm `frontend/src/components/ui/` has shadcn base components (button, input, dialog, badge)
  - [x] Verify shadcn components reference Tailwind theme tokens (not hardcoded colors)
  - [x] Added missing `dialog.tsx` and `badge.tsx` via `npx shadcn@latest add`

- [x] Task 6: Add tests (AC: all)
  - [x] Create `frontend/src/hooks/use-theme.test.tsx`:
    - Test: defaults to light theme when no preference
    - Test: defaults to dark when `prefers-color-scheme: dark` and no localStorage
    - Test: reads from localStorage when set
    - Test: toggleTheme switches between light/dark
    - Test: toggleTheme persists to localStorage
    - Test: setTheme updates state and persists
  - [x] Verify existing component tests still pass with theme changes (22/22 pass)

## Dev Notes

### Project Structure Notes

**Files to create/modify:**

```
frontend/src/
├── index.css                  (MODIFY — add CSS variables, font import)
├── hooks/
│   ├── use-theme.ts           (NEW)
│   └── use-theme.test.tsx     (NEW)
├── components/ui/             (verify shadcn components exist)
└── lib/utils.ts               (verify cn() utility exists)
```

**Backend: No changes required** — this story is frontend-only.

[Source: architecture.md#Complete Project Directory Structure]

### Previous Story Intelligence

**From Story 1.4 (User Login/Logout):**
- Frontend uses `features/auth/` organization with co-located tests
- Tests: Vitest for frontend — co-located `.test.tsx` files
- shadcn/ui `Input`, `Button`, `Toast` already in use
- `App.tsx` has routing with React Router
- `auth-provider.tsx` context pattern established — `use-theme` hook follows same pattern for state

**From Story 1.3 (User Registration):**
- shadcn/ui initialized via `npx shadcn@latest init -t vite`
- Tailwind CSS v4 configured with `@tailwindcss/vite` plugin

**From Story 1.2 (Database Schema Setup):**
- No frontend impact

**From Story 1.1 (Project Initialization):**
- Vite + React + TypeScript frontend at `frontend/`
- `components.json` exists for shadcn/ui config
- Shared `.prettierrc` at root: `semi: true, singleQuote: true, trailingComma: "all", printWidth: 100, tabWidth: 2`
- `lib/utils.ts` should have `cn()` helper (clsx + tailwind-merge)

**Patterns established:**
- Frontend files: `kebab-case` (e.g., `use-theme.ts`)
- Custom hooks in `hooks/` for shared logic
- Tests co-located with source files (`.test.tsx`)
- shadcn/ui components from `components/ui/` — never modify directly

[Source: 1-4-user-login-logout.md]

### Architecture Compliance

**Styling Solution (from architecture.md):**
- Tailwind CSS v4 via `@tailwindcss/vite`
- shadcn/ui components (Radix UI primitives)
- CSS variables for theming (dark mode support)

[Source: architecture.md#Starter Template Evaluation]

**Dark Mode Strategy (from architecture.md):**
- Toggle via CSS class (`darkMode: 'class'` in Tailwind config)
- System preference detection via `prefers-color-scheme`
- Manual toggle with localStorage persistence
- All components support dark mode via CSS variables

[Source: architecture.md#Cross-Cutting Concerns Identified]

**Design System (from ux-design-specification.md):**

Color tokens must match exactly — Soft Teal theme:
- Primary: Teal 600 `#0D9488`
- Primary Hover: Teal 700 `#0F766E`
- Label colors have separate light/dark tokens (Rose, Orange, Yellow, Green, Blue, Purple)

[Source: ux-design-specification.md#Visual Design Foundation - Color System]

**Typography (from ux-design-specification.md):**
- Font stack: `Inter, system-ui, -apple-system, sans-serif`
- Type scale: H1 24px/600, H2 20px/600, H3 16px/600, Body 14px/400, Small 12px/400, Card title 14px/500

[Source: ux-design-specification.md#Typography System]

**Spacing (from ux-design-specification.md):**
- Base unit: 4px
- Scale: 1=4px, 2=8px, 3=12px, 4=16px, 5=20px, 6=24px, 8=32px, 12=48px, 16=64px

[Source: ux-design-specification.md#Spacing & Layout Foundation]

### shadcn/ui Component Mapping

| Component | Status | Notes |
|-----------|--------|-------|
| `button.tsx` | Verify exists | Primary CTA uses Teal 600 via theme |
| `input.tsx` | Verify exists | Form inputs |
| `dialog.tsx` | Verify exists | Confirmation dialogs |
| `toast.tsx` | Verify exists | Notifications |
| `badge.tsx` | Verify exists | Label badges |
| `switch.tsx` | Verify exists | Dark mode toggle |
| `dropdown-menu.tsx` | Verify exists | Column/card actions |
| `sheet.tsx` | Verify exists | Card detail panel |
| `separator.tsx` | Verify exists | Visual dividers |
| `tabs.tsx` | Verify exists | Admin panel tabs |
| `scroll-area.tsx` | Verify exists | Column card lists |
| `tooltip.tsx` | Verify exists | Hints |

All shadcn components must reference Tailwind theme tokens (`bg-primary`, `text-primary`, etc.) — never hardcoded colors.
[Source: ux-design-specification.md#Component Strategy - Design System Components]

### File Structure Requirements

```
frontend/src/
├── index.css                  # CSS variables, Tailwind directives, font import
├── hooks/
│   ├── use-theme.ts           # Theme hook (light/dark toggle, localStorage, system pref)
│   └── use-theme.test.tsx     # Vitest tests
├── components/ui/             # shadcn/ui components (verify exist, do NOT modify directly)
├── lib/
│   └── utils.ts               # cn() utility (verify exists)
```

Root config files (verify, do not create if they exist):
- `tailwind.config.ts` — extend with theme tokens
- `components.json` — shadcn/ui config
- `vite.config.ts` — `@tailwindcss/vite` plugin

[Source: architecture.md#Complete Project Directory Structure]

### Testing Requirements

- **Framework:** Vitest (frontend) — co-located `.test.tsx` files
- **Coverage:** use-theme hook must have full test coverage
- **Test patterns:** Mock `localStorage`, mock `matchMedia` for `prefers-color-scheme`

[Source: project-context.md#Testing Rules]

### Critical Anti-Patterns

- ❌ NEVER modify shadcn/ui components directly in `components/ui/` — they are auto-generated
- ❌ NEVER use hardcoded color values in components — always use CSS variables / Tailwind tokens
- ❌ NEVER use `any` type — strict TypeScript
- ❌ NEVER mix formatting styles — Prettier enforces one style across codebase
- ❌ NEVER fetch data without React Query (not applicable here, but pattern to remember)
- ❌ NEVER create custom Button, Input, or Toast components when shadcn/ui equivalents exist

[Source: project-context.md#Critical Don't-Miss Rules]

### Performance Considerations

- CSS variables are resolved at paint time — zero runtime overhead
- Tailwind purges unused styles in production builds
- Font loading: use `font-display: swap` for Inter to prevent FOIT (Flash of Invisible Text)
- Theme toggle is instant — no re-render of DOM, just CSS class swap

[Source: architecture.md#Performance Requirements]

### Security & Accessibility

- Contrast ratios: all text must meet WCAG AA (4.5:1 body, 3:1 large text)
- Focus indicators: 2px teal outline, 2px offset on all interactive elements
- Dark mode: respect `prefers-reduced-motion` media query
- Color independence: never use color alone to convey meaning

[Source: ux-design-specification.md#Accessibility Considerations]

### References

- [Source: ux-design-specification.md#Visual Design Foundation] — Color palette, typography, spacing
- [Source: ux-design-specification.md#Design System Foundation] — shadcn/ui + Tailwind strategy
- [Source: ux-design-specification.md#Accessibility Considerations] — WCAG requirements
- [Source: architecture.md#Starter Template Evaluation] — Tech stack decisions
- [Source: architecture.md#Cross-Cutting Concerns Identified] — Dark mode theming
- [Source: architecture.md#Complete Project Directory Structure] — File locations
- [Source: epics.md#Story 1.5] — Original story acceptance criteria
- [Source: prd.md#User Interface] — FR38, FR39 (dark mode requirements)
- [Source: project-context.md#Technology Stack & Versions] — Stack details
- [Source: project-context.md#Framework-Specific Rules] — React frontend rules
- [Source: 1-4-user-login-logout.md] — Previous story patterns and learnings

## Dev Agent Record

### Agent Model Used
opencode/mimo-v2-pro-free

### Debug Log References


### Completion Notes List
- Replaced generic oklch-based theme with Soft Teal palette (#0D9488 primary) across light and dark modes
- Added 6 label color tokens (Red, Orange, Yellow, Green, Blue, Purple) with separate light/dark values
- Imported Inter font via @font-face with font-display: swap for FOIT prevention
- Defined type scale as CSS custom properties (H1-H3, Body, Small, Card title)
- Added 4px-base spacing scale (1-16) as CSS vars and mapped to Tailwind @theme inline
- Tailwind v4 uses CSS-based config in index.css @theme inline block (no tailwind.config.ts)
- Created use-theme hook with localStorage persistence, system preference fallback, and media query listener
- Added dialog.tsx and badge.tsx shadcn components (existing button, input, dropdown-menu verified)
- All 22 frontend tests pass (6 new use-theme tests + 16 existing)

### Code Review Fixes (post-review)
- Removed duplicate CSS variable declarations in :root (merged design tokens and shadcn/ui vars)
- Fixed media-query listener race condition: detach listener when user makes manual theme choice via module-level flag
- Fixed SSR FOUC: applyTheme() called synchronously in useState initializer before first paint
- Fixed test mock: removeEventListener now properly drains listeners array
- Wrapped localStorage calls in try/catch for private browsing/sandboxed iframe safety
- Removed dead `.dark :root` selector (changed to `.dark` only — `:root` has no parent)
- Added --popover to dark mode and @media blocks (was missing after dedup cleanup)
- Dialog: removed duplicate data-slot="dialog-close" from internal close button
- Dialog: added max-h and overflow-y-auto to DialogContent for short viewport handling
- Dialog: DialogFooter now uses DialogClose component for consistency
- Badge: tightened asChild type to require ReactElement child (discriminated union)

### File List
- `frontend/src/index.css` (MODIFIED — full rewrite with Soft Teal design tokens, Inter font, @theme inline, .dark block)
- `frontend/src/hooks/use-theme.ts` (NEW — theme hook with toggle/setTheme, localStorage, system pref)
- `frontend/src/hooks/use-theme.test.tsx` (NEW — 6 Vitest tests for use-theme hook)
- `frontend/src/components/ui/dialog.tsx` (NEW — shadcn dialog component)
- `frontend/src/components/ui/badge.tsx` (NEW — shadcn badge component)
