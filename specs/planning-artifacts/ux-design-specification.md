---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
inputDocuments:
  - prd.md
workflowType: 'ux-design'
project_name: KanbanFlow
author: Raziur
date: 2026-03-19
---

# UX Design Specification KanbanFlow

**Author:** Raziur
**Date:** 2026-03-19

---

<!-- UX design content will be appended sequentially through collaborative workflow steps -->

## Executive Summary

### Project Vision

KanbanFlow is a self-hosted kanban board application designed for personal productivity sovereignty. It replaces commercial tools like Trello by giving users full ownership of their boards, data, and workflow — no subscriptions, no vendor lock-in. The vision centers on transforming the overwhelmed, scattered state of project management into clarity and control through an intuitive, satisfying visual experience.

### Target Users

- **The Organizer** — Primary user managing multiple projects and life aspects. Needs quick card capture, visual clarity, and a drag-drop experience that feels natural. Emotionally moves from overwhelmed to in-control.
- **The Explorer** — Curious demo visitor exploring the product. Needs clean onboarding, intuitive UI, and a "this actually works well" moment with details like dark mode and checklist progress.
- **The Admin** — Application owner managing users and settings. Needs efficient user management, security controls, and activity monitoring without complexity.

### Key Design Challenges

- **Drag-drop feel** — The core interaction must be tactile and responsive on web. The Organizer's critical moment is the first drag-drop — it must feel right.
- **Simplicity vs. power** — Exposing checklists, labels, due dates, markdown, search/filter, and sorting without overwhelming new users like The Explorer.
- **Onboarding friction** — With registration open by default, the empty state for new users must inspire action rather than create confusion.

### Design Opportunities

- **Quick task capture** — Nailing "brain dump to board in under 30 seconds" creates a tangible competitive edge over bloated productivity tools.
- **Emotional design arc** — The Organizer's journey from overwhelmed to in-control can be embedded in the UI through calm colors, clear hierarchy, and satisfying micro-interactions.
- **Self-hosted warmth** — Unlike corporate SaaS, this is personal. The design can lean into warmth and ownership rather than sterile corporate aesthetics.

## Core User Experience

### Defining Experience

The core experience of KanbanFlow is defined by a single loop: **capture → organize → track → complete**. The heartbeat of this loop is the drag-drop interaction — moving cards between columns. This is the moment where users transform scattered thoughts into organized action. Every feature in the product either enables this loop or enhances the visibility of progress within it.

### Platform Strategy

- **Web SPA** built with React/Vite — browser-native, no install required
- **Primary input:** Mouse and keyboard for desktop productivity
- **Secondary support:** Touch interactions for tablet/mobile casual use
- **Responsive layout** adapting to screen sizes without native app complexity
- **No offline requirement** — server-dependent by design (self-hosted architecture)

### Effortless Interactions

1. **Card creation** — Type title, hit enter. Brain dump mode with zero friction.
2. **Drag-drop** — Grab and go. No confirmation dialogs, no friction. Undo if needed.
3. **Progress visibility** — Checklist progress bar renders automatically on card face.
4. **Dark mode** — System preference auto-detected, manual toggle always one click away.

### Critical Success Moments

1. **First drag-drop** — The Organizer: "This feels right." This is the make-or-break moment.
2. **Checklist progress bar** — The Explorer: "Oh, this has the details I care about."
3. **Rapid card capture** — The Organizer dumps 5 tasks without stopping to think.
4. **Dark mode discovery** — An unexpected delight that signals thoughtful design.

### Experience Principles

1. **Quick capture** — Minimize friction to get thoughts out of your head and onto the board.
2. **Tactile feedback** — Interactions should feel physical, not digital. Drag, drop, done.
3. **Visual calm** — The UI reduces overwhelm, not adds to it. Clear hierarchy, breathing room.
4. **Progressive detail** — Start simple (board view), reveal complexity on demand (card details).
5. **Progressive complexity** — Start simple (just boards), reveal organization (projects) when the user is ready. Don't force structure on day one.

## Desired Emotional Response

### Primary Emotional Goals

**From overwhelmed to in control.** This is the emotional arc that defines KanbanFlow. The user arrives with a brain full of scattered thoughts and leaves feeling organized and capable. Every interaction should reinforce this transformation — turning chaos into clarity through the simple act of capturing and organizing tasks.

### Emotional Journey Mapping

| Stage | The Organizer | The Explorer |
|-------|--------------|--------------|
| Opening | Overwhelmed, scattered | Curious |
| Discovery | Hopeful | Guided |
| Action | Focused, creating order | Playful, experimenting |
| Completion | Relieved, in control | Delighted |
| Return | Satisfied | Satisfied |

### Micro-Emotions

- **Confidence** — "I know exactly what to do next." Clear hierarchy and obvious next actions.
- **Trust** — "My data is safe. It just works." Reliable persistence and error recovery.
- **Satisfaction** — "That drag felt good." Tactile interactions and smooth animations.
- **Delight** — "Dark mode! Progress bar! This is well-made." Thoughtful details that exceed expectations.
- **Relief** — "Got that thought captured before I forgot." Quick capture with minimal friction.

### Emotions to Avoid

- **Confusion** — "Where do I even start?" Prevented by empty state guidance and clear hierarchy.
- **Frustration** — "Why won't this card go where I want?" Prevented by responsive drag-drop and clear drop zones.
- **Anxiety** — "Did my changes save?" Prevented by loading states and reliable persistence.
- **Overwhelm** — "There's too much on screen." Prevented by progressive disclosure and visual calm.

### Design Implications

- **Confidence** → Clear visual hierarchy, obvious next actions, empty state guidance
- **Trust** → Loading states, reliable persistence, graceful error recovery
- **Satisfaction** → Tactile drag-drop physics, progress bars, smooth animations
- **Delight** → Dark mode, thoughtful micro-interactions, polished details
- **Relief** → Quick capture flow, minimal steps, keyboard shortcuts

### Emotional Design Principles

1. **Calm before action** — The interface should feel peaceful, not urgent. Users should feel invited, not pressured.
2. **Earned confidence** — Every successful interaction (card created, card moved, task completed) should reinforce "I've got this."
3. **Quiet excellence** — Delight comes from things working beautifully, not from flashy animations or interruptions.

## UX Pattern Analysis & Inspiration

### Inspiring Products Analysis

#### Trello — Primary Inspiration

**What it does well:** Trello nails the card-based kanban metaphor. The hierarchy is shallow (Board → List → Card), the interface is minimal, and drag-drop is the primary language of interaction. Color labels enable quick visual scanning, and board backgrounds add personality without complexity. It "just works" — users understand it in seconds.

**Key lesson:** Simple doesn't mean basic. Trello proves that a focused, well-executed interaction model beats feature richness every time.

#### Notion — Anti-Inspiration

**What to avoid:** Notion's flexibility becomes its weakness. Deep nesting, multiple ways to accomplish the same task, block-based complexity, and customization paralysis create cognitive overload. Users spend more time configuring than working.

**Key lesson:** Power without clarity is a trap. Every feature added must pass the test: "Does this make the core experience faster, or just more complex?"

### Transferable UX Patterns

**Navigation Patterns:**
- **Flat hierarchy** — Board → Column → Card. Projects are optional grouping. Users always know where they are.
- **Sidebar** — Shows boards (primary) with optional project groupings (secondary). Homepage = board list.

**Interaction Patterns:**
- **Drag-drop as primary action** — Moving cards between columns is the core interaction. No menus required.
- **Quick capture** — Type title, hit enter. No forms, no configuration.
- **Color labels at card level** — Visual scanning without opening cards.

**Visual Patterns:**
- **Content-dominant layout** — Minimal chrome, cards take center stage.
- **Board backgrounds** — Personal touch that creates ownership without complexity.
- **Progress bars on card face** — Information visible without clicks.

### Anti-Patterns to Avoid

- **Feature bloat** — Every new feature risks pulling focus from drag-drop simplicity. Resist the urge to add databases, relations, or formulas.
- **Deep nesting** — No folders within folders. No sidebar navigation that goes 5 levels deep.
- **Customization paralysis** — Limited, meaningful choices over infinite configuration.
- **Multiple paths** — One way to do each thing. Don't offer three different ways to create a card.
- **Tutorial-dependent features** — If it needs a tutorial, it doesn't ship in MVP.

### Design Inspiration Strategy

**What to Adopt:**
- Trello's card metaphor and shallow hierarchy — supports the core experience of capture → organize
- Trello's drag-drop language — aligns with tactile feedback emotional goal
- Trello's minimal chrome — supports visual calm design principle

**What to Adapt:**
- Checklists with progress bars (Trello has them, but bury them) — bring progress visibility to the card face
- Markdown descriptions (Trello uses rich text) — simpler, developer-friendly
- Dark mode (Trello doesn't have it natively) — expected modern feature

**What to Avoid:**
- Notion's feature bloat — conflicts with quick capture and visual calm goals
- Notion's navigation complexity — conflicts with "user always knows where they are"
- Notion's customization paralysis — conflicts with getting started in seconds

**The Rule:** If a feature requires a tutorial, it doesn't ship in MVP.

## Design System Foundation

### Design System Choice

**shadcn/ui + Tailwind CSS** — built on Radix UI primitives for accessibility and behavior, styled with Tailwind CSS for full visual control.

### Rationale for Selection

- **Speed** — Copy-paste component ownership. No library dependency to fight.
- **Customization** — Full code control. Modify any component to match the warm, personal aesthetic.
- **Dark mode** — Native Tailwind `dark:` variants with system preference detection.
- **Accessibility** — Radix primitives handle ARIA labels, focus management, keyboard navigation.
- **Performance** — Zero runtime CSS-in-JS. Tailwind purges unused styles.
- **Team fit** — Solo developer. Low overhead, maximum control.

### Implementation Approach

**Use shadcn for:**
- Dialogs, modals, popovers (card detail panels, settings)
- Dropdowns, selects, menus (sorting, filtering, actions)
- Form inputs, buttons, badges (labels, metadata)
- Tabs, tooltips, toasts (navigation, hints, notifications)

**Build custom:**
- Board view with horizontal column layout
- Card component with drag-drop, labels, progress bar
- Drag-drop system (dnd-kit)
- Checklist with progress visualization
- Markdown editor/preview

### Customization Strategy

- **Tailwind config** — Define brand colors, spacing scale, typography
- **CSS variables** — Theme tokens for light/dark mode switching
- **Color palette** — Warm, personal tones. Not sterile SaaS gray.
- **Component styling** — Tactile feel: subtle shadows, rounded corners, soft transitions
- **Typography** — Clean, readable. System font stack or Inter.

## Defining Core Experience

### Defining Experience

**"Drag a card from 'To Do' to 'Done' and feel your life getting organized."**

This is the heart of KanbanFlow. If the drag-drop experience feels right, everything else follows. Users don't describe kanban apps by their feature lists — they describe the *feeling* of organizing their work. The defining experience is the moment a user picks up a card and moves it to a new column, transforming chaos into order with a single gesture.

### User Mental Model

Users approach kanban boards with a physical bulletin board mental model:
- **Left column** = things I need to do (inbox, backlog)
- **Middle column(s)** = things I'm working on (in progress, review)
- **Right column** = things I'm done with (completed, shipped)

Moving a card is like picking up a sticky note and placing it elsewhere. The interaction should feel *tangible* — not like clicking a button, but like manipulating a physical object.

### Success Criteria

| Criterion | Description |
|-----------|-------------|
| **Smooth** | Card follows cursor with zero lag or jitter |
| **Clear** | Drop zone highlights obviously — user always knows where it lands |
| **Responsive** | Other cards shift to make room automatically |
| **Forgiving** | Undo available if you grab the wrong card |
| **Physical** | Feels like moving a real object, not triggering a digital event |

### Novel vs. Established Patterns

**Established pattern** — Drag-drop kanban is proven by Trello, Jira, Asana. We don't need to reinvent the wheel.

**Innovation happens in execution quality and surrounding experience:**
- Faster card capture (type + enter, no modal)
- Progress bars visible at card level (Trello buries them in card details)
- Dark mode (Trello doesn't have it natively)
- Markdown descriptions (simpler than rich text for developers)
- Board backgrounds for personal ownership

### Experience Mechanics

**1. Initiation:**
- User clicks or touches a card
- Visual feedback: card lifts 2px, subtle shadow appears beneath it
- Cursor changes to grab hand

**2. Interaction:**
- User drags toward a column
- Visual feedback: card follows cursor with slight rotation for physical feel
- Drop zones highlight with a subtle glow/outline

**3. Feedback:**
- Hovering over a drop zone: target column shows insertion point line
- Neighboring cards shift slightly to indicate "room will be made"
- If hovering over an invalid zone: card shows rejection state (slight shake)

**4. Completion:**
- User releases the card
- Card settles into position with a subtle bounce animation
- If moved to "Done" column: brief checkmark micro-animation
- Surrounding cards animate into their final positions

## Visual Design Foundation

### Color System

**Theme Direction:** Soft Teal — calm, focused, like a clear sky.

**Light Mode Tokens:**

| Token | Value | Hex |
|-------|-------|-----|
| Primary | Teal 600 | #0D9488 |
| Primary Hover | Teal 700 | #0F766E |
| Background | Cool white | #F8FAFB |
| Surface | Light slate | #F1F5F9 |
| Card | White | #FFFFFF |
| Border | Slate 200 | #E2E8F0 |
| Text Primary | Slate 900 | #0F172A |
| Text Secondary | Slate 500 | #64748B |
| Success | Emerald 500 | #10B981 |
| Warning | Amber 500 | #F59E0B |
| Error | Rose 500 | #F43F5E |

**Dark Mode Tokens:**

| Token | Value | Hex |
|-------|-------|-----|
| Background | Slate 950 | #0F172A |
| Surface | Slate 900 | #1E293B |
| Card | Slate 800 | #334155 |
| Border | Slate 700 | #475569 |
| Text Primary | Slate 50 | #F8FAFB |
| Text Secondary | Slate 400 | #94A3B8 |

**Label Colors:**

| Label | Light Mode | Dark Mode |
|-------|-----------|-----------|
| Red | Rose 100 bg / Rose 600 text | Rose 950 bg / Rose 400 text |
| Orange | Orange 100 bg / Orange 600 text | Orange 950 bg / Orange 400 text |
| Yellow | Yellow 100 bg / Yellow 600 text | Yellow 950 bg / Yellow 400 text |
| Green | Green 100 bg / Green 600 text | Green 950 bg / Green 400 text |
| Blue | Blue 100 bg / Blue 600 text | Blue 950 bg / Blue 400 text |
| Purple | Purple 100 bg / Purple 600 text | Purple 950 bg / Purple 400 text |

### Typography System

**Font Stack:** `Inter, system-ui, -apple-system, sans-serif`

**Type Scale:**

| Element | Size | Weight | Line Height |
|---------|------|--------|-------------|
| H1 (Page title) | 24px | 600 | 1.3 |
| H2 (Section) | 20px | 600 | 1.3 |
| H3 (Subsection) | 16px | 600 | 1.4 |
| Body | 14px | 400 | 1.5 |
| Small / Metadata | 12px | 400 | 1.4 |
| Card title | 14px | 500 | 1.4 |

### Spacing & Layout Foundation

**Base Unit:** 4px

**Spacing Scale:**
- 1: 4px (tight gaps)
- 2: 8px (card internal padding min)
- 3: 12px (card padding, column gap)
- 4: 16px (standard gap)
- 5: 20px
- 6: 24px (section gap)
- 8: 32px (major sections)
- 12: 48px (page margins)
- 16: 64px (hero/empty states)

**Layout Principles:**
- **Content-dominant** — Cards take center stage, chrome is minimal
- **Horizontal scroll** — Columns extend right, not stacked vertically
- **Breathing room** — White space between cards (8px), generous padding on cards (12px)
- **Fixed header** — Board name and actions stay visible while scrolling columns

### Accessibility Considerations

- **Contrast ratios** — All text meets WCAG AA (4.5:1 body, 3:1 large text)
- **Focus indicators** — Visible focus rings on all interactive elements using primary color
- **Color independence** — Labels use color + text, never color alone
- **Dark mode** — System preference detection with manual override
- **Font sizing** — Minimum 12px for metadata, 14px for body text

## Design Direction Decision

### Design Directions Explored

Four design directions were generated and explored via interactive HTML showcase:

- **A: Classic Clean** — Trello-inspired, minimal chrome, cards dominate
- **B: Compact Dense** — Power-user focused, smaller cards, more visible at once
- **C: Spacious Calm** — Larger cards with descriptions, breathing room, peaceful feel
- **D: Sidebar Navigation** — Project list sidebar, board takes full width

### Chosen Direction

**Direction C: Spacious Calm** as the base, with elements combined from other directions:

- **From A (Classic Clean):** Colored labels (Red, Orange, Yellow, Green, Blue, Purple) visible on card face
- **From D (Sidebar Navigation):** Collapsible sidebar for project/board navigation, collapsed by default
- **Color theme:** Soft Teal — calm, focused, clean

### Design Rationale

- **Spacious Calm** matches the emotional goal of "from overwhelmed to in control" — breathing room reduces cognitive load
- **Colored labels** enable quick visual scanning without opening cards — one of Trello's best patterns
- **Collapsible sidebar** enables fast project/board switching without cluttering the main view — collapsed by default preserves the spacious feel
- **Soft Teal** feels focused and modern without being cold — better fit than Warm Amber for a tool meant to reduce overwhelm

### Implementation Approach

- Cards: 20px padding, 12px border-radius, subtle shadow on rest, elevated shadow on hover
- Labels: 6-color system (Red, Orange, Yellow, Green, Blue, Purple), pill-shaped badges
- Sidebar: 240px width when expanded, 0px when collapsed, smooth 300ms transition
- Columns: 320px min-width, 24px gap, horizontal scroll
- Primary color: Teal 600 (#0D9488) for all interactive elements

## User Journey Flows

### Quick Card Capture Flow

The Organizer's primary interaction — capturing tasks as fast as they think of them.

**Entry point:** Board view, column footer "+ Add a card" button or keyboard shortcut

**Flow:**
1. User clicks "+ Add a card" at bottom of any column
2. Inline text input appears — no modal, no page change
3. User types card title and presses Enter
4. Card is created instantly, input stays focused for next card
5. User repeats — brain dump mode, zero friction
6. User clicks away or presses Escape to close input

**Key interactions:**
- Enter creates card AND keeps input open
- Tab switches focus to next column's input
- Empty title = input closes (no empty cards)
- New card animates in with subtle slide-up

**Success moment:** User captures 5 tasks in a row without stopping to think.

### Board Setup Flow

Creating the organizational structure.

**Entry point:** Boards page (homepage), "Create Board" button

**Flow:**
1. User clicks "Create Board"
2. Modal form: board name + background color picker + optional project selector
3. Board created with default columns: "To Do", "In Progress", "Done"
4. User redirected to new board view
5. User can immediately start adding cards
6. User adds columns via "+ Add column" at right edge
7. Columns are editable: rename, reorder, delete via column menu

**Key interactions:**
- Background color picker is simple — 8 preset colors, no custom hex input
- Default columns get user started immediately (no blank board)
- Column operations via three-dot menu on column header

**Success moment:** Board is ready to use in under 30 seconds.

### Drag-Drop Card Movement Flow

The defining experience — moving cards between columns.

**Entry point:** Any card on the board

**Flow:**
1. User hovers over card → cursor changes to grab, subtle lift shadow (2px)
2. User clicks and drags → card lifts with shadow, follows cursor smoothly
3. User moves toward a column → drop zones highlight with border glow
4. User hovers over insertion point → neighboring cards shift to make room
5. User releases → card settles into position with subtle bounce animation
6. If moved to "Done" column → brief checkmark micro-animation

**Error recovery:**
- Drag to invalid zone → card snaps back to original position with slight shake
- Browser loses focus during drag → card returns to original position
- Undo available via toast notification (3-second window) or Ctrl+Z

**Key interactions:**
- 60fps smooth tracking — no lag, no jitter
- Ghost placeholder shows where card was
- Touch support: long-press to initiate drag on mobile

**Success moment:** "This feels right." The Organizer's first drag-drop.

### First-Time Onboarding Flow

The Explorer's entry — from curious click to productive use.

**Entry point:** Landing page, "Register" button

**Flow:**
1. User clicks "Register"
2. Minimal form: email + password only (no name, no preferences)
3. Submit → auto-login, redirect to boards page (homepage)
4. Empty state: illustration + "Create your first board" prompt
5. User enters board name + picks color → board created with default columns
6. Empty board: "Add your first card" with inline input pre-focused
7. User creates first card → subtle celebration animation

**Key interactions:**
- Registration open by default (admin can disable)
- No email verification required for MVP
- Empty state is inviting, not intimidating
- First card creation triggers delight moment

**Success moment:** "This actually works well." The Explorer's first card.

### Admin User Management Flow

The Admin managing users and security.

**Entry point:** Admin navigation, /admin route

**Flow:**
1. Admin navigates to /admin (restricted to admin role)
2. Dashboard shows: user list, registration toggle, activity log (tabs)
3. User list: searchable, filterable by status and date
4. Admin selects user → action menu: Block, Suspend, Delete
5. Confirmation dialog explains consequences (e.g., "Delete will remove all user data")
6. Action executed → status updated, activity logged

**Key interactions:**
- Registration toggle is prominent — one click to enable/disable
- User deletion is cascade (all boards, cards, data removed)
- Activity log shows timestamped entries for debugging
- Destructive actions require confirmation

**Success moment:** "Chaos contained." Quick action to block abusive user.

### Journey Patterns

**Navigation Patterns:**
- Flat hierarchy: Board → Column → Card. Projects are optional grouping (never deeper)
- Sidebar for project/board switching (collapsed by default)
- Breadcrumbs for context: Project > Board name in header

**Decision Patterns:**
- Inline forms over modals (less disruptive)
- Confirmation only for destructive actions
- Smart defaults reduce decisions (default columns, auto-focus)

**Feedback Patterns:**
- Optimistic updates (card moves immediately, syncs in background)
- Toast notifications for async actions (save, delete, error)
- Micro-animations for delight (card settle, checkmark, celebration)

### Flow Optimization Principles

1. **Minimize steps to value** — Every flow should reach its goal in the fewest clicks possible
2. **Reduce cognitive load** — One decision at a time, smart defaults, clear labels
3. **Provide clear feedback** — User always knows what happened and what's next
4. **Create delight moments** — Small animations and surprises that exceed expectations
5. **Graceful error recovery** — Every error path has a clear way back

## Component Strategy

### Design System Components (shadcn/ui)

**Use directly — no customization needed:**

| Component | Used For |
|-----------|----------|
| Dialog / Modal | Card detail panel, confirmation dialogs |
| Sheet | Sidebar panels, mobile card detail |
| Dropdown Menu | Column actions, card actions, sorting |
| Button | All CTAs and actions |
| Input | Forms, inline card creation |
| Textarea | Card description editing |
| Badge | Labels, status indicators |
| Tabs | Admin panel navigation, settings |
| Toast | Notifications, undo actions |
| Tooltip | Hints, keyboard shortcuts |
| Switch | Dark mode toggle, admin settings |
| ScrollArea | Column card lists |
| Separator | Visual dividers |

### Custom Components

**Board**
- **Purpose:** Horizontal scroll container holding columns
- **Anatomy:** Scrollable row of Column components + "Add Column" button
- **States:** Loading skeleton, empty (no columns), populated
- **Behavior:** Horizontal scroll with momentum, columns snap to edges
- **Accessibility:** Arrow keys navigate between columns

**Column**
- **Purpose:** Vertical list of cards with header and controls
- **Anatomy:** Header (title + count + menu) → Card list → "+ Add a card" button
- **States:** Empty (placeholder text), populated, drag-over (highlight border)
- **Interactions:** Inline rename, sort by date, delete, move all cards to another column
- **Accessibility:** ARIA label for column name, keyboard navigation within list

**Card**
- **Purpose:** Individual task unit — the primary draggable element
- **Anatomy:** Title + description preview (2 lines) + labels + progress bar + due date
- **States:** Rest (shadow-sm), hover (shadow-md, lift 2px), dragging (shadow-lg, rotated), done (opacity 0.65)
- **Interactions:** Click → open detail panel. Drag → move between columns/reorder.
- **Accessibility:** ARIA label with title + status, keyboard grab (Space/Enter) and arrow move

**Checklist**
- **Purpose:** Track subtasks within a card with visual progress
- **Anatomy:** Checkbox + label text + progress bar (shows % on card face)
- **States:** Empty, partial (teal fill), complete (full green fill + checkmark)
- **Interactions:** Toggle items, add new item, delete, reorder via drag

**LabelPicker**
- **Purpose:** Assign color-coded labels to cards
- **Anatomy:** Grid of color swatches with label names
- **States:** No labels selected, some selected (highlighted), creating new label
- **Interactions:** Toggle label on/off, create custom label names

**MarkdownEditor**
- **Purpose:** Rich card descriptions with preview
- **Anatomy:** Textarea (edit mode) / Rendered preview / Toggle button
- **States:** Edit, preview, split (desktop only)
- **Interactions:** Type markdown, toggle preview, auto-save on blur

**EmptyState**
- **Purpose:** Guide users when no content exists — inviting, not intimidating
- **Anatomy:** Illustration + headline + description + CTA button
- **Variants:** No projects, no boards, no cards, no search results, no filter matches

### Component Implementation Strategy

- Build custom components using shadcn tokens (colors, spacing, typography from our foundation)
- All custom components use Tailwind CSS classes following our design tokens
- Drag-drop uses dnd-kit library (accessible, performant, touch-supportive)
- Checklist progress is computed client-side and displayed on Card component
- All components support dark mode via CSS variables

### Implementation Roadmap

**Phase 1 — Core (needed for basic board view):**
- Board container with horizontal scroll
- Column with header and card list
- Card with title and drag-drop
- Drag-drop system (dnd-kit)

**Phase 2 — Features (needed for full card experience):**
- Checklist with progress bar
- Label system with LabelPicker
- MarkdownEditor for descriptions
- EmptyState components

**Phase 3 — Polish (enhances experience):**
- Card detail modal/sheet
- Search and filter components
- Admin panel components
- Toast notifications for undo

## UX Consistency Patterns

### Button Hierarchy

| Type | Use | Style |
|------|-----|-------|
| **Primary** | Main CTA (Add Card, Create Board) | Teal 600 fill, white text, 6px radius |
| **Secondary** | Supporting actions (Filter, Search) | 1px border, transparent bg, hover: surface |
| **Ghost** | Tertiary actions (Cancel, Close) | No border, text only, hover: surface |
| **Destructive** | Dangerous actions (Delete, Remove) | Rose 500 fill or border, confirmation required |

**Rule:** One primary button visible at a time. Never two primaries next to each other.

### Feedback Patterns

| Type | Visual | Duration | Example |
|------|--------|----------|---------|
| **Success** | Green toast, bottom-right | 3s auto-dismiss | "Card created" |
| **Error** | Red toast, bottom-right | Manual dismiss | "Failed to save. Retry?" |
| **Warning** | Amber toast, bottom-right | 5s auto-dismiss | "Unsaved changes" |
| **Info** | Neutral toast, bottom-right | 3s auto-dismiss | "3 cards moved" |
| **Undo** | Toast with "Undo" button | 5s auto-dismiss | "Card deleted. Undo?" |

**Rules:**
- Toasts never block interaction
- Undo available for all destructive actions for 5 seconds
- Error toasts require manual dismiss (user must acknowledge)
- Success toasts auto-dismiss quickly (don't linger)

### Form Patterns

| Pattern | Use | Behavior |
|---------|-----|----------|
| **Inline** | Card creation, column rename, quick edits | No modal, edit in place, Enter to save, Escape to cancel |
| **Modal** | Board creation, confirmations, settings | Small dialog, focused task, click outside to dismiss |
| **Sheet** | Card detail editing, full content editing | Side panel (right), full detail view, scrollable |

**Validation Rules:**
- Inline errors appear below field on blur (not on keystroke)
- Red border on invalid fields
- Disable submit until required fields valid
- Never clear form on error — preserve user input
- Show error count if multiple fields invalid

### Navigation Patterns

**Header Navigation:**
- Top-level tabs in header: **Projects**, **Boards**, **Notes**
- Active tab indicated with underline or filled background
- Tabs navigate to respective pages: `/projects`, `/`, `/notes`
- On mobile: tabs collapse to hamburger menu

**Sidebar (Contextual):**
- Collapsible left sidebar (240px)
- **On board pages:** Shows board-linked notes panel (when notes exist)
- **On non-board pages:** Hidden (projects/boards/notes have their own full-page layouts)
- Toggle via header hamburger button (≡) or auto-show when board has linked notes

**Breadcrumbs:**
- Project > Board shown in header
- Clickable to navigate up hierarchy
- Hidden on top-level pages (projects, boards list, notes)

**Rules:**
- Flat hierarchy: Board → Column → Card. Projects are optional grouping (never deeper)
- Header tabs are primary navigation for top-level pages
- Sidebar is contextual — only appears when relevant (board-linked notes)
- Breadcrumbs provide context, not primary navigation
- Current location always clear via sidebar highlight + breadcrumb

### Empty States

| Context | Headline | CTA |
|---------|----------|-----|
| No boards | "Start organizing" | "Create your first board" |
| No projects | "No projects yet" | "Create a project" (secondary action) |
| No cards on board | "Add your first task" | Inline input pre-focused |
| No search results | "No cards found" | "Clear search" |
| No filter matches | "No cards match your filters" | "Clear all filters" |

**Rules:**
- Every empty state has: illustration + headline + ONE clear CTA
- Empty states are inviting, not intimidating
- CTAs are contextual (not generic "Get started")
- Illustrations are simple, not distracting

### Loading States

| Context | Pattern |
|---------|---------|
| Initial page load | Skeleton screens (gray shapes matching expected layout) |
| Card/board creation | Optimistic: show immediately, sync in background |
| Saving changes | Spinner on save button, "Saving..." text |
| Drag-drop | No loading indicator — optimistic update, toast on failure |
| Search | Debounced (300ms), no spinner, results update in place |

**Rules:**
- Skeleton screens for initial load (match expected layout)
- Optimistic updates for user actions (feels instant)
- Spinners only for unavoidable waits (network-dependent operations)
- Never show loading for drag-drop (too fast, would feel janky)

### Search & Filter

| Pattern | Behavior |
|---------|----------|
| **Search** | Inline input in board header. Filters cards by title as you type. Debounced 300ms. |
| **Label filter** | Filter icon → dropdown with label toggles (multi-select) |
| **Due date filter** | Filter icon → dropdown: Overdue / Today / This Week / No date |
| **Checklist filter** | Filter icon → dropdown: All / With checklist / Complete / Incomplete |
| **Active filters** | Shown as chips below board header. X to clear individual. "Clear all" to reset. |

**Rules:**
- Search is always visible in board header
- Filters are secondary (hidden behind filter icon) to reduce chrome
- Active filters are visible as chips (user always knows what's filtered)
- "Clear all" resets all filters at once

## Responsive Design & Accessibility

### Responsive Strategy

**Desktop-first approach** — KanbanFlow is primarily a productivity tool used on desktop. Mobile supports quick capture and viewing, not full board management.

| Device | Width | Experience |
|--------|-------|------------|
| **Desktop** | 1024px+ | Full layout. Horizontal scroll columns. Collapsible sidebar. All features. |
| **Tablet** | 640-1023px | Sidebar auto-collapses. Columns still horizontal scroll. Touch-optimized targets (44px min). |
| **Mobile** | < 640px | Single column view. Swipe between columns. Sidebar becomes bottom sheet. Cards stack vertically. |

**Mobile-specific behaviors:**
- Single column at a time, swipe left/right to switch columns
- "Add card" button fixed at bottom
- Sidebar accessible via hamburger menu → bottom sheet
- Drag-drop uses long-press (500ms) to initiate

### Breakpoint Strategy

**Tailwind CSS breakpoints:**

| Token | Width | Use |
|-------|-------|-----|
| `sm:` | 640px | Small tablets, large phones |
| `md:` | 768px | Tablets |
| `lg:` | 1024px | Small desktops, laptops |
| `xl:` | 1280px | Large desktops |

**Layout changes at breakpoints:**
- `< 640px`: Single column, bottom navigation
- `640px`: Columns visible, sidebar collapsed
- `1024px`: Full layout, sidebar toggle available
- `1280px`: Maximum content width, generous spacing

### Accessibility Strategy

**Target: WCAG AA compliance**

| Requirement | Implementation |
|-------------|----------------|
| **Color contrast** | All text meets 4.5:1 ratio. Large text 3:1. Tested with contrast checker. |
| **Keyboard navigation** | Full keyboard support. Tab through all interactive elements. Arrow keys within lists. |
| **Screen reader** | Semantic HTML (nav, main, article, section). ARIA labels on cards, columns, buttons. |
| **Focus indicators** | 2px teal outline, 2px offset. Visible on all interactive elements. |
| **Touch targets** | Minimum 44x44px for all interactive elements. |
| **Color independence** | Labels always show text + color. Status indicated by icon + text, never color alone. |
| **Skip links** | "Skip to main content" link at top of page (hidden until focused). |
| **Motion** | Respect `prefers-reduced-motion` media query. Disable animations if user preference set. |

### Drag-Drop Accessibility

| Input Method | Behavior |
|--------------|----------|
| **Mouse** | Click and drag. Cursor changes to grab. |
| **Keyboard** | Focus card → Space to grab → Arrow keys to move → Space to drop → Escape to cancel |
| **Touch** | Long-press (500ms) to grab → drag to move → release to drop |
| **Screen reader** | "Card title. Draggable. Press Space to grab." / On drop zone: "Column name. Drop zone. Position 3 of 5." |

**Keyboard shortcuts:**
- `Space` — Grab/drop card
- `Arrow Left/Right` — Move card between columns
- `Arrow Up/Down` — Reorder card within column
- `Escape` — Cancel drag operation
- `Tab` — Navigate between cards

### Testing Strategy

**Responsive testing:**
- Chrome DevTools device simulation
- Real device testing (iPhone, iPad, Android)
- Browser testing: Chrome, Firefox, Safari, Edge

**Accessibility testing:**
- axe-core automated testing (browser extension)
- Lighthouse accessibility audit
- Keyboard-only navigation testing
- Screen reader testing: VoiceOver (macOS), NVDA (Windows)
- Color blindness simulation (Chrome DevTools)

### Implementation Guidelines

**Responsive development:**
- Use Tailwind responsive prefixes (`sm:`, `md:`, `lg:`)
- Relative units (rem, %) over fixed pixels
- Flexible layouts with CSS Grid and Flexbox
- Touch-friendly targets on mobile (44px minimum)

**Accessibility development:**
- Semantic HTML elements first, ARIA second
- Test with keyboard only (unplug mouse)
- Run axe-core on every page
- Ensure focus is managed correctly on dynamic content (modals, toasts)
- Announce dynamic changes to screen readers via ARIA live regions

## Notes User Experience Design

### Notes Page (`/notes`)

**Layout:** Full-page layout following the app shell (sidebar + main content). Main content is a two-pane layout: note list on the left (40%), note editor on the right (60%).

**Note List (Left Pane):**
- Header: "Notes" title + "New Note" button (primary teal)
- Search bar: full-width, debounced, placeholder "Search notes..."
- Filter chips: "All", "General", "Board", "Project", "Card" — sticky below search
- Note cards: compact cards showing title (truncated), type badge, tag chips, last updated date
- Empty state: "No notes yet. Create your first note to start documenting." with illustration
- Loading state: Skeleton cards (3–5 lines)

**Note Editor (Right Pane):**
- Title input: large, borderless, placeholder "Note title..."
- Toolbar: bold, italic, heading, code block, quote, list, mermaid diagram toggle
- Editor area: markdown textarea (edit mode) or rendered preview (preview mode)
- Toggle: "Edit" / "Preview" tab switch
- Link section: dropdowns to link to Board/Project/Card (optional)
- Tags: tag picker with autocomplete, color-coded tags
- Actions: Save (primary), Delete (danger, with confirmation)
- Auto-save indicator: "Saved" / "Saving..." / "Unsaved changes"

### Board View Toggle (Kanban ↔ List)

**Trigger:** Segmented control in board header

**Layout:** Board header contains a segmented toggle with two options: "Board" (kanban) and "List".
- "Board" (default): Shows columns with cards, drag-drop enabled
- "List": Shows flat list of all cards across all columns
- Current view is visually indicated with filled background
- View preference is persisted per board (`boards.view_mode`)

**Kanban View:**
- Horizontal scrolling columns
- Cards drag-drop between columns and within columns
- Column headers with name, count, menu
- "Add Column" button at right edge

**List View:**
- Single flat list, vertical scroll
- Each row: card title, column badge, labels, due date, checklist progress
- Sortable by: created date, updated date, due date, title (click column header)
- Search and filters work identically to kanban view
- Click card opens detail panel (same as kanban)
- Inline title editing (same as kanban)
- "Add Card" button opens dialog to select target column

**View Transition:**
- Smooth fade transition between views (300ms)
- Previous view state preserved: scroll position, filters, search
- No data refetch — same query, different rendering

### Board Notes Panel (Contextual)

**Trigger:** Left sidebar panel automatically visible when viewing a board with linked notes.

**Layout:** Left sidebar (240px) repurposed as board-linked notes panel. Only visible on board pages when board has linked notes or user has permission to add notes.
- Header: "Notes" label
- Note list: compact cards with title (truncated), truncated preview (1-2 lines), tags
- Click note: opens in note editor (modal or slide-out panel)
- "New Note" button at bottom: opens editor with `board_id` pre-filled

**Note List:**
- Compact vertical list
- Each note: title (truncated), truncated preview (1-2 lines), tags as small pills
- Empty state: "No notes for this board" (minimal, no CTA — user creates from global notes page)
- Click note: opens in note editor

**Board-Linked Note Creation:**
- "New Note" button in sidebar panel
- Opens editor with `board_id` pre-filled
- Note appears in sidebar immediately (optimistic UI)
- Note also appears in global `/notes` list with "Board" type badge

**Sidebar Visibility:**
- Hidden on non-board pages (projects, global notes, settings)
- Hidden on board pages with no linked notes AND no create permission
- Auto-visible when board has linked notes
- User can manually collapse via toggle button

### Card Notes Panel (Contextual)

**Trigger:** Card detail panel "Notes" tab

**Layout:** Tab within card detail panel.
- Note list: compact list of notes linked to this card
- "Add Note" button: opens inline editor or dialog
- Inline note: title + truncated content + tags

### Markdown Rendering UX

**Edit Mode:**
- Plain textarea with monospace font
- Toolbar buttons insert markdown syntax at cursor
- Live character count (optional)
- Keyboard shortcuts: Ctrl+B (bold), Ctrl+I (italic), Ctrl+K (code)

**Preview Mode:**
- Rendered HTML with proper typography
- Code blocks: syntax-highlighted with dark background
- Mermaid diagrams: rendered as SVG, centered, with "Copy as Image" option
- Tables: styled with borders, alternating row colors
- Links: underlined, open in new tab
- Images: max-width 100%, click to expand

**Safety:**
- All rendered HTML is sanitized (DOMPurify or equivalent)
- No script execution in preview
- Raw HTML in markdown is escaped or stripped

### Component Specifications

**NoteCard:**
- Surface: subtle border, hover elevation
- Title: font-medium, truncate at 1 line
- Type badge: small pill, color-coded (General = slate, Board = teal, Project = rose, Card = amber)
- Tags: horizontal scroll if overflow, small colored pills
- Date: text-xs, muted color
- Click: opens note in editor

**NoteEditor:**
- Split view on large screens (editor + preview side by side)
- Tab toggle on small screens
- Toolbar: sticky top, icon buttons with tooltips
- Auto-save: debounced 2 seconds after typing stops
- Exit confirmation if unsaved changes

**TagPicker:**
- Multi-select dropdown
- Create new tag inline (type + Enter)
- Color picker for new tags (preset palette)
- Remove tag: click X on tag chip
- Search/filter existing tags
