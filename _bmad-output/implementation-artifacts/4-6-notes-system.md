# Story 4.6: Notes System

Status: in-progress

## Story

As a user,
I want to create, edit, and organize rich markdown notes that can be standalone or linked to boards, projects, or cards,
So that I can plan, brainstorm, and document alongside my task tracking.

## Acceptance Criteria

1. **Given** I am on the notes page (`/notes`)
   **When** I click "New Note"
   **Then** a note editor opens with a title field and markdown textarea
   **And** I can type markdown content with support for headings, lists, code blocks, and mermaid diagrams

2. **Given** I am editing a note
   **When** I toggle to preview mode
   **Then** the markdown renders as formatted HTML with syntax-highlighted code blocks and rendered mermaid diagrams
   **And** the output is sanitized to prevent XSS attacks

3. **Given** I want to link a note to a board
   **When** I select a board from the link dropdown
   **Then** the note is associated with that board and appears in the board's notes sidebar panel
   **And** the note still appears in the global notes list with a "Board" type badge

4. **Given** I am viewing a board with linked notes
   **When** I look at the left side of the screen
   **Then** I see the notes sidebar panel showing board-linked notes
   **And** the panel header shows "Notes"
   **And** each note shows: title (truncated), truncated preview (1-2 lines), tags as small pills
   **And** I can click a note to open it in the note editor (modal or slide-out)
   **And** I can create new notes via "New Note" button at the bottom of the panel
   **And** new notes are automatically linked to this board
   **And** I can edit or delete existing board-linked notes from the panel

5. **Given** I am viewing a board with no linked notes
   **When** I look at the left side
   **Then** the sidebar shows "No notes for this board" (minimal text)
   **And** a "New Note" button is available to create the first board-linked note

6. **Given** I am on a non-board page (projects, global notes)
   **When** the page loads
   **Then** the left sidebar is hidden
   **And** the main content takes full width

7. **Given** I want to hide the notes sidebar
   **When** I click the collapse button (◀)
   **Then** the sidebar collapses and the main board view expands
   **And** the board content reflows to use the full width

8. **Given** I want to create a standalone note
   **When** I leave all link fields empty
   **Then** the note is created as a standalone note with a "General" type badge

9. **Given** I am on the notes page
   **When** I view my notes list
   **Then** I see all my notes with title, type badge, tags, and last updated date
   **And** I can search notes by title
   **And** I can filter notes by type (All, General, Board, Project, Card)

10. **Given** I want to organize notes
    **When** I add tags to a note
    **Then** the tags appear as colored badges on the note
    **And** I can filter the notes list by clicking a tag

11. **Given** I want to delete a note
    **When** I click delete and confirm
    **Then** the note is permanently deleted
    **And** a toast notification appears with "Undo" button (5 seconds)

12. **Given** I want to link a note to a project or card
    **When** I select a project or card from the link dropdown
    **Then** the note is associated with that project/card
    **And** the note shows the appropriate type badge ("Project" or "Card") in the notes list

13. **Given** I am editing a note
    **When** I stop typing for 2 seconds
    **Then** the note is automatically saved
    **And** a subtle indicator shows "Saved" or "Saving..." status

## Tasks / Subtasks

- [ ] Backend: Create `notes` module structure
  - [ ] Create `backend/src/notes/notes.module.ts`
  - [ ] Create `backend/src/notes/entities/note.entity.ts`
  - [ ] Create `backend/src/notes/dto/create-note.dto.ts`
  - [ ] Create `backend/src/notes/dto/update-note.dto.ts`
  - [ ] Create `backend/src/notes/dto/list-notes.dto.ts`
  - [ ] Create `backend/src/notes/notes.service.ts`
  - [ ] Create `backend/src/notes/notes.controller.ts`
- [ ] Backend: Create `tags` module structure
  - [ ] Create `backend/src/tags/tags.module.ts`
  - [ ] Create `backend/src/tags/entities/tag.entity.ts`
  - [ ] Create `backend/src/tags/dto/create-tag.dto.ts`
  - [ ] Create `backend/src/tags/dto/update-tag.dto.ts`
  - [ ] Create `backend/src/tags/tags.service.ts`
  - [ ] Create `backend/src/tags/tags.controller.ts`
- [ ] Backend: Create database migrations
  - [ ] Create migration for `notes` table
  - [ ] Create migration for `tags` table
  - [ ] Create migration for `note_tags` join table
- [ ] Backend: Update Board entity
  - [ ] Add `@OneToMany(() => Note, (note) => note.board, { cascade: true }) notes: Note[]`
- [ ] Backend: Register modules in app.module.ts
  - [ ] Import `NotesModule` and `TagsModule`
- [ ] Backend: Implement Notes endpoints
  - [ ] `GET /api/notes` — List notes (with search, type filter, tag filter)
  - [ ] `POST /api/notes` — Create note
  - [ ] `GET /api/notes/:id` — Get note detail
  - [ ] `PATCH /api/notes/:id` — Update note (title, content, links, tags)
  - [ ] `DELETE /api/notes/:id` — Delete note
- [ ] Backend: Implement Tags endpoints
  - [ ] `GET /api/tags` — List user tags
  - [ ] `POST /api/tags` — Create tag
  - [ ] `PATCH /api/tags/:id` — Update tag (name, color)
  - [ ] `DELETE /api/tags/:id` — Delete tag
- [ ] Frontend: Install new markdown dependencies
  - [ ] `npm install rehype-highlight remark-gfm rehype-raw dompurify`
  - [ ] `npm install --save-dev @types/dompurify`
  - [ ] `npm install mermaid`
- [ ] Frontend: Create `notes` feature folder
  - [ ] Create `frontend/src/features/notes/note-list.tsx`
  - [ ] Create `frontend/src/features/notes/note-editor.tsx`
  - [ ] Create `frontend/src/features/notes/note-card.tsx`
  - [ ] Create `frontend/src/features/notes/note-detail.tsx`
  - [ ] Create `frontend/src/features/notes/create-note-dialog.tsx`
  - [ ] Create `frontend/src/features/notes/use-notes.ts`
  - [ ] Create `frontend/src/features/notes/notes.api.ts`
- [ ] Frontend: Create `tags` feature folder
  - [ ] Create `frontend/src/features/tags/tag-picker.tsx`
  - [ ] Create `frontend/src/features/tags/tag-badge.tsx`
  - [ ] Create `frontend/src/features/tags/use-tags.ts`
  - [ ] Create `frontend/src/features/tags/tags.api.ts`
- [ ] Frontend: Create MarkdownEditor component
  - [ ] Edit/Preview toggle
  - [ ] Toolbar with formatting buttons
  - [ ] Code syntax highlighting with rehype-highlight
  - [ ] Mermaid diagram rendering
  - [ ] XSS sanitization with DOMPurify
- [ ] Frontend: Add `/notes` route
  - [ ] Add route to React Router
  - [ ] Add "Notes" link to header navigation
- [ ] Frontend: Implement Board Notes Sidebar
  - [ ] Show notes panel on board pages when notes exist
  - [ ] Collapse/expand toggle
  - [ ] Create note auto-linked to board
  - [ ] Empty state for no-linked-notes
  - [ ] Minimal text on non-board pages (sidebar hidden)
- [ ] Tests: Backend unit tests for notes service
  - [ ] Test CRUD operations
  - [ ] Test search/filter by type
  - [ ] Test tag assignment
  - [ ] Test cascade delete (deleting board removes linked notes)
  - [ ] Verify persistence after each POST/PATCH/DELETE with a follow-up DB query
- [ ] Tests: Backend unit tests for tags service
  - [ ] Test CRUD operations
  - [ ] Test note-tag association
- [ ] Tests: Frontend unit tests for note components
  - [ ] NoteEditor renders correctly
  - [ ] Edit/Preview toggle works
  - [ ] Code blocks render with syntax highlighting
  - [ ] Mermaid diagram renders
  - [ ] Sanitization strips dangerous HTML
  - [ ] Delete shows confirmation and toast
- [ ] Tests: E2E test for notes CRUD operations
  - [ ] Full flow: login → create note → edit → add tags → link to board → verify on board page
  - [ ] Delete note → verify removed from notes list and board sidebar
- [ ] Frontend: Tag-click filter (AC10)
  - [ ] Add optional `onClick` prop to `TagBadge`, set cursor pointer when provided
  - [ ] Wire `onTagClick` through `NoteCard` → `NoteList` sets tagId filter
  - [ ] API already supports `?tagId=` — just needs UI wiring
- [ ] Frontend: Undo toast on delete (AC11)
  - [ ] Change delete flow: AlertDialog confirm → defer API call 5s → toast with Undo button
  - [ ] On Undo click → dismiss toast → clear timeout → note stays
  - [ ] On timeout → execute API delete → invalidate queries
- [ ] Frontend: Link dropdown in NoteEditor (AC3, AC12)
  - [ ] Add board/project/card selector to NoteEditor
  - [ ] Fetch boards, projects, cards for the current user
  - [ ] On save, send appropriate link field (board_id/project_id/card_id)
  - [ ] Show type-specific badge ("Board", "Project", "Card") based on selection
- [ ] Frontend: Auto-save behavior (AC13)
  - [ ] Implement debounced save (2 seconds after typing stops)
  - [ ] Show "Saving..." indicator during save
  - [ ] Show "Saved" indicator on successful save
  - [ ] Show error toast if save fails
  - [ ] Disable manual save button while auto-saving

## Dev Notes

### Architecture Compliance

- **New NestJS Modules:** Create `notes/` and `tags/` modules following existing patterns (boards, cards, columns, checklists)
- **Module template:**
  - `NotesModule`: `@Module({ imports: [TypeOrmModule.forFeature([Note, Tag])], controllers: [NotesController], providers: [NotesService], exports: [NotesService] })`
  - `TagsModule`: `@Module({ imports: [TypeOrmModule.forFeature([Tag])], controllers: [TagsController], providers: [TagsService], exports: [TagsService] })`
- **Registration:** Import `NotesModule` and `TagsModule` in `app.module.ts`
- **Entity Relationships:**
  - Note → Board: `@ManyToOne(() => Board)` with `onDelete: 'CASCADE'`, nullable FK, `@JoinColumn({ name: 'board_id' })`
  - Note → Project: `@ManyToOne(() => Project)` with `onDelete: 'CASCADE'`, nullable FK, `@JoinColumn({ name: 'project_id' })`
  - Note → Card: `@ManyToOne(() => Card)` with `onDelete: 'CASCADE'`, nullable FK, `@JoinColumn({ name: 'card_id' })`
  - Note ↔ Tag: `@ManyToMany(() => Tag)` with `@JoinTable({ name: 'note_tags' })`
  - Note → User: `@ManyToOne(() => User)` with `onDelete: 'CASCADE'`, `@JoinColumn({ name: 'user_id' })`
  - Board → Notes: `@OneToMany(() => Note, (note) => note.board)` with `cascade: true`
  - **Important:** All three FK relations (Board, Project, Card) MUST have `@ManyToOne` decorators for TypeORM to properly load related entities
- **Feature-based Frontend:** Create `features/notes/` and `features/tags/` following existing patterns
- **React Query:** Use mutations with optimistic updates for note operations

### New Dependencies (must install)

```bash
cd frontend
npm install rehype-highlight remark-gfm rehype-raw dompurify mermaid
npm install --save-dev @types/dompurify
```

- `rehype-highlight` — Code syntax highlighting in markdown preview
- `remark-gfm` — GitHub Flavored Markdown (tables, strikethrough, task lists, URLs)
- `rehype-raw` — Allow raw HTML in markdown (needed for mermaid)
- `dompurify` — XSS sanitization on rendered HTML
- `mermaid` — Diagram rendering (flowcharts, sequence diagrams, etc.)
- Markdown rendering stack: `react-markdown` (already installed ^10.1.0) + `remark-gfm` + `rehype-raw` + `rehype-sanitize` (already installed ^6.0.0) + `rehype-highlight`
- **Security note:** Apply DOMPurify to final HTML output only, NOT to raw markdown input. This prevents stripping valid markdown syntax while still preventing XSS.

### Migration Strategy

**Existing convention** (from `backend/src/migrations/`): All migrations are **hand-written raw SQL** via `queryRunner.query()`, NOT auto-generated. Pattern:

```typescript
import { MigrationInterface, QueryRunner } from 'typeorm';

export class CreateNotes<timestamp> implements MigrationInterface {
  name = 'CreateNotes<timestamp>';

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`CREATE TABLE \`notes\` (
      \`id\` int NOT NULL AUTO_INCREMENT,
      \`title\` varchar(255) NOT NULL,
      \`content\` text NOT NULL,
      \`board_id\` int NULL,
      \`project_id\` int NULL,
      \`card_id\` int NULL,
      \`user_id\` int NOT NULL,
      \`created_at\` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
      \`updated_at\` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
      PRIMARY KEY (\`id\`)
    ) ENGINE=InnoDB`);
    await queryRunner.query(`ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_board_id\` FOREIGN KEY (\`board_id\`) REFERENCES \`boards\`(\`id\`) ON DELETE CASCADE`);
    await queryRunner.query(`ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_project_id\` FOREIGN KEY (\`project_id\`) REFERENCES \`projects\`(\`id\`) ON DELETE CASCADE`);
    await queryRunner.query(`ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_card_id\` FOREIGN KEY (\`card_id\`) REFERENCES \`cards\`(\`id\`) ON DELETE CASCADE`);
    await queryRunner.query(`ALTER TABLE \`notes\` ADD CONSTRAINT \`FK_notes_user_id\` FOREIGN KEY (\`user_id\`) REFERENCES \`users\`(\`id\`) ON DELETE CASCADE`);
    await queryRunner.query(`CREATE INDEX \`IDX_notes_board_id\` ON \`notes\`(\`board_id\`)`);
    await queryRunner.query(`CREATE INDEX \`IDX_notes_user_id\` ON \`notes\`(\`user_id\`)`);
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`DROP INDEX \`IDX_notes_user_id\` ON \`notes\``);
    await queryRunner.query(`DROP INDEX \`IDX_notes_board_id\` ON \`notes\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_user_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_card_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_project_id\``);
    await queryRunner.query(`ALTER TABLE \`notes\` DROP FOREIGN KEY \`FK_notes_board_id\``);
    await queryRunner.query(`DROP TABLE \`notes\``);
  }
}
```

**tags table migration:**
```sql
CREATE TABLE `tags` (
  `id` int NOT NULL AUTO_INCREMENT,
  `name` varchar(50) NOT NULL,
  `color` varchar(20) DEFAULT 'teal',
  `user_id` int NOT NULL,
  `created_at` datetime(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB;
```

**note_tags join table migration:**
```sql
CREATE TABLE `note_tags` (
  `note_id` int NOT NULL,
  `tag_id` int NOT NULL,
  PRIMARY KEY (`note_id`, `tag_id`)
) ENGINE=InnoDB;
```

**Migration registration:** Import and add to both `entities` and `migrations` arrays in `backend/src/database/typeorm-registry.ts`.

### Database Schema

**notes table:**
| Column | Type | Constraints |
|--------|------|-------------|
| id | int | PK, AUTO_INCREMENT |
| title | varchar(255) | NOT NULL |
| content | text | NOT NULL (markdown) |
| board_id | int | NULL, FK → boards(id) ON DELETE CASCADE |
| project_id | int | NULL, FK → projects(id) ON DELETE CASCADE |
| card_id | int | NULL, FK → cards(id) ON DELETE CASCADE |
| user_id | int | NOT NULL, FK → users(id) ON DELETE CASCADE |
| created_at | datetime(6) | DEFAULT CURRENT_TIMESTAMP |
| updated_at | datetime(6) | DEFAULT CURRENT_TIMESTAMP ON UPDATE |

**tags table:**
| Column | Type | Constraints |
|--------|------|-------------|
| id | int | PK, AUTO_INCREMENT |
| name | varchar(50) | NOT NULL |
| color | varchar(20) | DEFAULT 'teal' |
| user_id | int | NOT NULL, FK → users(id) ON DELETE CASCADE |
| created_at | datetime(6) | DEFAULT CURRENT_TIMESTAMP |

**note_tags (join table):**
| Column | Type | Constraints |
|--------|------|-------------|
| note_id | int | PK, FK → notes(id) ON DELETE CASCADE |
| tag_id | int | PK, FK → tags(id) ON DELETE CASCADE |

### API Endpoints (Backend)

**Notes:**
- `GET /api/notes?search=&type=board|project|card|general&tagId=` — List notes
- `POST /api/notes` — Create note `{ title, content, board_id?, project_id?, card_id?, tagIds?[] }`
- `GET /api/notes/:id` — Get note with tags
- `PATCH /api/notes/:id` — Update note `{ title, content, board_id?, project_id?, card_id?, tagIds?[] }`
- `DELETE /api/notes/:id` — Delete note

**Tags:**
- `GET /api/tags` — List tags
- `POST /api/tags` — Create tag `{ name, color? }`
- `PATCH /api/tags/:id` — Update tag `{ name?, color? }`
- `DELETE /api/tags/:id` — Delete tag (removes from note_tags)

**Board-linked notes (for sidebar):**
- `GET /api/boards/:boardId/notes` — Get notes linked to a board

### Frontend Components

**Note Editor (`features/notes/note-editor.tsx`):**
```typescript
interface NoteEditorProps {
  note?: Note; // undefined = new note
  onSave: (data: CreateNoteDto) => void;
  onDelete?: () => void;
}
// Split view on large screens (editor + preview side by side)
// Tab toggle on small screens
// Toolbar: sticky top with formatting buttons (Bold, Italic, Heading, Code, List, Mermaid)
// Auto-save: debounced 2 seconds after typing stops
// Exit confirmation if unsaved changes
```

**Note List (`features/notes/note-list.tsx`):**
- Header: "Notes" title + "New Note" button (primary teal)
- Search bar: full-width, debounced, placeholder "Search notes..."
- Filter chips: "All", "General", "Board", "Project", "Card" — sticky below search
- Note cards: compact cards showing title, type badge, tag chips, last updated date
- Empty state: "No notes yet. Create your first note to start documenting."
- Loading state: Skeleton cards (3-5 lines)

**Board Notes Sidebar:**
- Visible on board pages when board has linked notes
- Header: "Notes" + collapse button
- Note list: compact cards with title (truncated), preview (1-2 lines), tags
- "New Note" button at bottom
- Empty state: "No notes for this board"
- Collapse/expand toggle

**Markdown Editor:**
- `react-markdown` with plugins: `remark-gfm`, `rehype-raw`, `rehype-highlight`, `rehype-sanitize`
- Mermaid rendering: Custom component detecting ````mermaid` code blocks, rendering with `mermaid.render(id, code)` for programmatic one-off rendering (not `mermaid.run()` which is for batch DOM processing)
- XSS sanitization: DOMPurify applied to final HTML output only (not raw markdown input)
- Edit mode: Plain textarea with monospace font
- Preview mode: Rendered HTML with proper typography

**NoteCard:**
- Surface: subtle border, hover elevation
- Title: font-medium, truncate at 1 line
- Type badge: small pill, color-coded (General = slate, Board = teal, Project = rose, Card = amber)
- Tags: horizontal scroll if overflow, small colored pills
- Date: text-xs, muted color

**TagPicker:**
- Multi-select dropdown
- Create new tag inline (type + Enter)
- Color picker for new tags (preset palette: teal, rose, amber, blue, green, purple)
- Remove tag: click X on tag chip
- Search/filter existing tags

### Shadcn Components to Use
- `Button` — Actions (New Note, Save, Delete)
- `Input` — Note title, search
- `Badge` — Type badges, tag badges
- `Dialog` — Create note dialog
- `Sheet` — Board-linked note editing
- `DropdownMenu` — Note actions (edit, delete)
- `Toast` — Notifications, undo
- `ScrollArea` — Note list scroll
- `Separator` — Visual dividers
- `Tooltip` — Toolbar button hints

### React Query Key Convention
```typescript
const noteKeys = {
  all: () => ['notes'] as const,
  list: (filters?: NoteFilters) => ['notes', 'list', filters] as const,
  detail: (id: number) => ['notes', id] as const,
  byBoard: (boardId: number) => ['notes', 'board', boardId] as const,
};

const tagKeys = {
  all: () => ['tags'] as const,
};
```

- After mutations, invalidate:
  - `noteKeys.list()` — refreshes note list
  - `noteKeys.byBoard(boardId)` — refreshes board sidebar
  - `noteKeys.detail(id)` — refreshes detail view

### Previous Story Intelligence

- Story 4.5 (Checklists w/ Progress) established patterns for:
  - Hand-written raw SQL migrations with FK/index conventions
  - TypeORM registry pattern for entity + migration registration
  - Feature-based frontend folder structure
  - React Query mutations with per-feature hook file
  - Co-located `.spec.ts` and `.test.tsx` test files
- Story 4.1 (Card Detail Panel) established:
  - Sheet-based panel structure
  - Section organization with `<Separator />` dividers
  - Auto-save on blur for inline edits
- Story 4.4 (Due Dates) established:
  - CardDetailPanel integration: `<Separator />` + `<div className="space-y-2">` + `<h3>` section pattern
  - React Query hooks with per-feature API file

### Git Intelligence

- Frontend: React 19, React Query v5, shadcn/ui (radix primitives + cva), Tailwind CSS v4, Vite 8, Vitest + @testing-library/react, Playwright E2E
- Backend: NestJS 11, TypeORM 0.3, MySQL 8, Fastify adapter, class-validator, bcryptjs
- Path aliases: `@/` → `src/` (both Vite + tsconfig)
- Existing markdown packages: `react-markdown ^10.1.0`, `rehype-sanitize ^6.0.0`
- Tests co-located with source files (`.spec.ts` / `.test.tsx`)
- E2E tests in `frontend/e2e/<feature>.spec.ts` with shared `test-utils.ts`

### Markdown Rendering Stack Configuration

```typescript
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import rehypeHighlight from 'rehype-highlight';
import DOMPurify from 'dompurify';

<Markdown
  remarkPlugins={[remarkGfm]}
  rehypePlugins={[rehypeRaw, rehypeSanitize, rehypeHighlight]}
  components={{
    code({ className, children, ...props }) {
      const match = /language-(\w+)/.exec(className || '');
      // For mermaid code blocks, render with mermaid.run()
      if (match && match[1] === 'mermaid') {
        return <MermaidDiagram code={String(children)} />;
      }
      // Return highlighted code or inline code
      return <code className={className} {...props}>{children}</code>;
    },
  }}
>
  {markdownContent}
</Markdown>
```

### Mermaid Rendering Component

Create a `MermaidDiagram` component that:
1. Uses `mermaid.render(id, code)` for programmatic rendering (returns `{ svg }`)
2. Uses React's `useId()` hook for stable diagram IDs across re-renders
3. Cleans up on unmount (removes SVG, resets mermaid state)
4. Shows a loading placeholder while rendering
5. Shows an error state if diagram fails to render
6. Sets `mermaid.initialize({ startOnLoad: false })` on mount

```typescript
// frontend/src/features/notes/mermaid-diagram.tsx
interface MermaidDiagramProps {
  code: string;
}
// Pattern: const { svg } = await mermaid.render(id, code)
// Use useId() for stable React IDs, not Date.now()
// Cleanup: container.innerHTML = '' in useEffect cleanup
```

### Testing Patterns

**Backend tests** (co-located):
- `notes.service.spec.ts` — test CRUD, search, type filter, tag association, cascade delete
- `notes.controller.spec.ts` — test route handling, param validation
- `tags.service.spec.ts` — test CRUD, duplicate prevention
- **Persistence verification:** After every POST/PATCH/DELETE, make a follow-up DB query or API call to verify state is persisted (not just returned in response)

**Frontend tests** (co-located, follow existing patterns):
- `note-editor.test.tsx` — edit/preview toggle, auto-save, exit confirmation
- `note-list.test.tsx` — render notes, search, filter chips, empty state
- `note-card.test.tsx` — renders title, type badge, tags, date
- `tag-picker.test.tsx` — select/deselect, create new, color picker
- `mermaid-diagram.test.tsx` — renders diagram, shows error on failure, cleanup on unmount
- Vitest + @testing-library/react + userEvent
- QueryClient wrapper with `retry: false`
- **Auto-save tests:**
  - `note-editor.test.tsx`: Test debounce behavior (2s delay before save)
  - `note-editor.test.tsx`: Test save indicator states (idle → saving → saved)
  - `note-editor.test.tsx`: Test error handling on save failure
- **Link selector tests:**
  - `note-link-selector.test.tsx`: Test board/project/card dropdown options
  - `note-link-selector.test.tsx`: Test type badge updates based on selection

**E2E tests** (`frontend/e2e/notes.spec.ts`):
- Follow existing patterns (e.g., `due-dates.spec.ts`):
  - `test.beforeAll` — register test user
  - Login via API, setup test data
  - UI login → navigate → interact → assert → reload → verify persistence
  - Use `monitoringTest` from `test-utils.ts`
- **Auto-save E2E:**
  - Type in editor → wait 3s → reload → verify content persisted
  - Verify "Saving..." indicator appears during save
  - Verify "Saved" indicator appears after save completes
- **Project/card linking E2E:**
  - Create note → link to project → verify "Project" badge in list
  - Create note → link to card → verify "Card" badge in list
  - Verify linked notes appear in project/card detail views (if applicable)

### Accessibility

- Note editor: ARIA labels on toolbar buttons with tooltips
- Note list: ARIA labels for search, filter chips
- Note card: `role="article"` with `aria-label` containing title and type
- Mermaid diagrams: `role="img"` with `aria-label="Diagram: {title}"`
- Sidebar collapse: `aria-expanded` on toggle, `aria-label="Toggle notes sidebar"`
- Delete confirmations: AlertDialog with clear description
- Keyboard navigation: Tab through note list, Enter to open, Escape to close editor
- Ensure focus management when opening/closing note editor

### Edge Cases

- **Deleting a board/project/card with linked notes:** CASCADE deletes handle this — notes are removed automatically
- **Loading state:** Skeleton screens for initial note list load
- **Empty state:** Descriptive empty state for new users, "No notes for this board" for sidebar
- **Error state:** Toast with "Failed to load notes. Retry?" on fetch failure
- **Optimistic updates:** Create/update note immediately in UI, rollback on error
- **Concurrent edits:** Last-write-wins (MVP — single user focus)
- **Mermaid rendering failure:** Fallback to code block display with error tooltip
- **XSS in markdown:** DOMPurify strips all dangerous HTML, scripts, event handlers
- **Large markdown content:** ScrollArea for long content, no rendering limit
- **Sidebar visibility:** Hidden on non-board pages, auto-visible when board has linked notes
- **Auto-save failure:** Show error toast, keep unsaved changes in editor, allow manual retry
- **Concurrent edits:** Last-write-wins for MVP. Future: Add optimistic locking or conflict detection
- **Title uniqueness:** Allow duplicate titles. Consider auto-appending " (N)" for UX clarity
- **Tag count limit:** No hard limit, but consider warning at 10+ tags for UX
- **Content size:** MySQL `text` type supports 64KB. Add frontend warning at 50KB+ for very large notes

## References

- [Source: epics.md#Story 4.6: Notes System]
- [Source: architecture.md#Notes & Tags Data Model — Note entity, Tag entity, note_tags join table]
- [Source: architecture.md#Board Entity — notes relation with cascade]
- [Source: architecture.md#Frontend Organization — notes/ and tags/ feature folders]
- [Source: ux-design-specification.md#Notes User Experience Design]
- [Source: ux-design-specification.md#Component Specifications — NoteCard, NoteEditor, TagPicker]
- [Source: ux-design-specification.md#Navigation Patterns — header tabs, sidebar]
- [Source: project-context.md#Notes System Rules]
- [Source: 4-5-checklists-with-progress.md — Migration patterns, registry, API conventions]

## Dev Agent Record

### Agent Model Used

DeepSeek v4 Flash

### Debug Log References

### Completion Notes List

### File List

Backend:
- `backend/src/notes/notes.module.ts` — new
- `backend/src/notes/notes.controller.ts` — new
- `backend/src/notes/notes.service.ts` — new
- `backend/src/notes/notes.service.spec.ts` — new
- `backend/src/notes/notes.controller.spec.ts` — new
- `backend/src/notes/entities/note.entity.ts` — new
- `backend/src/notes/dto/create-note.dto.ts` — new
- `backend/src/notes/dto/update-note.dto.ts` — new
- `backend/src/notes/dto/list-notes.dto.ts` — new
- `backend/src/tags/tags.module.ts` — new
- `backend/src/tags/tags.controller.ts` — new
- `backend/src/tags/tags.service.ts` — new
- `backend/src/tags/tags.service.spec.ts` — new
- `backend/src/tags/entities/tag.entity.ts` — new
- `backend/src/tags/dto/create-tag.dto.ts` — new
- `backend/src/tags/dto/update-tag.dto.ts` — new
- `backend/src/migrations/<timestamp>-CreateNotesTable.ts` — new
- `backend/src/migrations/<timestamp>-CreateTagsTable.ts` — new
- `backend/src/migrations/<timestamp>-CreateNoteTagsTable.ts` — new
- `backend/src/boards/entities/board.entity.ts` — modified (add notes relation)

Frontend:
- `frontend/src/features/notes/note-list.tsx` — new
- `frontend/src/features/notes/note-editor.tsx` — new
- `frontend/src/features/notes/note-card.tsx` — new
- `frontend/src/features/notes/note-detail.tsx` — new
- `frontend/src/features/notes/create-note-dialog.tsx` — new
- `frontend/src/features/notes/use-notes.ts` — new
- `frontend/src/features/notes/notes.api.ts` — new
- `frontend/src/features/notes/mermaid-diagram.tsx` — new
- `frontend/src/features/notes/note-list.test.tsx` — new
- `frontend/src/features/notes/note-editor.test.tsx` — new
- `frontend/src/features/notes/note-card.test.tsx` — new
- `frontend/src/features/notes/mermaid-diagram.test.tsx` — new
- `frontend/src/features/tags/tag-picker.tsx` — new
- `frontend/src/features/tags/tag-badge.tsx` — new
- `frontend/src/features/tags/use-tags.ts` — new
- `frontend/src/features/tags/tags.api.ts` — new
- `frontend/src/features/tags/tag-picker.test.tsx` — new
- `frontend/src/features/boards/board-view.tsx` — modified (add notes sidebar)
- `frontend/src/App.tsx` — modified (add /notes route, Notes nav link)
- `frontend/src/layouts/app-layout.tsx` — modified (sidebar context awareness)
- `frontend/e2e/notes.spec.ts` — new
