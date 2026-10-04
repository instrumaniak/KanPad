---
title: 'Custom Scrollbars'
slug: 'custom-scrollbars'
created: '2026-05-02T02:54:00.000Z'
status: 'completed'
stepsCompleted: [1, 2, 3, 4, 5]
tech_stack: ['React 19', 'Vite', 'Tailwind v4', 'shadcn/ui', 'CSS custom properties']
files_to_modify: ['frontend/src/index.css']
code_patterns: ['CSS variables in :root/.dark blocks', '@layer base for global styles']
test_patterns: ['Visual verification only (CSS-only feature)']
---

# Tech-Spec: Custom Scrollbars

**Created:** 2026-05-02

## Overview

### Problem Statement

Browser native scrollbars don't match the app's premium aesthetic and theme system. They appear inconsistent across browsers and break the cohesive visual experience of the application.

### Solution

Implement cross-browser compatible custom scrollbars using CSS `::-webkit-scrollbar` pseudo-elements for webkit browsers and `scrollbar-color` property for Firefox. Use CSS variables for theme-aware colors that automatically respond to light/dark mode.

### Scope

**In Scope:**
- Global custom scrollbar styles applied to all scrollable elements
- Light/dark mode theme support via CSS variables
- Cross-browser compatibility (Chrome, Firefox, Safari, Edge)
- Thin (8px), minimal design with rounded corners
- Subtle background track

**Out of Scope:**
- Hide-scrollbar on hover/timeout functionality
- Custom scrollbar with content momentum scrolling

## Context for Development

### Codebase Patterns

- Theme system uses CSS variables defined in `frontend/src/index.css`
- Light mode: `:root` block (lines 34-120) - `--surface: #F4F4F5`, `--border: #E4E4E7`
- Dark mode: `.dark` block (lines 123-170) - `--surface: #1B2028`, `--border: #323A46`
- All color tokens follow pattern `--color-name`
- Tailwind v4 is used with `@theme inline` for CSS var mapping
- Global styles live in `@layer base` (lines 244-278)

### Files to Reference

| File | Purpose |
| ---- | ------- |
| `frontend/src/index.css` (lines 34-120) | Light mode CSS variables - ADD scrollbar vars here |
| `frontend/src/index.css` (lines 123-170) | Dark mode CSS variables - ADD scrollbar vars here |
| `frontend/src/index.css` (lines 244-278) | @layer base - ADD global scrollbar styles here |

### Technical Decisions

1. Use CSS custom properties for scrollbar colors to support theme switching
2. **Firefox**: Use `scrollbar-color: thumb track` (order: thumb first, then track) - removed `scrollbar-width: thin` for thicker default sizing
3. **Webkit** (Chrome/Safari/Edge): Use `::-webkit-scrollbar` pseudo-elements (scrollbar, track, thumb, thumb:hover)
4. Keep minimal 8px width for modern look
5. Apply globally via `@layer base` to ensure it cascades to all scrollable elements
6. Add hover state on scrollbar thumb for better interactivity
7. Safari compatibility: Use standard webkit prefixes (works on Safari 14.1+)
8. Rounded corners via `border-radius: 4px` on thumb element (not track)

## Implementation Plan

### Tasks

1. Add scrollbar CSS variables to `:root` (light mode):
   - `--scrollbar-track: var(--surface)`
   - `--scrollbar-thumb: var(--border)`
   - `--scrollbar-thumb-hover: #C0C0C5` (slightly darker than border)
2. Add scrollbar CSS variables to `.dark` (dark mode):
   - `--scrollbar-track: var(--surface)`
   - `--scrollbar-thumb: var(--border)`
   - `--scrollbar-thumb-hover: #404858` (slightly darker than border)
3. Add global scrollbar styles in `@layer base`:
   - **Webkit** (Chrome/Safari/Edge):
     - `::-webkit-scrollbar` — width: 8px, height: 8px
     - `::-webkit-scrollbar-track` — background: var(--scrollbar-track)
     - `::-webkit-scrollbar-thumb` — background: var(--scrollbar-thumb), border-radius: 4px
     - `::-webkit-scrollbar-thumb:hover` — background: var(--scrollbar-thumb-hover)
   - **Firefox**:
     - `scrollbar-color: var(--scrollbar-thumb) var(--scrollbar-track)`
     - `scrollbar-width: thin`
4. Test in browser:
   - Verify light mode scrollbar appearance
   - Verify dark mode scrollbar appearance
   - Test theme switching
   - Check sidebar, columns, and modal scrollbars

### Acceptance Criteria

1. **GIVEN** a user views any scrollable area in light mode **WHEN** the scrollbar appears **THEN** it displays with 8px width, rounded thumb (4px radius), track using `--scrollbar-track` (maps to `--surface`) and thumb using `--scrollbar-thumb` (maps to `--border`)
2. **GIVEN** a user hovers over the scrollbar thumb **WHEN** hovering **THEN** the thumb color changes to `--scrollbar-thumb-hover` for visual feedback
3. **GIVEN** a user views any scrollable area in dark mode **WHEN** the scrollbar appears **THEN** it displays with 8px width, rounded thumb, track using `--scrollbar-track` and thumb using `--scrollbar-thumb` (dark mode variants)
4. **GIVEN** a user switches between light and dark mode **WHEN** the switch occurs **THEN** scrollbar colors update automatically via CSS variables
5. **GIVEN** a user uses any modern browser (Chrome, Firefox, Safari 14.1+, Edge) **WHEN** they encounter a scrollbar **THEN** it renders consistently across all browsers

## Additional Context

### Dependencies

None - pure CSS implementation

### Testing Strategy

- Manual visual verification across browsers
- Check light/dark mode toggle
- Verify all scrollable areas (sidebar, columns, modals)

### Notes

- **Firefox**: Use `scrollbar-color: thumb track` for cross-browser support
- **Webkit browsers**: Use `::-webkit-scrollbar` pseudo-elements (track, thumb, thumb:hover)
- Scrollbar colors should use existing CSS variables for consistency
- Include hover state: thumb darkens slightly on hover for interactivity feedback