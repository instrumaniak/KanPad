# Story 5.3: Dark Mode

Status: done

## Story

As a user,
I want dark mode that respects my system preference and can be toggled manually,
so that I can use the app comfortably in any lighting condition.

## Acceptance Criteria

1. **Given** I open the app for the first time
   **When** my system prefers dark mode
   **Then** the app automatically applies dark theme

2. **Given** I want to override the system preference
   **When** I click the theme toggle in the header
   **Then** the app switches between light and dark mode
   **And** my preference is persisted in localStorage

3. **Given** dark mode is active
   **When** I view the app
   **Then** all colors use the dark mode tokens from the design system
   **And** all components render correctly in dark mode
   **And** contrast ratios meet WCAG AA requirements

## Tasks / Subtasks

- [x] Create `useTheme` hook (AC: #1, #2)
  - [x] Read theme from localStorage on mount
  - [x] Fall back to `prefers-color-scheme` media query
  - [x] Apply theme class (`light`/`dark`) to `document.documentElement`
  - [x] Toggle function that persists to localStorage
  - [x] Listen for OS preference changes when no manual selection
- [x] Define CSS variables for light/dark themes (AC: #3)
  - [x] Light mode tokens in `:root` selector
  - [x] Dark mode tokens in `.dark` selector
  - [x] Map tokens to Tailwind v4 theme via `@theme inline`
  - [x] Configure `@custom-variant dark` for class-based strategy
- [x] Add theme toggle button to app header (AC: #2)
  - [x] Ghost button with Sun/Moon icons from lucide-react
  - [x] Accessible with `aria-label="Toggle theme"`
- [x] Apply `dark:` Tailwind classes to components (AC: #3)
  - [x] Button variants (`dark:border-input`, `dark:bg-input/30`)
  - [x] Badge variants
  - [x] Input component
  - [x] Label colors (6 colors with dark variants)
  - [x] Due date badge colors
  - [x] Note type badges
  - [x] Markdown prose (`dark:prose-invert`)
- [x] Add FOUC prevention script to `index.html` (AC: #1, #3)
  - [x] Inline script in `<head>` reads localStorage before first paint
  - [x] Falls back to `prefers-color-scheme` media query
  - [x] Fixes auth pages (login/register) not receiving theme
- [x] Fix WCAG AA contrast ratios (AC: #3)
  - [x] Update `--muted-foreground` from `#7E8A99` to `#94A3B8` (5.30:1 on card)
  - [x] Verify all text/background combinations meet 4.5:1 ratio
- [x] Write tests for `useTheme` hook
  - [x] Default to light when no preference
  - [x] Default to dark when OS prefers dark
  - [x] Read from localStorage when set
  - [x] Toggle persists to localStorage
  - [x] Set theme updates state and persists

## Dev Notes

### Architecture Compliance

- **Stack:** React 19 + Tailwind CSS v4 + shadcn/ui + Vite 8
- **Theme strategy:** Class-based (`dark` on `<html>`) via `@custom-variant dark (&:where(.dark, .dark *))`
- **State management:** Custom hook `useTheme` using `useState`/`useEffect`/`useCallback`/`useRef`
- **Persistence:** `localStorage` key `'theme'` stores `'light'` or `'dark'`
- **No React Context:** Hook used directly in `app-layout.tsx` (sufficient for current needs)

### Key Files

| File | Purpose |
|------|---------|
| `frontend/src/hooks/use-theme.ts` | Core theme hook (79 lines) |
| `frontend/src/hooks/use-theme.test.tsx` | Hook tests (95 lines) |
| `frontend/src/index.css:36-182` | CSS variables for light/dark |
| `frontend/src/index.css:187-254` | Tailwind v4 theme mapping |
| `frontend/src/layouts/app-layout.tsx:139-141` | Toggle button |
| `frontend/index.html:12-21` | FOUC prevention script |

### Dark Mode Token Values

```css
.dark {
  --background: #0B0C0F;
  --surface: #1B2028;
  --card: #2A2E38;
  --border: #323A46;
  --text-primary: #DFE6EF;
  --text-secondary: #94A3B8;  /* Updated for WCAG AA */
  --primary: #DFE6EF;
  --primary-foreground: #0B0C0F;
  --muted: #1B2028;
  --muted-foreground: #94A3B8;  /* Updated for WCAG AA */
}
```

### WCAG AA Compliance

All text/background combinations now meet 4.5:1 minimum:

| Combination | Ratio |
|-------------|-------|
| text-primary on background | 15.56:1 |
| text-secondary on background | 7.63:1 |
| cardForeground on card | 10.80:1 |
| mutedForeground on muted | 6.38:1 |
| text-secondary on card | 5.30:1 |

### Implementation Notes

1. **FOUC fix:** Inline script in `index.html` runs synchronously before CSS paints, preventing white flash for dark mode users.

2. **Auth pages:** Previously rendered outside `AppLayout` where `useTheme` was called. FOUC script now ensures theme is applied before React mounts, fixing auth pages automatically.

3. **System preference tracking:** When no manual selection exists in localStorage, a `matchMedia` change listener follows OS changes in real-time.

4. **Label colors:** All 6 label colors (red, orange, yellow, green, blue, purple) have dark variants with proper contrast ratios (7:1+).

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.3]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Contrast ratios]
- [Source: frontend/src/hooks/use-theme.ts]
- [Source: frontend/src/index.css#Dark Mode Tokens]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

- Implemented with class-based strategy (`dark` on `<html>`) per Tailwind v4 best practices
- FOUC prevention script added to `index.html` to fix flash on initial load
- Auth pages theme enforcement fixed by inline script (no React dependency)
- WCAG AA contrast ratios verified for all text/background combinations
- All 588 tests passing

### File List

- `frontend/src/hooks/use-theme.ts` (created)
- `frontend/src/hooks/use-theme.test.tsx` (created)
- `frontend/src/index.css` (modified - dark tokens, WCAG fix)
- `frontend/src/layouts/app-layout.tsx` (modified - toggle button)
- `frontend/index.html` (modified - FOUC script)
- 10+ component files (modified - `dark:` classes)
