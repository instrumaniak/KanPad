# Sprint Change Proposal: Notes System & Epic 4 Expansion

**Date:** 2026-05-20
**Module:** BMM (Build My Method)
**Project:** KanbanFlow — Personal Kanban Board
**Status:** Approved

---

## 1. Change Overview

### What Changed

User requests adding a **major Notes feature** (markdown, mermaid, code highlighting) + **minor features** (board view toggle, minor fixes) to the KanbanFlow project.

### Why This Change

- Notes system is a **core differentiator** — combines Google Docs + Trello into a single tool
- Supports "personal productivity sovereignty" vision with rich content capabilities
- User explicitly requested this as a priority feature

### Impact Level

**Moderate** — expands Epic 4 scope, requires new backend module + frontend feature, but no existing epics are invalidated.

---

## 2. Epic Impact Summary

| Epic | Impact | Required Changes |
|------|--------|------------------|
| **Epic 1–3** | None | Already completed — no changes needed |
| **Epic 4** (Rich Card Details) | **Major** | Renamed to "Rich Content, Notes & Organization". Added Stories 4.6 (Notes System) and 4.7 (Board View Toggle). |
| **Epic 5+** | Minor | No structural changes. Notes search may be added when Epic 5 is planned. |

**No epics invalidated. No new epics needed.**

---

## 3. Artifact Changes Applied

### 3.1 Sprint Status (`sprint-status.yaml`)

- Epic 3 and Story 3-5 marked **done**
- Epic 4 renamed to **"Rich Content, Notes & Organization"**
- Added stories: **4.6-notes-system** (backlog), **4.7-board-view-toggle** (backlog)

### 3.2 Epics Document (`epics.md`)

- Epic 4 renamed and scope expanded
- Added 9 new Functional Requirements (FR50–FR58) for Notes
- Added 3 new UX Design Requirements (UX-DR23–UX-DR25)
- Added Story 4.6: Notes System with full acceptance criteria
- Added Story 4.7: Board View Toggle (Kanban ↔ List) with full acceptance criteria
- Updated board-linked notes to use collapsible sidebar panel (not view toggle)
- Updated FR Coverage Map

### 3.3 PRD (`prd.md`)

- Added **Notes** to MVP scope (8 bullet points)
- Updated User Success criteria to include "Notes for planning and documentation"
- Updated Measurable Outcomes with Notes-specific metrics
- Added FR50–FR58 under new "Notes" section

### 3.4 Architecture (`architecture.md`)

- Added `react-markdown + rehype-highlight + mermaid` to tech stack
- Added `Notes & Tags` to implementation sequence (step 7)
- Added **Board entity** with `view_mode` column (`'kanban' | 'list'`, default `'kanban'`)
- Added Notes entity documentation with nullable FKs pattern
- Added **generic Tag entity** (cross-cutting design for future use with cards/projects/boards)
- Updated backend directory tree with `notes/` and `tags/` modules
- Updated frontend directory tree with `features/notes/` and `features/tags/`
- Updated cross-component dependencies for cascade deletes and tag extensibility
- Added Notes & Tags Data Model section with full TypeORM entity code

### 3.5 UX Design (`ux-design-specification.md`)

- Added comprehensive **Notes User Experience Design** section covering:
  - Notes Page (`/notes`) — two-pane layout (list + editor)
  - Board View Toggle — segmented control for Kanban ↔ List view with persisted preference
- Board Notes Panel — left sidebar panel on board pages, auto-visible when notes exist
- Header Navigation — tabs for Projects, Boards, Notes in top navigation bar
  - Card Notes Panel — tab within card detail panel
  - Markdown Rendering UX — edit/preview modes, syntax highlighting, mermaid
  - Component Specifications — NoteCard, NoteEditor, TagPicker

### 3.6 Project Context (`project-context.md`)

- Added markdown rendering libraries to frontend tech stack
- Added **Notes System Rules** section with implementation guidelines

---

## 4. Path Forward

### Recommended Approach: Direct Adjustment

**Justification:**
- Notes is an **additive feature** that naturally extends Epic 4's "Rich Content" theme
- No rollback needed — nothing to revert
- Existing architecture (NestJS modules, React feature folders) accommodates Notes cleanly
- Generic tag system is future-proof for cards, projects, and boards

### Next Steps

1. **Create Story** for 4.1 (card detail panel) — start Epic 4 implementation
2. **Create Story** for 4.6 (notes system) — can be implemented after 4.1–4.5 or in parallel depending on dependencies
3. **Continue sprint** with updated plan

### Effort Estimate

- **Epic 4 scope increase:** +2–3 story cycles (Notes + Board Toggle)
- **Technical risk:** Low
- **Timeline impact:** Moderate (2–3 additional stories)

---

## 5. Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Notes schema | Nullable FKs (`card_id`, `board_id`, `project_id`) | Simple, TypeORM-native, supports top-level notes (all NULL) |
| Card description | **NOT** replaced by Notes | Description stays as `cards.description` TEXT column |
| Tags | Normalized many-to-many (generic `Tag` entity) | Future-proof for cards/projects/boards, excellent query performance |
| Markdown libraries | `react-markdown` + `rehype-highlight` + `remark-gfm` + `mermaid` | Industry standard, well-maintained, fits React ecosystem |
| Notes UI | Hybrid: global `/notes` + contextual panels | Best of both worlds — dedicated space + contextual access |
| Board view mode | `boards.view_mode` enum (`'kanban' | 'list'`) | Persisted per board, default kanban, supports dual-purpose boards |
| Board-linked notes | Left sidebar panel on board pages | Contextual access, auto-visible when notes exist, collapsible, clean layout |
| Navigation | Header tabs [projects][boards][notes] | Top-level navigation, sidebar becomes contextual only |
| Visibility enum | **Deferred** | Collaboration is post-MVP; keeps scope focused |

---

## 6. Approval

**Change approved by:** User (Raziur)
**Date:** 2026-05-20
**Approach:** Direct Adjustment (expand Epic 4)

---

## Appendix: Files Modified

| File | Path | Changes |
|------|------|---------|
| Sprint Status | `_bmad-output/implementation-artifacts/sprint-status.yaml` | Epic 4 renamed, stories 4.6/4.7 added, epic-3/3-5 marked done |
| Epics | `_bmad-output/planning-artifacts/epics.md` | Epic 4 expanded, FR50–58, UX-DR23–25, Stories 4.6/4.7 |
| PRD | `_bmad-output/planning-artifacts/prd.md` | Notes added to MVP, success criteria updated, FR50–58 |
| Architecture | `_bmad-output/planning-artifacts/architecture.md` | Notes/Tags entities, modules, tech stack, sequence |
| UX Design | `_bmad-output/planning-artifacts/ux-design-specification.md` | Notes UX section (page, panels, markdown, components) |
| Project Context | `_bmad-output/project-context.md` | Tech stack + implementation rules for Notes |
