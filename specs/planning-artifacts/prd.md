---
stepsCompleted: ["step-01-init", "step-02-discovery", "step-02b-vision", "step-02c-executive-summary", "step-03-success", "step-04-journeys", "step-05-domain", "step-06-innovation", "step-07-project-type", "step-08-scoping", "step-09-functional", "step-10-nonfunctional", "step-11-polish"]
inputDocuments: []
workflowType: 'prd'
project_name: KanbanFlow
author: Raziur
date: 2026-03-19
classification:
  projectType: web_app
  domain: productivity
  complexity: low
  projectContext: greenfield
vision:
  coreProblem: Personal productivity management without SaaS lock-in
  differentiator: Self-hosted, unlimited boards, fully customizable to your workflow
  targetUsers: Primarily personal use, with demo capability for others
  useCases: Managing projects and life organization - a true Trello replacement
  valueProposition: "Your boards, your server, your data - infinite possibilities, zero subscriptions"
success:
  userSuccess:
- Quick task capture
- Visual clarity
- Zero-friction organization
- Progress visibility
- Markdown for rich card details
- Notes for planning and documentation
  businessSuccess:
    - Zero maintenance overhead
    - Data safety
    - Reliable performance
  technicalSuccess:
    - cPanel deployable
    - Stable WebSocket (post-MVP)
    - Data integrity
  measurableOutcomes:
    - User can complete task workflow in under 2 minutes
    - Board loads in under 1 second
    - All CRUD operations complete reliably
---

# Product Requirements Document - KanbanFlow

**Author:** Raziur
**Date:** 2026-03-19

## Executive Summary

**KanbanFlow** is a self-hosted kanban board application that serves as a lightweight, full-featured alternative to Trello. Built for personal productivity management, it provides unlimited boards organized under customizable projects, with full data ownership and zero subscription costs.

Users can create and manage multiple boards with customizable backgrounds, define flexible columns per board, and organize cards with color-coded labels, markdown descriptions, and checklist tracking. The system supports drag-and-drop card management, per-column sorting, and search/filter capabilities.

### What Makes This Special

- **Self-hosted sovereignty** — Your data stays on your server. No SaaS subscriptions, no vendor lock-in, complete privacy.
- **Unlimited flexibility** — Unlimited boards under projects, each with fully customizable columns to match any workflow.
- **Developer-ready** — Built with modern, deployable tech (React/Vite, NestJS, MySQL) that you control end-to-end.
- **Demo-ready** — Clean UX and polished enough to invite others to explore and create their own boards.

### Project Classification

| Aspect | Value |
|--------|-------|
| **Project Type** | Web Application (SPA) |
| **Domain** | Productivity / Task Management |
| **Complexity** | Low |
| **Project Context** | Greenfield (new build) |

## Success Criteria

### User Success

- Quick task capture — User can quickly add a card to the right board/column with minimal friction
- Visual clarity — At a glance, user knows exactly what's in progress, what's done, what needs attention
- Zero friction organization — Creating boards, columns, and cards feels natural, not like fighting the system
- Progress visibility — Checklist progress bars give immediate feedback on task completion
- Markdown for rich card details — Edit in markdown, preview rendered output

### Business Success

- Zero maintenance overhead — Once deployed, it just works
- Data safety — No data loss, reliable persistence
- Reliable performance — Fast load times, responsive drag-drop

### Technical Success

- cPanel deployable — Works with cPanel Node.js app feature
- Data integrity — All CRUD operations are reliable

### Measurable Outcomes

- User can complete task workflow in under 2 minutes
- Board loads in under 1 second
- All CRUD operations complete reliably
- User can create a note in under 30 seconds
- Notes page loads in under 1 second
- Markdown preview renders within 500ms

## User Journeys

### The Organizer (Primary User)

**Persona:** Manages multiple projects and life aspects using kanban boards

**Journey: Setting Up a New Project**

| Step | Action | Emotional State |
|------|--------|-----------------|
| Opening | Login after a long day, brain full of things to do | Overwhelmed, scattered |
| Discovery | See board list, click "Create Board" | Hopeful |
| Action | Name board "Website Redesign", pick a color | Focused |
| Building | Board created with default columns: To Do, In Progress, Done | Creating order from chaos |
| Capture | Quickly add 5 cards for tasks I remember | Relieved, things are being captured |
| Capture | Quickly add 5 cards for tasks I remember | Relieved, things are being captured |
| Detail | Open a card, add checklist items, set due date | In control |
| Close | Drag card from "To Do" to "In Progress" as I start work | Satisfied |

**Critical Moment:** First drag-drop from one column to another — "This feels right."

---

### The Explorer (Demo User)

**Persona:** Curious visitor exploring the demo

**Journey: First-Time Exploration**

| Step | Action | Emotional State |
|------|--------|-----------------|
| Opening | Received link to your demo, clicks it | Curious |
| Entry | Register with email, set password | No friction |
| Onboarding | Land on empty boards page, see "Create your first board" prompt | Guided but not forced |
| Experimenting | Create a board with default columns, drag cards around | Playful, impressed |
| Discovery | Find dark mode toggle, switch to dark | Delighted |
| Value Moment | Add a card with checklist, see progress bar | "This actually works well" |
| Exit | Close browser, plan to return | Satisfied |

**Critical Moment:** Finding dark mode + checklist progress bar — "Oh, this has the details I care about."

---

### The Admin

**Persona:** Application owner managing users and settings

**Journey: Managing Users and Settings**

| Step | Action | Emotional State |
|------|--------|-----------------|
| Opening | Notices suspicious activity in logs | Alert |
| Investigation | Login to `/admin`, check user list | Detective mode |
| Action | See abnormal registrations, block offending users | Decisive |
| Resolution | Disable new registration temporarily | Problem solved |
| Recovery | Re-enable registration when issue resolved | Relief |

**Critical Moment:** Quick action to block user and prevent further abuse — "Chaos contained."

---

### Journey Requirements Summary

| Journey | Reveals These Needs |
|---------|---------------------|
| The Organizer | Quick card creation, drag-drop, board organization, checklist + progress |
| The Explorer | Clean onboarding, intuitive UI, dark mode, registration flow |
| The Admin | User management, settings toggle, activity logs, access control |

## Technical Architecture

### SPA Architecture

- **Frontend Framework:** React with Vite
- **UI Components:** shadcn (Radix UI based)
- **State Management:** React Query (TanStack Query) for server state
- **Routing:** React Router
- **SPA Behavior:** Client-side routing, API-based data loading

### Browser Support

| Browser | Support Level |
|---------|---------------|
| Chrome 90+ | Full support |
| Firefox 90+ | Full support |
| Safari 14+ | Full support |
| Edge 90+ | Full support |

Older browsers not supported.

## Product Scope

### MVP - Minimum Viable Product

**Authentication**
- User registration (email/password) — Registration open by default
- User login/logout
- Session management
- User roles: regular user, admin (CLI script to create initial superadmin)

**Boards**
- Create boards with background color (top-level, no project required)
- Edit board name and background
- Optionally group boards into projects for organization
- Board archiving (hidden from main view)
- Archive section (view archived, restore, permanently delete)
- Board list view (homepage)

**Projects**
- Create, view, edit projects (optional organizational containers)
- Delete projects (does NOT cascade to boards — boards become ungrouped)
- Project list view (secondary navigation)

**Columns**
- Create columns per board
- Edit column name
- Delete columns (cascades to cards)
- Column sorting by created date (asc/desc)
- Move all cards from one column to another

**Cards**
- Create cards with title
- Card details: title, description (markdown edit + preview), color-coded labels, due dates
- Checklist per card with progress bar showing completion %
- Drag-drop cards between columns
- Drag-drop cards within column (reorder)
- Card search by title
- Card filter by: labels, due date, checklist status
- Delete cards

**UI/UX**
- Dark mode (system preference auto-detect + manual toggle)
- Responsive layout
- Loading states and error handling

**Admin Panel (`/admin`)**
- Global settings: toggle new registration on/off
- User management: view user list, block/suspend users, delete users (cascade delete all user data)
- Activity log viewer: view logs for debugging abnormal situations
- Access restricted to admin role users

**Notes**
- Create notes with title and markdown content
- Edit and delete notes
- Link notes to boards, projects, or cards (optional)
- View all notes in a dedicated notes page (`/notes`)
- Search notes by title
- Filter notes by type (standalone, board-linked, project-linked, card-linked)
- Optional tags for note organization
- Markdown rendering with code syntax highlighting and mermaid diagrams
- Sanitized output to prevent XSS

### Growth Features (Post-MVP)

- WebSocket real-time updates
- Card attachments/files
- Board templates
- Data export/import
- User collaboration (multi-user boards)

### Vision (Future)

- Rich text editor for cards
- Card @mentions
- Email notifications

## Functional Requirements

### User Authentication

- FR1: Users can register with email and password
- FR2: Users can log in to access their workspace
- FR3: Users can log out to end their session
- FR4: System can prevent new user registration when disabled by admin

### Board Management

- FR5: Users can create boards (top-level, owned by user)
- FR6: Users can view their board list (homepage)
- FR7: Users can customize board background color
- FR8: Users can edit board name
- FR9: Users can optionally group boards into projects
- FR10: Users can archive boards (hidden from main view)
- FR11: Users can view archived boards in archive section
- FR12: Users can restore archived boards
- FR13: Users can permanently delete archived boards

### Project Management (Optional)

- FR14: Users can create projects as organizational containers
- FR15: Users can view their project list
- FR16: Users can edit project details (name)
- FR17: Users can delete projects (boards become ungrouped, NOT deleted)

### Column Management

- FR18: Users can create columns within a board
- FR19: Users can edit column names
- FR20: Users can delete columns (cascades to cards)
- FR21: Users can sort columns by created date (ascending/descending)
- FR22: Users can move all cards from one column to another column

### Card Management

- FR23: Users can create cards within a column
- FR24: Users can edit card title
- FR25: Users can write card description in markdown
- FR26: System renders markdown preview for card descriptions
- FR27: Users can assign color-coded labels to cards
- FR28: Users can set due dates on cards
- FR29: Users can create checklists on cards
- FR30: System displays checklist progress as a percentage bar
- FR31: Users can drag-drop cards between columns
- FR32: Users can drag-drop cards within a column (reorder)
- FR33: Users can delete cards
- FR34: Users can search cards by title
- FR35: Users can filter cards by labels
- FR36: Users can filter cards by due date
- FR37: Users can filter cards by checklist status

### User Interface

- FR38: System auto-detects and applies system dark/light mode preference
- FR39: Users can manually toggle between dark and light mode
- FR40: System displays loading states during data operations
- FR41: System displays error messages for failed operations
- FR42: Layout adapts to different screen sizes

### Administration

- FR43: Admin can access admin panel at `/admin`
- FR44: Admin can view list of all users
- FR45: Admin can block/suspend users
- FR46: Admin can delete users (cascade deletes all user data)
- FR47: Admin can toggle new user registration on/off
- FR48: Admin can view activity logs for debugging
- FR49: CLI script can create initial superadmin user

### Notes

- FR50: Users can create notes with a title and markdown content
- FR51: Users can edit note title and markdown content
- FR52: Users can delete notes permanently
- FR53: Users can optionally link a note to a board, project, or card
- FR54: Users can view all their notes in a dedicated notes page
- FR55: Users can search notes by title
- FR56: Users can filter notes by type (standalone, board-linked, project-linked, card-linked)
- FR57: Users can assign optional tags to notes for organization
- FR58: System renders markdown content with code syntax highlighting and mermaid diagrams

## Non-Functional Requirements

### Performance

- NFR1: Page loads complete within 2 seconds
- NFR2: Board renders within 1 second after data load
- NFR3: Drag-drop interactions respond within 100ms
- NFR4: API responses complete within 500ms
- NFR5: System remains responsive under typical single-user load

### Security

- NFR6: Passwords are securely hashed (bcrypt or equivalent)
- NFR7: Session tokens are securely managed
- NFR8: CSRF protection on all state-changing operations
- NFR9: XSS prevention on markdown rendering (sanitize output)
- NFR10: Rate limiting on authentication endpoints (login, registration)

### Reliability

- NFR11: Data persists reliably across server restarts
- NFR12: All CRUD operations complete successfully (no partial saves)
- NFR13: Graceful error handling with user-friendly messages
- NFR14: No data loss during server errors

### Accessibility

- NFR15: Standard keyboard navigation for core interactions
- NFR16: ARIA labels on interactive elements
- NFR17: Focus management for modals and dialogs
