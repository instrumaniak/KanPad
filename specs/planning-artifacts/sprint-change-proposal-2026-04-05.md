---
date: 2026-04-05
change_trigger: "Board-First Architecture: Boards should be first-class citizens, not forced into projects"
scope_classification: "Moderate"
artifacts_modified: ["PRD", "Epics", "Architecture", "UX Design Specification", "Story 2.1"]
routed_to: "Product Owner / Scrum Master for backlog reorganization, then Development team for implementation"
---

# Sprint Change Proposal: Board-First Architecture

**Prepared by:** Bob (Scrum Master)
**Date:** 2026-04-05
**Project:** KanbanFlow
**Status:** Pending Approval

---

## Section 1: Issue Summary

### Problem Statement

The current architecture forces every board to belong to a project (`boards.project_id` NOT NULL). This creates a 2-click friction path for users: create project → create board. Trello and similar kanban tools use a board-first model where boards are top-level entities and projects (workspaces) are optional organizational containers.

### How This Was Discovered

During Party Mode multi-agent review of Story 2.1 (Board CRUD), the team identified that the project-first hierarchy contradicts the Trello-clone UX goal and creates unnecessary friction for the primary user persona (solo developers and personal productivity users).

### Evidence & Team Consensus

| Agent | Position |
|-------|----------|
| **Sally (UX)** | Board-first enables progressive complexity. Homepage should be board list, not project list. |
| **John (PM)** | Trello, Linear, Notion all use board/page-first. Projects are optional grouping. YAGNI applies. |
| **Winston (Architect)** | Make `project_id` nullable, add `user_id` as direct FK. Minimal cost, both paths supported. |
| **Mary (BA)** | 80% of users (solo devs) will never use projects. Board-first matches user data. |
| **Amelia (Dev)** | Implementation impact is low: nullable FK, API endpoint restructuring. |

---

## Section 2: Impact Analysis

### Epic Impact

| Epic | Impact | Details |
|------|--------|---------|
| **Epic 1: User Onboarding & Project Setup** | **Modified** | Onboarding flow changes from "create project first" to "create board first". Epic description updated. Stories 1.6, 1.7, 1.8 affected. |
| **Epic 2: Board Organization & Column Management** | **Modified** | Renamed to "Board & Project Organization". Story 2.1 restructured. Story 2.2 (archiving) updated. Project management becomes secondary feature. |
| **Epic 3: Task Capture & Card Management** | **No change** | Operates at card/column level, unaffected by board ownership model. |
| **Epic 4: Rich Card Details** | **No change** | Unaffected. |
| **Epic 5: Search, Filter & Visual Polish** | **No change** | Unaffected. |
| **Epic 6: Admin Panel** | **No change** | Unaffected. |

### Artifact Conflicts

| Artifact | Conflict | Severity |
|----------|----------|----------|
| **PRD** | FR9, FR10, FR13 reference project-first hierarchy. MVP scope lists projects before boards. User journeys assume project-first flow. | High |
| **Architecture** | Board entity has `project_id` NOT NULL (implied). API endpoints are nested under projects. Data boundaries define User→Projects→Boards chain. | High |
| **UX Design Spec** | Onboarding flow, empty states, navigation hierarchy all assume project-first. Board setup flow starts from project page. | High |
| **Story 2.1** | Entire story assumes boards within projects. All ACs reference project context. | High |
| **Story 1.7** | References board count on project cards — still valid but secondary. | Low |
| **Story 1.8** | Empty states reference "no projects" as primary — should become "no boards". | Medium |

### Technical Impact

- **Database:** `boards` table needs `user_id` FK added, `project_id` changed to NULLable
- **API:** Endpoints change from `/api/projects/:id/boards` to `/api/boards` (top-level)
- **Authorization:** Boards verify ownership via `user_id` directly instead of through project relationship
- **Frontend:** Homepage becomes board list, project list moves to secondary navigation
- **Migration:** Existing migration plan needs adjustment — boards table migration must include `user_id`

---

## Section 3: Recommended Approach

### Selected Path: Option 1 — Direct Adjustment

**Rationale:**
- This is a design-time change — no code has been implemented yet
- Effort is low now (document updates only), would be very high after implementation
- Board-first aligns with Trello clone goal, reduces user friction, and supports both simple and power user workflows
- `project_id` as nullable FK gives us the best of both worlds: boards can exist standalone OR be grouped into projects

**Effort Estimate:** Medium (document updates + story restructuring)
**Risk Level:** Low (no implementation work to undo)
**Timeline Impact:** None (no stories have been started)

### What Changes vs What Stays

| Changes | Stays the Same |
|---------|----------------|
| Board entity ownership model | MVP feature scope (all features remain) |
| API endpoint structure | Tech stack (React, NestJS, MySQL) |
| Onboarding user flow | Architecture patterns and conventions |
| Epic 1 & 2 descriptions | Epics 3-6 (unchanged) |
| PRD functional requirements (FR9, FR10, FR13) | Design system and UX patterns |
| Empty state hierarchy | Admin panel and user management |
| Navigation hierarchy | Migration strategy and database patterns |

---

## Section 4: Detailed Change Proposals

### 4.1 PRD Changes

#### Change 4.1.1: MVP Scope — Boards Section

**File:** `_bmad-output/planning-artifacts/prd.md`

**OLD:**
```markdown
**Projects**
- Create, view, edit projects
- Delete projects (cascades to boards)
- Project list view

**Boards**
- Create boards with background color
- Edit board name and background
- Board archiving (hidden from main view)
- Archive section (view archived, restore, permanently delete)
- Board list view per project
```

**NEW:**
```markdown
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
```

#### Change 4.1.2: Functional Requirements

**OLD:**
```markdown
### Project Management
- FR5: Users can create projects
- FR6: Users can view their project list
- FR7: Users can edit project details (name)
- FR8: Users can delete projects (cascades to boards)
- FR9: System organizes boards under projects

### Board Management
- FR10: Users can create boards within a project
- FR11: Users can customize board background color
- FR12: Users can edit board name
- FR13: Users can view boards within a project
```

**NEW:**
```markdown
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
```

#### Change 4.1.3: User Journey — The Organizer

**OLD:**
```markdown
| Step | Action | Emotional State |
|------|--------|-----------------|
| Opening | Login after a long day, brain full of things to do | Overwhelmed, scattered |
| Discovery | See project list, click "New Project" | Hopeful |
| Action | Name project "Website Redesign", pick a color | Focused |
| Building | Create board "Backlog" with columns: To Do, In Progress, Done | Creating order from chaos |
```

**NEW:**
```markdown
| Step | Action | Emotional State |
|------|--------|-----------------|
| Opening | Login after a long day, brain full of things to do | Overwhelmed, scattered |
| Discovery | See board list, click "Create Board" | Hopeful |
| Action | Name board "Website Redesign", pick a color | Focused |
| Building | Board created with default columns: To Do, In Progress, Done | Creating order from chaos |
| Capture | Quickly add 5 cards for tasks I remember | Relieved, things are being captured |
```

#### Change 4.1.4: User Journey — The Explorer

**OLD:**
```markdown
| Step | Action | Emotional State |
|------|--------|-----------------|
| Entry | Register with email, set password | No friction |
| Onboarding | Land on empty projects page, see "Create your first project" prompt | Guided but not forced |
| Experimenting | Create a board, add columns, drag cards around | Playful, impressed |
```

**NEW:**
```markdown
| Step | Action | Emotional State |
|------|--------|-----------------|
| Entry | Register with email, set password | No friction |
| Onboarding | Land on empty boards page, see "Create your first board" prompt | Guided but not forced |
| Experimenting | Create a board with default columns, drag cards around | Playful, impressed |
```

---

### 4.2 Architecture Changes

**File:** `_bmad-output/planning-artifacts/architecture.md`

#### Change 4.2.1: Board Entity

**OLD (implied from Story 2.1 Dev Notes):**
```
Board entity:
- id (PK auto-increment)
- name (string)
- background_color (string)
- user_id (FK to users) ← was questioned, should be removed
- project_id (FK to projects, NOT NULL)
- created_at, updated_at
```

**NEW:**
```
Board entity:
- id (PK auto-increment)
- name (string)
- background_color (string)
- user_id (FK to users, NOT NULL) ← direct ownership
- project_id (FK to projects, NULLable) ← optional grouping
- created_at, updated_at
```

#### Change 4.2.2: API Endpoints

**OLD:**
```
POST /api/boards                    (nested under project context)
GET /api/projects/:projectId/boards (list boards in project)
GET /api/boards/:id                 (get single board)
PATCH /api/boards/:id               (update board)
DELETE /api/boards/:id              (delete board)
```

**NEW:**
```
POST /api/boards                    (create board, optional project_id in body)
GET /api/boards                     (list all user's boards)
GET /api/boards?projectId=:id       (filter boards by project, optional)
GET /api/boards/:id                 (get single board)
PATCH /api/boards/:id               (update board name/color/project)
DELETE /api/boards/:id              (delete board, cascade to columns/cards)
```

#### Change 4.2.3: Data Boundaries

**OLD:**
```
User owns Projects → Projects own Boards → Boards own Columns → Columns own Cards
```

**NEW:**
```
User owns Boards → Boards own Columns → Columns own Cards
User optionally groups Boards into Projects ( Projects → Boards is 1:many, NULLable )
```

#### Change 4.2.4: Authorization Flow

**OLD:**
```
1. User requests boards for projectId
2. Backend verifies project.user_id === session.userId
3. If not authorized → ForbiddenException
```

**NEW:**
```
1. User requests boards (optionally filtered by projectId)
2. Backend verifies board.user_id === session.userId
3. If projectId filter provided, also verify project.user_id === session.userId
4. If not authorized → ForbiddenException
```

#### Change 4.2.5: Cascade Delete Behavior

**OLD:**
```
Delete project → cascades to boards → cascades to columns → cascades to cards
```

**NEW:**
```
Delete project → boards become ungrouped (project_id set to NULL), boards NOT deleted
Delete board → cascades to columns → cascades to cards
```

---

### 4.3 UX Design Specification Changes

**File:** `_bmad-output/planning-artifacts/ux-design-specification.md`

#### Change 4.3.1: First-Time Onboarding Flow

**OLD:**
```markdown
**Flow:**
1. User clicks "Register"
2. Minimal form: email + password only
3. Submit → auto-login, redirect to projects page
4. Empty state: illustration + "Create your first project" prompt
5. User enters project name → board created with default columns
6. Empty board: "Add your first card" with inline input pre-focused
7. User creates first card → subtle celebration animation
```

**NEW:**
```markdown
**Flow:**
1. User clicks "Register"
2. Minimal form: email + password only
3. Submit → auto-login, redirect to boards page (homepage)
4. Empty state: illustration + "Create your first board" prompt
5. User enters board name + picks color → board created with default columns
6. Empty board: "Add your first card" with inline input pre-focused
7. User creates first card → subtle celebration animation
```

#### Change 4.3.2: Navigation Patterns

**OLD:**
```markdown
**Navigation Patterns:**
- Shallow hierarchy: Project → Board → Column → Card. No deeper.
- Single sidebar — Context switches only at board level.
```

**NEW:**
```markdown
**Navigation Patterns:**
- Flat hierarchy: Board → Column → Card. Projects are optional grouping tags.
- Sidebar shows boards (primary) with optional project groupings (secondary).
- Homepage = board list. Project list accessible via sidebar section.
```

#### Change 4.3.3: Empty States

**OLD:**
```markdown
| Context | Headline | CTA |
|---------|----------|-----|
| No projects | "Start organizing" | "Create your first project" |
| No boards in project | "This project is empty" | "Create a board" |
```

**NEW:**
```markdown
| Context | Headline | CTA |
|---------|----------|-----|
| No boards | "Start organizing" | "Create your first board" |
| No projects | "No projects yet" | "Create a project" (secondary action) |
```

#### Change 4.3.4: Board Setup Flow

**OLD:**
```markdown
**Entry point:** Project page, "Create Board" button

**Flow:**
1. User clicks "Create Board"
2. Inline form: board name + background color picker
3. Board created with default columns: "To Do", "In Progress", "Done"
```

**NEW:**
```markdown
**Entry point:** Boards page (homepage), "Create Board" button

**Flow:**
1. User clicks "Create Board"
2. Modal form: board name + background color picker + optional project selector
3. Board created with default columns: "To Do", "In Progress", "Done"
4. User redirected to new board view
```

#### Change 4.3.5: Experience Principles

**ADD:**
```markdown
5. **Progressive complexity** — Start simple (just boards), reveal organization (projects) when the user is ready. Don't force structure on day one.
```

---

### 4.4 Epics & Stories Changes

**File:** `_bmad-output/planning-artifacts/epics.md`

#### Change 4.4.1: Epic 1 Description

**OLD:**
```markdown
### Epic 1: User Onboarding & Project Setup
Users can register, login, and create their first project with a board — the foundation that gets users productive immediately.
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR7, FR8, FR9, FR49
```

**NEW:**
```markdown
### Epic 1: User Onboarding & Foundation
Users can register, login, and create their first board — the foundation that gets users productive immediately. Projects are optional and introduced later.
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR7, FR8, FR49
```

#### Change 4.4.2: Epic 2 Description & Title

**OLD:**
```markdown
### Epic 2: Board Organization & Column Management
Users can manage boards within projects and structure their workflow with customizable columns.
**FRs covered:** FR10, FR11, FR12, FR13, FR14, FR15, FR16, FR17, FR18, FR19, FR20, FR21, FR22
```

**NEW:**
```markdown
### Epic 2: Board & Project Organization
Users can manage boards as first-class entities and optionally organize them into projects. Columns provide customizable workflow structure.
**FRs covered:** FR9, FR10, FR11, FR12, FR13, FR14, FR15, FR16, FR17, FR18, FR19, FR20, FR21, FR22
```

#### Change 4.4.3: Story 2.1 — Board CRUD (Full Rewrite)

**OLD:**
```markdown
### Story 2.1: Board CRUD

As a user,
I want to create, view, edit, and delete boards within my projects,
So that I can organize different workflows for different purposes.

**Acceptance Criteria:**

**Given** I am viewing a project
**When** I click "Create Board"
**Then** an inline form appears with board name and background color picker
**And** the color picker shows 8 preset colors
**And** submitting creates the board with default columns: "To Do", "In Progress", "Done"
**And** I am navigated to the new board view

**Given** I have boards in a project
**When** I view the project page
**Then** I see all boards listed with name and background color preview

**Given** I want to rename a board
**When** I click edit on the board header
**Then** I can modify the name inline
**And** changes are saved on Enter or blur

**Given** I want to change board background
**When** I click the color picker in board settings
**Then** I can select a new color
**And** the board background updates immediately

**Given** I want to delete a board
**When** I click delete and confirm
**Then** the board and all its columns/cards are deleted (cascade)
**And** a success toast appears with undo option
```

**NEW:**
```markdown
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
```

#### Change 4.4.4: Story 1.7 — Project CRUD (Update)

**OLD:**
```markdown
### Story 1.7: Project CRUD

As a user,
I want to create, view, edit, and delete projects,
So that I can organize my boards into logical groups.

**Acceptance Criteria:**
...
**And** each project shows its board count
```

**NEW:**
```markdown
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
```

#### Change 4.4.5: Story 1.8 — Empty State & Toast System (Update)

**OLD:**
```markdown
**Given** I have no projects
**When** I view the projects page
**Then** I see an empty state with illustration, "Start organizing" headline, and "Create your first project" CTA
```

**NEW:**
```markdown
**Given** I have no boards
**When** I view the boards page (homepage)
**Then** I see an empty state with illustration, "Start organizing" headline, and "Create your first board" CTA

**Given** I have no projects
**When** I view the projects list
**Then** I see a simplified empty state with "Create a project" CTA (secondary action)
```

#### Change 4.4.6: Story 2.2 — Board Archiving (Update)

**OLD:**
```markdown
**Given** I want to see archived boards
**When** I click "Archived Boards" in the project
**Then** I see a list of all archived boards for that project
```

**NEW:**
```markdown
**Given** I want to see archived boards
**When** I click "Archived Boards" in the boards page or board settings
**Then** I see a list of all my archived boards (across all projects)
```

#### Change 4.4.7: FR Coverage Map (Update)

**OLD:**
```
FR9  | Epic 1 | Boards organized under projects
FR10 | Epic 2 | Create boards
```

**NEW:**
```
FR5  | Epic 1 | Create boards (top-level)
FR6  | Epic 1 | View board list (homepage)
FR7  | Epic 1 | Board background color
FR8  | Epic 1 | Edit board name
FR9  | Epic 2 | Optionally group boards into projects
FR10 | Epic 2 | Archive boards
FR14 | Epic 1 | Create projects (optional)
FR15 | Epic 1 | View project list
FR16 | Epic 1 | Edit project details
FR17 | Epic 1 | Delete projects (boards ungrouped, not deleted)
```

---

### 4.5 Story 2.1 Implementation File Changes

**File:** `_bmad-output/implementation-artifacts/2-1-board-crud.md`

#### Change 4.5.1: Story Description & ACs

**OLD:**
```markdown
As a user,
I want to create, view, edit, and delete boards within my projects,
So that I can organize different workflows for different purposes.
```

**NEW:**
```markdown
As a user,
I want to create, view, edit, and delete boards,
So that I can organize different workflows for different purposes.
```

#### Change 4.5.2: AC#1 — Create Board

**OLD:**
```markdown
1. **Given** I am viewing a project, **When** I click "Create Board", **Then** an inline form appears with board name and background color picker
```

**NEW:**
```markdown
1. **Given** I am on the boards page, **When** I click "Create Board", **Then** a modal form appears with board name, background color picker, and optional project selector
```

#### Change 4.5.3: AC#5 — Board List

**OLD:**
```markdown
5. **Given** I have boards in a project, **When** I view the project page, **Then** I see all boards listed with name and background color preview
```

**NEW:**
```markdown
5. **Given** I have boards, **When** I view the boards page, **Then** I see all my boards listed with name, background color preview, and project label (if assigned)
```

#### Change 4.5.4: New AC — Project Assignment

**ADD after AC#9:**
```markdown
10. **Given** I want to assign a board to a project, **When** I open board settings, **Then** I can select a project or choose "No project"
11. **And** the change saves immediately
```

(Renumber existing AC#10 to AC#12, AC#11 to AC#13)

#### Change 4.5.5: Dev Notes — Access Control

**OLD:**
```markdown
Boards are scoped to projects, which are owned by users. Users can only see/access boards in their own projects. The Project entity already has a `user_id` foreign key — boards inherit access through the project relationship.

**Authorization flow:**
1. User requests boards for `projectId`
2. Backend verifies `project.user_id === session.userId`
3. If not authorized → `ForbiddenException`
```

**NEW:**
```markdown
Boards are owned directly by users via `user_id`. Users can only see/access their own boards. Optional project grouping does not affect access control.

**Authorization flow:**
1. User requests boards (optionally filtered by `projectId`)
2. Backend verifies `board.user_id === session.userId`
3. If `projectId` filter provided, also verify `project.user_id === session.userId`
4. If not authorized → `ForbiddenException`
```

#### Change 4.5.6: Dev Notes — Board Entity

**OLD:**
```markdown
Board entity must have:
- id (PK auto-increment)
- name (string)
- background_color (string - hex code)
- user_id (FK to users)
- project_id (FK to projects)
- created_at, updated_at timestamps
```

**NEW:**
```markdown
Board entity must have:
- id (PK auto-increment)
- name (string)
- background_color (string - hex code)
- user_id (FK to users, NOT NULL)
- project_id (FK to projects, NULLable)
- created_at, updated_at timestamps
```

#### Change 4.5.7: Dev Notes — API Endpoints

**OLD:**
```markdown
- `POST /api/boards` - Create board with default columns
- `GET /api/projects/:projectId/boards` - List all boards in project
- `GET /api/boards/:id` - Get single board
- `PATCH /api/boards/:id` - Update board name/color
- `DELETE /api/boards/:id` - Delete board (cascade to columns/cards)
```

**NEW:**
```markdown
- `POST /api/boards` - Create board with default columns (optional `project_id` in body)
- `GET /api/boards` - List all user's boards (optional `?projectId=` query param)
- `GET /api/boards/:id` - Get single board
- `PATCH /api/boards/:id` - Update board name/color/project
- `DELETE /api/boards/:id` - Delete board (cascade to columns/cards)
```

#### Change 4.5.8: Dev Notes — Anti-Patterns

**ADD:**
```markdown
- ❌ NEVER require a project to create a board — project_id must be NULLable
- ❌ NEVER cascade delete boards when a project is deleted — set project_id to NULL instead
```

#### Change 4.5.9: Dev Notes — Project Structure

**OLD:**
```markdown
frontend/src/features/boards/
├── board-list.tsx
├── board-card.tsx
├── create-board-form.tsx
├── edit-board-form.tsx
├── board.api.ts
└── use-boards.ts
```

**NEW:**
```markdown
frontend/src/features/boards/
├── board-list.tsx          (homepage board list)
├── board-card.tsx          (board preview card with color + project label)
├── create-board-modal.tsx  (modal with name, color, optional project)
├── edit-board-form.tsx
├── board-settings.tsx      (includes project assignment)
├── board.api.ts
└── use-boards.ts
```

#### Change 4.5.10: Dev Notes — API Response Examples

**POST /api/boards — Update request body:**
```typescript
// Request (project_id is optional)
{ name: "My Board", background_color: "#0079BF", project_id: 1 }
// or
{ name: "My Board", background_color: "#0079BF" } // no project
```

**GET /api/boards — Update response:**
```typescript
// Response (200)
{
  data: [
    { id: 1, name: "My Board", project_id: 1, background_color: "#0079BF", ... },
    { id: 2, name: "Another Board", project_id: null, background_color: "#D29034", ... }
  ]
}
```

---

## Section 5: Implementation Handoff

### Scope Classification: **Moderate**

This change requires backlog reorganization and document updates before development can proceed. No implementation work needs to be undone.

### Handoff Plan

| Recipient | Responsibility | Deliverables |
|-----------|----------------|--------------|
| **Scrum Master (Bob)** | Update all planning artifacts with approved changes | Updated PRD, Epics, Architecture, UX Spec, Story 2.1 files |
| **Product Manager (John)** | Validate that board-first MVP still meets user goals | PRD sign-off |
| **Architect (Winston)** | Confirm entity and API changes are sound | Architecture sign-off |
| **Developer (Amelia/Barry)** | Implement Story 2.1 with updated spec after approval | Implementation |

### Success Criteria

1. All planning artifacts are consistent with board-first architecture
2. Story 2.1 is ready for development with clear, unambiguous acceptance criteria
3. No references to "boards within projects" remain in any document
4. `project_id` is documented as NULLable across all artifacts
5. Epic 1 onboarding flow reflects board-first user journey

---

## Appendix: Checklist Execution Log

| Checklist Item | Status | Notes |
|----------------|--------|-------|
| 1.1 Identify triggering story | [x] Done | Story 2.1: Board CRUD |
| 1.2 Define core problem | [x] Done | Project-first hierarchy creates friction |
| 1.3 Gather evidence | [x] Done | Multi-agent consensus |
| 2.1 Evaluate current epic | [!] Action-needed | Epic 1 & 2 need restructuring |
| 2.2 Epic-level changes | [!] Action-needed | Descriptions, FR mappings, story order |
| 2.3 Review future epics | [x] Done | Epics 3-6 unaffected |
| 2.4 New epics needed | [x] Done | None |
| 2.5 Epic order change | [!] Action-needed | Epic 1 onboarding flow changes |
| 3.1 PRD conflicts | [!] Action-needed | FR9, FR10, FR13, user journeys, MVP scope |
| 3.2 Architecture conflicts | [!] Action-needed | Board entity, API endpoints, data boundaries |
| 3.3 UI/UX conflicts | [!] Action-needed | Onboarding, empty states, navigation |
| 3.4 Other artifacts | [!] Action-needed | Story files, sprint plan |
| 4.1 Direct Adjustment | [x] Viable | Low effort, low risk |
| 4.2 Rollback | [x] N/A | Nothing implemented |
| 4.3 MVP Review | [x] Viable | Scope unchanged, just reorganized |
| 4.4 Selected path | [x] Done | Option 1 - Direct Adjustment |
| 5.1 Issue summary | [x] Done | See Section 1 |
| 5.2 Epic/artifact impact | [x] Done | See Section 2 |
| 5.3 Recommended path | [x] Done | See Section 3 |
| 5.4 MVP impact & action plan | [x] Done | See Section 3 |
| 5.5 Agent handoff plan | [x] Done | See Section 5 |
| 6.1 Checklist review | [x] Done | All sections addressed |
| 6.2 Proposal accuracy | [x] Done | All changes documented with before/after |
| 6.3 User approval | [ ] Pending | Awaiting Raziur's review |
| 6.4 Update sprint-status | [ ] Pending | Post-approval |
| 6.5 Confirm next steps | [ ] Pending | Post-approval |
