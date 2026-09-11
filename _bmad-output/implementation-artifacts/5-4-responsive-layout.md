# Story 5.4: Responsive Layout

Status: ready-for-dev

## Story

As a user,
I want the app to work well on desktop, tablet, and mobile,
so that I can access my boards from any device.

## Acceptance Criteria

1. **Given** I am on desktop (1024px+)
   **When** I view the app
   **Then** I see the full layout with horizontal scrolling columns
   **And** the sidebar is collapsible

2. **Given** I am on tablet (640-1023px)
   **When** I view the app
   **Then** the sidebar auto-collapses
   **And** columns still scroll horizontally
   **And** touch targets are minimum 48x48px (WCAG 2.5.8)

3. **Given** I am on mobile (<640px)
   **When** I view the app
   **Then** I see a single column view
   **And** I can swipe left/right to switch columns
   **And** the sidebar becomes a bottom sheet accessible via hamburger menu
   **And** cards stack vertically
   **And** drag-drop uses long-press (500ms) to initiate

4. **Given** I rotate my device between portrait and landscape
   **When** the viewport crosses a breakpoint
   **Then** the layout transitions smoothly without jarring
   **And** any in-progress swipe or drag operation completes or cancels gracefully

5. **Given** I am on a notched device (iPhone X+, Android punch-hole)
   **When** I view the app
   **Then** content respects safe area insets via `env(safe-area-inset-*)`

## Tasks / Subtasks

- [ ] Create responsive breakpoint hook `useBreakpoint` (AC: #1, #2, #3)
  - [ ] Return current breakpoint: 'mobile' | 'tablet' | 'desktop'
  - [ ] Use `matchMedia` with change listener
  - [ ] Define breakpoints: mobile <640px, tablet 640-1023px, desktop 1024px+
  - [ ] Handle orientation changes that cross breakpoints

- [ ] Update sidebar behavior for responsive (AC: #2, #3)
  - [ ] Auto-collapse on tablet (already implemented in app-layout.tsx)
  - [ ] Create new `MobileBottomSheet` component for mobile sidebar
  - [ ] Implement slide-up animation with backdrop overlay
  - [ ] Add swipe-down to dismiss gesture
  - [ ] Add hamburger menu trigger in mobile header

- [ ] Implement mobile column view (AC: #3)
  - [ ] Show single column on mobile
  - [ ] Add swipe left/right navigation between columns
  - [ ] Show column indicator dots
  - [ ] Stack cards vertically in single column
  - [ ] Pre-fetch adjacent column data for instant swipe transitions

- [ ] Update touch targets for tablet/mobile (AC: #2, #3)
  - [ ] Ensure all interactive elements have 48x48px minimum touch target
  - [ ] Update button sizes: `h-12 min-w-[48px]` for primary actions
  - [ ] Add 8px minimum spacing between touch targets
  - [ ] Audit: column header menu, card labels, due date badges, checklist checkboxes, filter chips, search clear, view toggle

- [ ] Implement mobile drag-drop (AC: #3)
  - [ ] Update dnd-kit TouchSensor delay from 300ms to 500ms
  - [ ] Add visual feedback on long-press start (scale + shadow)
  - [ ] Maintain existing drop behavior

- [ ] Add mobile search/filter UI adaptation (AC: #3)
  - [ ] Collapse search input to icon on mobile, expand on tap
  - [ ] Move filter dropdown to bottom sheet on mobile
  - [ ] Move view toggle to hamburger menu on mobile

- [ ] Adapt card detail panel for mobile (AC: #3)
  - [ ] Render as full-screen modal on mobile (<640px)
  - [ ] Add swipe-down to close gesture
  - [ ] Maintain existing sheet behavior on tablet/desktop

- [ ] Adapt column header actions for mobile (AC: #3)
  - [ ] Add overflow menu for column actions on mobile
  - [ ] Ensure inline rename works with single-column view

- [ ] Add safe area inset handling (AC: #5)
  - [ ] Add `env(safe-area-inset-top)` padding to header
  - [ ] Add `env(safe-area-inset-bottom)` padding to mobile bottom sheet
  - [ ] Add `env(safe-area-inset-left/right)` for landscape mode

- [ ] Add viewport height handling for mobile
  - [ ] Use `100dvh` instead of `100vh` for mobile layouts
  - [ ] Fallback to `100vh` for browsers without dvh support

- [ ] Add `prefers-reduced-motion` awareness
  - [ ] Disable swipe animations when reduced motion preferred
  - [ ] Disable long-press feedback animation
  - [ ] Use simple opacity transitions instead

- [ ] Add responsive test coverage
  - [ ] Test breakpoint detection at 360px, 390px, 430px, 640px, 768px, 1024px
  - [ ] Test sidebar behavior at each breakpoint
  - [ ] Test mobile column navigation swipe
  - [ ] Test touch target sizes
  - [ ] Test orientation changes
  - [ ] Test on real iOS and Android devices

## Dev Notes

### Architecture Compliance

- **Stack:** React 19 + Tailwind CSS v4 + shadcn/ui + Vite 8
- **Breakpoint strategy:** Mobile-first with `matchMedia` API
- **State management:** Custom hooks for responsive state
- **Touch handling:** Pointer events for cross-device compatibility
- **Touch targets:** 48x48px minimum per WCAG 2.5.8 (8px spacing)

### Key Files to Modify

| File | Purpose |
|------|---------|
| `frontend/src/hooks/use-breakpoint.ts` | New hook for responsive breakpoints |
| `frontend/src/components/mobile-bottom-sheet.tsx` | New component for mobile sidebar |
| `frontend/src/layouts/app-layout.tsx` | Update sidebar for mobile bottom sheet |
| `frontend/src/features/boards/board-view/board-view.tsx` | Add mobile column navigation, search/filter adaptation |
| `frontend/src/features/columns/column.tsx` | Adjust column width for mobile |
| `frontend/src/features/cards/card-draggable.tsx` | Update TouchSensor delay to 500ms |
| `frontend/src/features/cards/card-detail-panel.tsx` | Full-screen modal on mobile |
| `frontend/src/features/columns/column-header.tsx` | Overflow menu for mobile |
| `frontend/src/index.css` | Safe area insets, dvh support |

### Responsive Breakpoints (Content-Driven)

```
Mobile:  < 640px   → Single column, bottom sheet sidebar, long-press drag
Tablet:  640-1023px → Auto-collapsed sidebar, horizontal scroll, 48px touch
Desktop: 1024px+   → Full layout, collapsible sidebar, click drag
```

Breakpoints use `rem` not `px` for user font-size scaling:
- Mobile: < 40rem (640px)
- Tablet: 40rem-63.9375rem (640-1023px)
- Desktop: 64rem+ (1024px+)

### Mobile Column Navigation Design

```
┌─────────────────────────────────┐
│ ← Board Name          [≡] Menu │
├─────────────────────────────────┤
│                                 │
│   ┌─────────────────────────┐   │
│   │      Column 1          │   │
│   │  ┌─────────────────┐   │   │
│   │  │ Card 1          │   │   │
│   │  ├─────────────────┤   │   │
│   │  │ Card 2          │   │   │
│   │  └─────────────────┘   │   │
│   └─────────────────────────┘   │
│                                 │
│         ● ○ ○ (dots)           │
│                                 │
└─────────────────────────────────┘
```

### Touch Target Requirements (WCAG 2.5.8)

All interactive elements must meet minimum 48x48px touch target with 8px spacing:
- Buttons: `h-12 min-w-[48px]` for primary actions
- Links: Adequate padding
- Form inputs: `h-12`
- List items: Adequate row height
- Column header menu: 48x48px touch area

### Mobile Bottom Sheet Sidebar

On mobile, sidebar transforms to bottom sheet:
- Triggered by hamburger menu in header
- Slides up from bottom with spring animation
- Swipe down to dismiss (gesture handler)
- Backdrop click to dismiss
- Full width with safe area padding
- New component: `MobileBottomSheet`

### Mobile Search/Filter Adaptation

```
Desktop:  [Search Input] [Filter] [View Toggle]
Mobile:   [🔍] → expands to full search on tap
          [Filter] → opens bottom sheet
          [≡ Menu] → contains view toggle
```

### Card Detail Panel on Mobile

```
Desktop/Tablet: Sheet slides from right (existing)
Mobile:         Full-screen modal with swipe-down to close
```

### Safe Area Insets

```css
/* Add to relevant fixed elements */
padding-top: env(safe-area-inset-top);
padding-bottom: env(safe-area-inset-bottom);
padding-left: env(safe-area-inset-left);
padding-right: env(safe-area-inset-right);
```

### Viewport Height

```css
/* Mobile: use dynamic viewport height */
height: 100dvh;

/* Fallback for older browsers */
@supports not (height: 100dvh) {
  height: 100vh;
}
```

### Previous Story Learnings (5-3-dark-mode)

- CSS variables work well for theming - can extend for responsive
- `matchMedia` is reliable for breakpoint detection
- Tailwind v4 responsive prefixes (`sm:`, `md:`, `lg:`) work as expected
- shadcn/ui components have good responsive support
- `prefers-reduced-motion` already in index.css - extend for mobile animations

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 5.4]
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Responsive]
- [Source: frontend/src/layouts/app-layout.tsx:56-80]
- [Source: frontend/src/features/boards/board-view/board-view.tsx:236]
- [Source: frontend/src/features/columns/column.tsx:26]
- [Source: frontend/src/features/cards/drag-drop-context.tsx:60-64]
- [Source: MDN Responsive Design Best Practices 2026]
- [Source: WCAG 2.5.8 Target Size (Minimum)]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List
