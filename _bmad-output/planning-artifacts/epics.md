---
stepsCompleted: ["step-01-validate-prerequisites", "step-02-design-epics", "step-03-create-stories", "step-04-final-validation"]
inputDocuments:
  - "_bmad-output/planning-artifacts/prd.md"
  - "_bmad-output/planning-artifacts/architecture.md"
  - "_bmad-output/planning-artifacts/ux-design-specification.md"
workflowType: 'epics-stories'
project_name: KanbanFlow
author: Raziur
date: 2026-03-19
---

# KanbanFlow - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for KanbanFlow, decomposing the requirements from the PRD, UX Design Specification, and Architecture decisions into implementable stories.

## Requirements Inventory

### Functional Requirements

FR1: Users can register with email and password
FR2: Users can log in to access their workspace
FR3: Users can log out to end their session
FR4: System can prevent new user registration when disabled by admin
FR5: Users can create projects
FR6: Users can view their project list
FR7: Users can edit project details (name)
FR8: Users can delete projects (cascades to boards)
FR9: System organizes boards under projects
FR10: Users can create boards within a project
FR11: Users can customize board background color
FR12: Users can edit board name
FR13: Users can view boards within a project
FR14: Users can archive boards (hidden from main view)
FR15: Users can view archived boards in archive section
FR16: Users can restore archived boards
FR17: Users can permanently delete archived boards
FR18: Users can create columns within a board
FR19: Users can edit column names
FR20: Users can delete columns (cascades to cards)
FR21: Users can sort columns by created date (ascending/descending)
FR22: Users can move all cards from one column to another column
FR23: Users can create cards within a column
FR24: Users can edit card title
FR25: Users can write card description in markdown
FR26: System renders markdown preview for card descriptions
FR27: Users can assign color-coded labels to cards
FR28: Users can set due dates on cards
FR29: Users can create checklists on cards
FR30: System displays checklist progress as a percentage bar
FR31: Users can drag-drop cards between columns
FR32: Users can drag-drop cards within a column (reorder)
FR33: Users can delete cards
FR34: Users can search cards by title
FR35: Users can filter cards by labels
FR36: Users can filter cards by due date
FR37: Users can filter cards by checklist status
FR38: System auto-detects and applies system dark/light mode preference
FR39: Users can manually toggle between dark and light mode
FR40: System displays loading states during data operations
FR41: System displays error messages for failed operations
FR42: Layout adapts to different screen sizes
FR43: Admin can access admin panel at /admin
FR44: Admin can view list of all users
FR45: Admin can block/suspend users
FR46: Admin can delete users (cascade deletes all user data)
FR47: Admin can toggle new user registration on/off
FR48: Admin can view activity logs for debugging
FR49: CLI script can create initial superadmin user
FR50: Users can create notes with a title and markdown content
FR51: Users can edit note title and markdown content
FR52: Users can delete notes permanently
FR53: Users can optionally link a note to a board, project, or card
FR54: Users can view all their notes in a dedicated notes page
FR55: Users can search notes by title
FR56: Users can filter notes by type (standalone, board-linked, project-linked, card-linked)
FR57: Users can assign optional tags to notes for organization
FR58: System renders markdown content with code syntax highlighting and mermaid diagrams

### NonFunctional Requirements

NFR1: Page loads complete within 2 seconds
NFR2: Board renders within 1 second after data load
NFR3: Drag-drop interactions respond within 100ms
NFR4: API responses complete within 500ms
NFR5: System remains responsive under typical single-user load
NFR6: Passwords are securely hashed (bcrypt or equivalent)
NFR7: Session tokens are securely managed
NFR8: CSRF protection on all state-changing operations
NFR9: XSS prevention on markdown rendering (sanitize output)
NFR10: Rate limiting on authentication endpoints (login, registration)
NFR11: Data persists reliably across server restarts
NFR12: All CRUD operations complete successfully (no partial saves)
NFR13: Graceful error handling with user-friendly messages
NFR14: No data loss during server errors
NFR15: Standard keyboard navigation for core interactions
NFR16: ARIA labels on interactive elements
NFR17: Focus management for modals and dialogs

### Additional Requirements

- Starter Template: Official Vite + NestJS CLIs (first implementation story)
- Database: MySQL 8.x with TypeORM (synchronize: false, migrations only)
- Session Management: Cookie-based httpOnly with SameSite CSRF protection
- API Style: REST with Swagger/OpenAPI auto-generation
- Frontend Stack: React/Vite, shadcn/ui, React Query, dnd-kit
- Deployment: cPanel Node.js app compatible
- Config: NestJS ConfigModule with .env validation
- Formatting: Shared Prettier config, ESLint, lint-staged + husky
- Error Handling: NestJS exception filters, React Query onError handlers
- Naming: DB snake_case, API kebab-case, Code camelCase/PascalCase
- Project Structure: Monorepo with frontend/ and backend/ directories

### UX Design Requirements

UX-DR1: Implement Soft Teal color system with light/dark mode tokens (Teal 600 primary)
UX-DR2: Implement typography system (Inter, system-ui stack) with defined type scale
UX-DR3: Implement spacing scale based on 4px base unit
UX-DR4: Build Board component with horizontal scroll and column management
UX-DR5: Build Column component with header, card list, inline add card
UX-DR6: Build Card component with drag-drop, labels, progress bar, due date display
UX-DR7: Build Checklist component with progress bar visualization
UX-DR8: Build LabelPicker component with 6-color system (Red, Orange, Yellow, Green, Blue, Purple)
UX-DR9: Build MarkdownEditor component with edit/preview toggle
UX-DR10: Build EmptyState components for all contexts (no projects, boards, cards, search, filter)
UX-DR11: Implement CSS variable-based theming for dark/light mode switching
UX-DR12: Implement WCAG AA accessibility (contrast, keyboard nav, ARIA, focus indicators)
UX-DR13: Implement responsive layout (desktop 1024px+, tablet 640-1023px, mobile <640px)
UX-DR14: Implement drag-drop with dnd-kit (mouse, keyboard, touch support)
UX-DR15: Implement quick card capture flow (inline input, Enter to create, stays focused)
UX-DR16: Implement button hierarchy (Primary, Secondary, Ghost, Destructive)
UX-DR17: Implement toast notification system (success, error, warning, info, undo)
UX-DR18: Implement form patterns (inline, modal, sheet) with validation
UX-DR19: Implement navigation with collapsible sidebar (240px, collapsed by default)
UX-DR20: Implement search and filter system (search, label, due date, checklist filters)
UX-DR21: Implement loading states (skeleton screens, optimistic updates, spinners)
UX-DR22: Implement micro-animations (card settle, checkmark, celebration)
UX-DR23: Build NoteEditor component with markdown edit/preview toggle and rich rendering
UX-DR24: Build NoteList component with type badges, search, and tag filtering
UX-DR25: Implement Notes navigation integration (sidebar link, global /notes route)

### FR Coverage Map

| FR | Epic | Description |
|----|------|-------------|
| FR1 | Epic 1 | User registration |
| FR2 | Epic 1 | User login |
| FR3 | Epic 1 | User logout |
| FR4 | Epic 1 | Registration toggle |
| FR5  | Epic 1 | Create boards (top-level) |
| FR6  | Epic 1 | View board list (homepage) |
| FR7  | Epic 1 | Board background color |
| FR8  | Epic 1 | Edit board name |
| FR9  | Epic 2 | Optionally group boards into projects |
| FR10 | Epic 2 | Archive boards |
| FR11 | Epic 2 | View archived boards |
| FR12 | Epic 2 | Restore archived boards |
| FR13 | Epic 2 | Delete archived boards |
| FR14 | Epic 1 | Create projects (optional) |
| FR15 | Epic 1 | View project list |
| FR16 | Epic 1 | Edit project details |
| FR17 | Epic 1 | Delete projects (boards ungrouped) |
| FR18 | Epic 2 | Create columns |
| FR19 | Epic 2 | Edit column names |
| FR20 | Epic 2 | Delete columns |
| FR21 | Epic 2 | Sort columns |
| FR22 | Epic 2 | Move all cards |
| FR23 | Epic 3 | Create cards |
| FR24 | Epic 3 | Edit card title |
| FR25 | Epic 4 | Markdown description |
| FR26 | Epic 4 | Markdown preview |
| FR27 | Epic 4 | Color-coded labels |
| FR28 | Epic 4 | Due dates |
| FR29 | Epic 4 | Checklists |
| FR30 | Epic 4 | Progress bar |
| FR31 | Epic 3 | Drag-drop between columns |
| FR32 | Epic 3 | Drag-drop within column |
| FR33 | Epic 3 | Delete cards |
| FR34 | Epic 5 | Search cards |
| FR35 | Epic 5 | Filter by labels |
| FR36 | Epic 5 | Filter by due date |
| FR37 | Epic 5 | Filter by checklist status |
| FR38 | Epic 5 | Auto-detect dark mode |
| FR39 | Epic 5 | Manual dark mode toggle |
| FR40 | Epic 5 | Loading states |
| FR41 | Epic 5 | Error messages |
| FR42 | Epic 5 | Responsive layout |
| FR43 | Epic 6 | Admin panel access |
| FR44 | Epic 6 | View user list |
| FR45 | Epic 6 | Block/suspend users |
| FR46 | Epic 6 | Delete users |
| FR47 | Epic 6 | Registration toggle |
| FR48 | Epic 6 | Activity logs |
| FR49 | Epic 1 | CLI superadmin creation |
| FR50 | Epic 4 | Create notes with markdown |
| FR51 | Epic 4 | Edit note title and content |
| FR52 | Epic 4 | Delete notes |
| FR53 | Epic 4 | Link note to board/project/card |
| FR54 | Epic 4 | View notes page |
| FR55 | Epic 4 | Search notes by title |
| FR56 | Epic 4 | Filter notes by type |
| FR57 | Epic 4 | Tag notes |
| FR58 | Epic 4 | Render markdown with code and mermaid |

## Epic List

### Epic 1: User Onboarding & Foundation

Users can register, login, and create their first board — the foundation that gets users productive immediately. Projects are optional and introduced later.
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR7, FR8, FR14, FR15, FR16, FR17, FR49
**UX Requirements:** UX-DR1, UX-DR2, UX-DR3, UX-DR10, UX-DR11, UX-DR16, UX-DR17, UX-DR19
**Additional:** Starter template, Database, Sessions, Config, Formatting

### Epic 2: Board & Project Organization

Users can manage boards as first-class entities and optionally organize them into projects. Columns provide customizable workflow structure.
**FRs covered:** FR9, FR10, FR11, FR12, FR13, FR18, FR19, FR20, FR21, FR22
**UX Requirements:** UX-DR4, UX-DR5

### Epic 3: Task Capture & Card Management
Users can quickly create, edit, and move cards — the core kanban interaction that delivers immediate productivity value.
**FRs covered:** FR23, FR24, FR31, FR32, FR33
**UX Requirements:** UX-DR6, UX-DR14, UX-DR15

### Epic 4: Rich Content, Notes & Organization
Users can add depth to cards with markdown descriptions, color-coded labels, due dates, checklists with progress tracking, and create rich markdown notes that can be board-linked or standalone for planning and documentation.
**FRs covered:** FR25, FR26, FR27, FR28, FR29, FR30, FR50, FR51, FR52, FR53, FR54, FR55, FR56, FR57, FR58
**UX Requirements:** UX-DR7, UX-DR8, UX-DR9, UX-DR23, UX-DR24, UX-DR25

### Epic 5: Search, Filter & Visual Polish
Users can find cards quickly, filter by criteria, and enjoy a polished experience with dark mode, responsive design, and delightful animations.
**FRs covered:** FR34, FR35, FR36, FR37, FR38, FR39, FR40, FR41, FR42
**UX Requirements:** UX-DR12, UX-DR13, UX-DR20, UX-DR21, UX-DR22

### Epic 6: Admin Panel & User Management
Admins can manage users, control registration, and monitor activity to keep the application secure and healthy.
**FRs covered:** FR43, FR44, FR45, FR46, FR47, FR48
**UX Requirements:** None specific (uses existing patterns)

---

## Epic 1: User Onboarding & Foundation

Users can register, login, and create their first board — the foundation that gets users productive immediately.

### Story 1.1: Project Initialization

As a developer,
I want to initialize the monorepo with Vite frontend and NestJS backend,
So that the project has a solid foundation with modern tooling.

**Acceptance Criteria:**

**Given** a fresh project directory
**When** I run the initialization commands
**Then** a monorepo structure is created with `frontend/` and `backend/` directories
**And** frontend uses Vite with React TypeScript template
**And** backend uses NestJS CLI with TypeScript
**And** shared `.prettierrc` is configured at root
**And** `frontend/` has shadcn/ui initialized with Tailwind CSS
**And** both projects have `package.json` with correct dependencies
**And** `.gitignore` covers node_modules, dist, .env files

### Story 1.2: Database Schema Setup

As a developer,
I want the database schema initialized with TypeORM,
So that the application can persist user and project data.

**Acceptance Criteria:**

**Given** a MySQL database is available
**When** I configure the `.env` file with database credentials
**Then** the NestJS ConfigModule loads configuration with validation
**And** TypeORM connects to MySQL with `synchronize: false`
**And** a `data-source.ts` file exists for TypeORM CLI
**And** initial migration creates `users` and `projects` tables
**And** `users` table has: id, email, password, role, created_at, updated_at
**And** `projects` table has: id, name, user_id (FK), created_at, updated_at
**And** `npm run migration:run` executes successfully

### Story 1.3: User Registration

As a new user,
I want to register with my email and password,
So that I can create an account and start using KanbanFlow.

**Acceptance Criteria:**

**Given** I am on the registration page
**When** I enter a valid email and password
**Then** my account is created and I am automatically logged in
**And** the password is hashed with bcrypt before storage
**And** a session cookie is set (httpOnly, SameSite)
**And** I am redirected to the projects page

**Given** I enter an email that already exists
**When** I submit the registration form
**Then** I see an error message "Email already registered"

**Given** registration is disabled by admin
**When** I try to access the registration page
**Then** I see a message "Registration is currently closed"

### Story 1.4: User Login/Logout

As a registered user,
I want to log in with my credentials and log out when done,
So that I can securely access my workspace.

**Acceptance Criteria:**

**Given** I am on the login page
**When** I enter valid email and password
**Then** I am authenticated and redirected to the projects page
**And** a session cookie is set

**Given** I am logged in
**When** I click the logout button
**Then** my session is cleared
**And** I am redirected to the login page

**Given** I enter invalid credentials
**When** I submit the login form
**Then** I see an error message "Invalid email or password"
**And** rate limiting prevents brute force attempts

### Story 1.5: Design System Foundation

As a developer,
I want the design system foundation implemented with CSS variables,
So that all components have consistent theming and dark mode support.

**Acceptance Criteria:**

**Given** the frontend is initialized
**When** I configure the design system
**Then** CSS variables define the Soft Teal color palette (light and dark tokens)
**And** typography uses Inter/system-ui stack with defined scale
**And** spacing uses a 4px base unit scale
**And** dark mode is toggled via CSS class and respects `prefers-color-scheme`
**And** a `use-theme` hook provides theme state and toggle function

### Story 1.6: App Shell & Navigation

As a user,
I want a consistent app layout with navigation,
So that I can easily move between projects and boards.

**Acceptance Criteria:**

**Given** I am logged in
**When** I view the application
**Then** I see a header with the app name and user menu
**And** I see a collapsible sidebar (240px expanded, 0px collapsed)
**And** the sidebar is collapsed by default
**And** the sidebar shows my boards list
**And** clicking a board navigates to it
**And** React Router handles client-side navigation

### Story 1.7: Project CRUD (Optional Organization)

As a user,
I want to create, view, edit, and delete projects,
So that I can optionally organize my boards into logical groups.

**Acceptance Criteria:**

**Given** I am on the boards page
**When** I click "Create Project" in the sidebar or projects section
**Then** an inline form appears for project name
**And** submitting creates the project and adds it to my projects list

**Given** I have projects
**When** I view the projects list
**Then** I see all my projects listed with names
**And** each project shows its board count (boards assigned to it)

**Given** I want to rename a project
**When** I click edit on a project
**Then** I can modify the name inline
**And** changes are saved on Enter or blur

**Given** I want to delete a project
**When** I click delete and confirm
**Then** the project is deleted but its boards become ungrouped (NOT deleted)
**And** a success toast appears with undo option (5 seconds)

### Story 1.8: Empty State & Toast System

As a user,
I want helpful empty states and notifications,
So that I know what to do next and get feedback on my actions.

**Acceptance Criteria:**

**Given** I have no boards
**When** I view the boards page (homepage)
**Then** I see an empty state with illustration, "Start organizing" headline, and "Create your first board" CTA

**Given** I have no projects
**When** I view the projects list
**Then** I see a simplified empty state with "Create a project" CTA (secondary action)

**Given** I perform an action (create, update, delete)
**When** the action completes
**Then** a toast notification appears in the bottom-right
**And** success toasts auto-dismiss after 3 seconds
**And** error toasts require manual dismiss
**And** destructive actions show an "Undo" button for 5 seconds

### Story 1.9: CLI Superadmin Creation

As an administrator,
I want a CLI script to create the initial superadmin user,
So that I can set up admin access before opening registration.

**Acceptance Criteria:**

**Given** the application is deployed
**When** I run `npm run create-admin` with email and password arguments
**Then** a user with admin role is created in the database
**And** the script validates email format and password strength
**And** the script prevents duplicate admin creation
**And** success message confirms admin creation

---

## Epic 2: Board & Project Organization

Users can manage boards as first-class entities and optionally organize them into projects. Columns provide customizable workflow structure.

### Story 2.1: Board CRUD

As a user,
I want to create, view, edit, and delete boards,
So that I can organize different workflows for different purposes.

**Acceptance Criteria:**

**Given** I am on the boards page (homepage)
**When** I click "Create Board"
**Then** a modal form appears with board name, background color picker, and optional project selector
**And** the color picker shows 8 preset colors
**And** the project selector shows my existing projects (or "No project" as default)
**And** submitting creates the board with default columns: "To Do", "In Progress", "Done"
**And** I am navigated to the new board view

**Given** I have boards
**When** I view the boards page
**Then** I see all my boards listed with name, background color preview, and project label (if assigned)
**And** boards are sorted by most recently updated

**Given** I want to rename a board
**When** I click edit on the board card or header
**Then** I can modify the name inline
**And** changes are saved on Enter or blur

**Given** I want to change board background
**When** I click the color picker in board settings
**Then** I can select a new color
**And** the board background updates immediately (optimistic UI)

**Given** I want to assign a board to a project (or remove from project)
**When** I open board settings
**Then** I can select a project or choose "No project"
**And** the change saves immediately

**Given** I want to delete a board
**When** I click delete and confirm
**Then** the board and all its columns/cards are deleted (cascade)
**And** a success toast appears with undo option (5 seconds)

### Story 2.2: Board Archiving

As a user,
I want to archive boards I'm not actively using,
So that my workspace stays clean without losing historical data.

**Acceptance Criteria:**

**Given** I am viewing a board
**When** I click "Archive Board"
**Then** the board is hidden from the main project view
**And** a toast confirms "Board archived" with undo option

**Given** I want to see archived boards
**When** I click "Archived Boards" in the boards page or board settings
**Then** I see a list of all my archived boards (across all projects)

**Given** I want to restore an archived board
**When** I click "Restore" on an archived board
**Then** the board reappears in the main project view

**Given** I want to permanently delete an archived board
**When** I click "Delete Permanently" and confirm
**Then** the board and all data are permanently removed
**And** a confirmation dialog explains the irreversible action

### Story 2.3: Column CRUD

As a user,
I want to create, edit, and delete columns within a board,
So that I can customize my workflow stages.

**Acceptance Criteria:**

**Given** I am viewing a board
**When** I click "Add Column"
**Then** a new column appears at the right end of the board
**And** the column has an editable name field focused

**Given** I want to rename a column
**When** I click the column header title
**Then** the name becomes editable inline
**And** changes are saved on Enter or blur

**Given** I want to delete a column
**When** I click the column menu and select "Delete"
**Then** a confirmation dialog appears
**And** confirming deletes the column and all its cards (cascade)
**And** a toast confirms the deletion

### Story 2.4: Column Sorting & Bulk Move

As a user,
I want to sort cards within columns and move all cards between columns,
So that I can efficiently organize and reorganize my workflow.

**Acceptance Criteria:**

**Given** I am viewing a column
**When** I click the column menu and select "Sort by Date"
**Then** I can choose ascending or descending order
**And** cards reorder immediately based on creation date

**Given** I want to move all cards from one column to another
**When** I click "Move All Cards" in the column menu
**Then** I see a dropdown of other columns in the board
**And** selecting a target column moves all cards to that column
**And** a toast confirms "X cards moved"

### Story 2.5: Board View Layout

As a user,
I want a horizontal scrolling board view,
So that I can see and interact with all my columns at a glance.

**Acceptance Criteria:**

**Given** I am viewing a board
**When** the board loads
**Then** columns are displayed horizontally with 24px gap
**And** each column has a minimum width of 320px
**And** the board scrolls horizontally when columns exceed viewport width
**And** scroll has momentum on desktop and touch support on mobile
**And** the board header (name, actions) stays fixed while scrolling
**And** "Add Column" button appears at the right edge of the scroll area

---

## Epic 3: Task Capture & Card Management

Users can quickly create, edit, and move cards — the core kanban interaction that delivers immediate productivity value.

### Story 3.1: Card Creation

As a user,
I want to quickly create cards by typing a title,
So that I can capture tasks as fast as I think of them.

**Acceptance Criteria:**

**Given** I am viewing a column
**When** I click "+ Add a card" at the bottom of the column
**Then** an inline text input appears (no modal)
**And** the input is focused immediately

**Given** I type a title and press Enter
**When** the card is created
**Then** the card appears at the bottom of the column
**And** the input stays focused for the next card
**And** a subtle slide-up animation plays on the new card

**Given** I press Escape or click away
**When** the input is empty
**Then** the input closes without creating a card

**Given** I press Tab
**When** creating cards
**Then** focus moves to the next column's "Add a card" input

### Story 3.2: Card Editing

As a user,
I want to edit a card's title,
So that I can correct or update task names.

**Acceptance Criteria:**

**Given** I am viewing a card
**When** I click on the card title
**Then** the title becomes editable inline
**And** changes are saved on Enter or blur
**And** pressing Escape cancels the edit and reverts to original

### Story 3.3: Drag-Drop Between Columns

As a user,
I want to drag cards between columns,
So that I can move tasks through my workflow stages.

**Acceptance Criteria:**

**Given** I am viewing a board with cards
**When** I hover over a card
**Then** the cursor changes to grab hand
**And** a subtle lift shadow appears (2px)

**Given** I click and drag a card
**When** the drag starts
**Then** the card lifts with elevated shadow
**And** a ghost placeholder remains in the original position
**And** the card follows the cursor smoothly (60fps)

**Given** I drag toward another column
**When** hovering over a column
**Then** the drop zone highlights with a border glow
**And** an insertion point line shows where the card will land
**And** neighboring cards shift to make room

**Given** I release the card in a valid drop zone
**When** the drop completes
**Then** the card settles with a subtle bounce animation
**And** if moved to a "Done" column, a brief checkmark animation plays

**Given** I drag to an invalid zone
**When** I release
**Then** the card snaps back to its original position with a slight shake

**Given** I press Escape during drag
**When** the drag is cancelled
**Then** the card returns to its original position

### Story 3.4: Drag-Drop Within Column

As a user,
I want to reorder cards within a column,
So that I can prioritize tasks within a workflow stage.

**Acceptance Criteria:**

**Given** I am viewing a column with multiple cards
**When** I drag a card up or down within the same column
**Then** other cards shift to show the insertion point
**And** releasing the card places it at the new position

**Given** I use keyboard navigation
**When** I focus a card and press Space to grab
**Then** I can use Arrow Up/Down to reorder within the column
**And** pressing Space again drops the card
**And** pressing Escape cancels the operation

### Story 3.5: Card Deletion

As a user,
I want to delete cards I no longer need,
So that my board stays clean and focused.

**Acceptance Criteria:**

**Given** I am viewing a card
**When** I click the card menu and select "Delete"
**Then** a confirmation dialog appears
**And** confirming deletes the card permanently
**And** a toast notification appears with "Undo" button (5 seconds)

**Given** I click Undo on the delete toast
**When** within 5 seconds
**Then** the card is restored to its original position

---

## Epic 4: Rich Card Details & Organization

Users can add depth to cards with markdown descriptions, color-coded labels, due dates, and checklists with progress tracking.

### Story 4.1: Card Detail Panel

As a user,
I want to open a detailed view of a card,
So that I can edit all card properties in one place.

**Acceptance Criteria:**

**Given** I am viewing a card on the board
**When** I click on the card
**Then** a sheet panel opens from the right side
**And** the sheet shows the card title (editable)
**And** the sheet shows sections for: Description, Labels, Due Date, Checklist
**And** the sheet is scrollable for long content
**And** clicking outside the sheet or pressing Escape closes it
**And** changes are saved automatically on blur

### Story 4.2: Markdown Description

As a user,
I want to write card descriptions in markdown with live preview,
So that I can add rich formatting to my task details.

**Acceptance Criteria:**

**Given** I am viewing the card detail panel
**When** I click on the Description section
**Then** a textarea appears for markdown input
**And** a toggle button switches between Edit and Preview modes

**Given** I am in Edit mode
**When** I type markdown
**Then** the raw markdown is saved on blur

**Given** I am in Preview mode
**When** the description has content
**Then** the markdown is rendered as formatted HTML
**And** the output is sanitized to prevent XSS attacks

**Given** I view a card with a description on the board
**When** the card renders
**Then** a 2-line preview of the description appears below the title

### Story 4.3: Color-Coded Labels

As a user,
I want to assign color-coded labels to cards,
So that I can visually categorize and scan my tasks.

**Acceptance Criteria:**

**Given** I am viewing the card detail panel
**When** I click on the Labels section
**Then** a LabelPicker appears with 6 colors: Red, Orange, Yellow, Green, Blue, Purple
**And** each color can have a custom name

**Given** I select a label
**When** the label is applied
**Then** the label appears as a pill-shaped badge on the card
**And** the badge shows the label color and name (if set)

**Given** I deselect a label
**When** the label is removed
**Then** the badge disappears from the card

**Given** I view the board
**When** cards have labels
**Then** labels are visible on the card face for quick visual scanning
**And** label colors meet WCAG AA contrast requirements in both light and dark mode

### Story 4.4: Due Dates

As a user,
I want to set due dates on cards,
So that I can track deadlines and prioritize time-sensitive tasks.

**Acceptance Criteria:**

**Given** I am viewing the card detail panel
**When** I click on the Due Date section
**Then** a date picker appears

**Given** I select a date
**When** the due date is set
**Then** the due date appears on the card as a small badge
**And** the badge shows the date in a readable format (e.g., "Mar 15")

**Given** a card has a due date in the past
**When** viewing the card
**Then** the due date badge appears in a warning color (amber/rose)

**Given** I want to remove a due date
**When** I click "Clear" in the date picker
**Then** the due date is removed from the card

### Story 4.5: Checklists with Progress

As a user,
I want to add checklists to cards with visual progress tracking,
So that I can break down tasks into subtasks and see completion status.

**Acceptance Criteria:**

**Given** I am viewing the card detail panel
**When** I click "Add Checklist"
**Then** a checklist section appears with an input for the first item

**Given** I type a checklist item and press Enter
**When** the item is added
**Then** it appears as a checkbox with label text
**And** the input stays focused for adding more items

**Given** I check/uncheck a checklist item
**When** the state changes
**Then** a progress bar updates showing completion percentage
**And** the progress bar appears on the card face on the board

**Given** a checklist is complete (100%)
**When** viewing the card
**Then** the progress bar shows full green fill with a checkmark

**Given** a checklist is partially complete
**When** viewing the card
**Then** the progress bar shows teal fill proportional to completion

**Given** I want to delete a checklist item
**When** I click the delete icon on an item
**Then** the item is removed and progress recalculates

---

### Story 4.6: Notes System

As a user,
I want to create, edit, and organize rich markdown notes that can be standalone or linked to boards, projects, or cards,
So that I can plan, brainstorm, and document alongside my task tracking.

**Acceptance Criteria:**

**Given** I am on the notes page (`/notes`)
**When** I click "New Note"
**Then** a note editor opens with a title field and markdown textarea
**And** I can type markdown content with support for headings, lists, code blocks, and mermaid diagrams

**Given** I am editing a note
**When** I toggle to preview mode
**Then** the markdown renders as formatted HTML with syntax-highlighted code blocks and rendered mermaid diagrams
**And** the output is sanitized to prevent XSS attacks

**Given** I want to link a note to a board
**When** I select a board from the link dropdown
**Then** the note is associated with that board and appears in the board's notes sidebar panel
**And** the note still appears in the global notes list with a "Board" type badge

**Given** I am viewing a board with linked notes
**When** I look at the left side of the screen
**Then** I see the notes sidebar panel showing board-linked notes
**And** the panel header shows "Notes" 
**And** each note shows: title (truncated), truncated preview (1-2 lines), tags as small pills
**And** I can click a note to open it in the note editor (modal or slide-out)
**And** I can create new notes via "New Note" button at the bottom of the panel
**And** new notes are automatically linked to this board
**And** I can edit or delete existing board-linked notes from the panel

**Given** I am viewing a board with no linked notes
**When** I look at the left side
**Then** the sidebar shows "No notes for this board" (minimal text)
**And** a "New Note" button is available to create the first board-linked note

**Given** I am on a non-board page (projects, global notes)
**When** the page loads
**Then** the left sidebar is hidden
**And** the main content takes full width

**Given** I want to hide the notes sidebar
**When** I click the collapse button (◀)
**Then** the sidebar collapses and the main board view expands
**And** the board content reflows to use the full width

**Given** I want to create a standalone note
**When** I leave all link fields empty
**Then** the note is created as a standalone note with a "General" type badge

**Given** I am on the notes page
**When** I view my notes list
**Then** I see all my notes with title, type badge, tags, and last updated date
**And** I can search notes by title
**And** I can filter notes by type (All, General, Board, Project, Card)

**Given** I want to organize notes
**When** I add tags to a note
**Then** the tags appear as colored badges on the note
**And** I can filter the notes list by clicking a tag

**Given** I want to delete a note
**When** I click delete and confirm
**Then** the note is permanently deleted
**And** a toast notification appears with "Undo" button (5 seconds)

---

### Story 4.7: Board View Toggle

As a user,
I want to switch between kanban board view and list view for a board,
So that I can use the same board for different purposes — structured workflow management or simple task tracking.

**Acceptance Criteria:**

**Given** I am viewing a board
**When** I look at the board header
**Then** I see a view toggle with two options: "Board" (kanban) and "List"
**And** the current view is visually indicated
**And** the board remembers my last selected view (persisted per board)

**Given** I am in "Board" (kanban) view
**When** the board loads
**Then** I see columns with cards that I can drag-drop between columns
**And** cards show labels, due dates, and progress bars

**Given** I switch to "List" view
**When** the view changes
**Then** I see all cards from all columns in a single flat list
**And** each row shows: title, column name (as a badge), labels, due date, checklist progress
**And** the list is sortable by: created date, updated date, due date, title
**And** I can click a card to open the detail panel (same as kanban view)
**And** I can edit card title inline (same as kanban view)

**Given** I switch back to "Board" view
**When** the view changes
**Then** I see the standard kanban columns and cards
**And** my previous board state is preserved (scroll position, filters, etc.)

**Given** I am in List view
**When** I filter or search cards
**Then** the list updates in real-time (same filter logic as kanban view)

**Given** I create a new card in List view
**When** I click "Add Card"
**Then** I can select which column to add it to
**And** the card appears in the list

**Given** I delete a card in List view
**When** I click delete
**Then** the card is removed (same behavior as kanban view)

---

## Epic 5: Search, Filter & Visual Polish

Users can find cards quickly, filter by criteria, and enjoy a polished experience with dark mode, responsive design, and delightful animations.

### Story 5.1: Card Search

As a user,
I want to search for cards by title,
So that I can quickly find specific tasks on a busy board.

**Acceptance Criteria:**

**Given** I am viewing a board
**When** I look at the board header
**Then** I see a search input field

**Given** I type in the search field
**When** I enter at least 2 characters
**Then** cards are filtered in real-time (debounced 300ms)
**And** only cards matching the search term in their title are visible
**And** non-matching cards are hidden (not removed)

**Given** I clear the search field
**When** the field is empty
**Then** all cards become visible again

**Given** no cards match my search
**When** the filter results are empty
**Then** I see "No cards found" with a "Clear search" button

### Story 5.2: Card Filtering

As a user,
I want to filter cards by labels, due date, and checklist status,
So that I can focus on specific subsets of my work.

**Acceptance Criteria:**

**Given** I am viewing a board
**When** I click the filter icon in the header
**Then** a dropdown appears with filter options: Labels, Due Date, Checklist

**Given** I open the Label filter
**When** I see the dropdown
**Then** I can toggle multiple labels on/off
**And** only cards with selected labels are visible

**Given** I open the Due Date filter
**When** I see the dropdown
**Then** I can select: Overdue, Today, This Week, No Date
**And** only cards matching the selection are visible

**Given** I open the Checklist filter
**When** I see the dropdown
**Then** I can select: All, With Checklist, Complete, Incomplete
**And** only cards matching the selection are visible

**Given** I have active filters
**When** I view the board header
**Then** active filters appear as chips below the header
**And** I can click X on a chip to clear that filter
**And** I can click "Clear all" to reset all filters

### Story 5.3: Dark Mode

As a user,
I want dark mode that respects my system preference and can be toggled manually,
So that I can use the app comfortably in any lighting condition.

**Acceptance Criteria:**

**Given** I open the app for the first time
**When** my system prefers dark mode
**Then** the app automatically applies dark theme

**Given** I want to override the system preference
**When** I click the theme toggle in the header
**Then** the app switches between light and dark mode
**And** my preference is persisted in localStorage

**Given** dark mode is active
**When** I view the app
**Then** all colors use the dark mode tokens from the design system
**And** all components render correctly in dark mode
**And** contrast ratios meet WCAG AA requirements

### Story 5.4: Responsive Layout

As a user,
I want the app to work well on desktop, tablet, and mobile,
So that I can access my boards from any device.

**Acceptance Criteria:**

**Given** I am on desktop (1024px+)
**When** I view the app
**Then** I see the full layout with horizontal scrolling columns
**And** the sidebar is collapsible

**Given** I am on tablet (640-1023px)
**When** I view the app
**Then** the sidebar auto-collapses
**And** columns still scroll horizontally
**And** touch targets are minimum 44px

**Given** I am on mobile (<640px)
**When** I view the app
**Then** I see a single column view
**And** I can swipe left/right to switch columns
**And** the sidebar becomes a bottom sheet accessible via hamburger menu
**And** cards stack vertically
**And** drag-drop uses long-press (500ms) to initiate

### Story 5.5: Loading States & Error Handling

As a user,
I want clear feedback when the app is loading or encounters errors,
So that I always know what's happening.

**Acceptance Criteria:**

**Given** a page is loading
**When** data is being fetched
**Then** skeleton screens appear matching the expected layout
**And** skeletons use gray shapes that approximate the content

**Given** I submit a form
**When** the request is processing
**Then** a spinner appears on the submit button
**And** the button text changes to "Saving..."

**Given** an operation fails
**When** an error occurs
**Then** a red toast appears with a friendly error message
**And** the toast requires manual dismiss
**And** the message is actionable (e.g., "Failed to save. Retry?")

**Given** I perform a drag-drop
**When** the move is in progress
**Then** no loading indicator appears (optimistic update)
**And** if the server request fails, a toast notifies me

### Story 5.6: Micro-Animations

As a user,
I want delightful micro-animations that make the app feel polished,
So that using KanbanFlow is a satisfying experience.

**Acceptance Criteria:**

**Given** I create a new card
**When** the card appears
**Then** it animates in with a subtle slide-up effect

**Given** I drag a card to a target column
**When** hovering over the target card position
**Then** a placeholder empty card appears to indicate where the drop will be
**And** neighboring cards move (animated) to their final positions

**Given** I drop a card in a new position
**When** the drop completes
**Then** the card settles with a subtle bounce animation
**And** neighboring cards animate to their final positions

**Given** I have `prefers-reduced-motion` enabled
**When** animations would play
**Then** animations are disabled or reduced to simple opacity changes

---

## Epic 6: Admin Panel & User Management

Admins can manage users, control registration, and monitor activity to keep the application secure and healthy.

### Story 6.1: Admin Access & Layout

As an admin,
I want a dedicated admin panel accessible only to admin users,
So that I can manage the application securely.

**Acceptance Criteria:**

**Given** I am logged in as an admin
**When** I navigate to `/admin`
**Then** I see the admin panel with a tabbed layout
**And** tabs include: Users, Settings, Activity Log

**Given** I am logged in as a regular user
**When** I try to access `/admin`
**Then** I am redirected to the home page
**And** a toast shows "Access denied. Admin only."

**Given** I am not logged in
**When** I try to access `/admin`
**Then** I am redirected to the login page

### Story 6.2: User List & Management

As an admin,
I want to view and manage all users,
So that I can maintain a healthy user base.

**Acceptance Criteria:**

**Given** I am on the Users tab
**When** the page loads
**Then** I see a searchable list of all users
**And** each user shows: email, role, status, registration date
**And** the list is filterable by status and date

**Given** I select a user
**When** I click the actions menu
**Then** I see options: Block, Suspend, Delete

**Given** I click "Block" on a user
**When** I confirm the action
**Then** the user is blocked from logging in
**And** a success toast appears
**And** the user status updates to "Blocked"

**Given** I click "Delete" on a user
**When** I confirm the action
**Then** a confirmation dialog explains "This will permanently delete all user data including boards, cards, and projects"
**And** confirming deletes the user and all their data (cascade)
**And** a success toast appears

### Story 6.3: Registration Toggle

As an admin,
I want to enable or disable new user registration,
So that I can control access to the application.

**Acceptance Criteria:**

**Given** I am on the Settings tab
**When** I view the registration toggle
**Then** I see the current state (enabled/disabled)
**And** I can click to toggle it

**Given** I disable registration
**When** the toggle is set to disabled
**Then** new users cannot access the registration page
**And** existing users are not affected
**And** a toast confirms "Registration disabled"

**Given** I enable registration
**When** the toggle is set to enabled
**Then** new users can register normally
**And** a toast confirms "Registration enabled"

### Story 6.4: Activity Log Viewer

As an admin,
I want to view activity logs,
So that I can debug issues and monitor for suspicious activity.

**Acceptance Criteria:**

**Given** I am on the Activity Log tab
**When** the page loads
**Then** I see a chronological list of system events
**And** each entry shows: timestamp, event type, user, details
**And** events include: login, logout, registration, user blocked, user deleted

**Given** I see suspicious activity
**When** I identify abnormal patterns
**Then** I can use the timestamps and user info to investigate
**And** I can take action (block/delete) directly from the user list
