# Story 4.2: Markdown Description

Status: done

---

## Story

As a user,
I want to write card descriptions in markdown with live preview,
so that I can add rich formatting to my task details.

---

## Acceptance Criteria

1. **Given** I am viewing the card detail panel
   **When** I look at the Description section
   **Then** I see an "Edit" / "Preview" toggle (tab switch)
   **And** Edit mode shows a textarea for raw markdown input
   **And** Preview mode renders the markdown as formatted HTML
   **And** the rendered output is sanitized to prevent XSS attacks

2. **Given** I am in Edit mode
   **When** I type markdown syntax (headings, lists, bold, italic, links, code)
   **Then** the raw markdown is saved on blur via `PATCH /api/cards/:id`
   **And** the card face on the board updates to show the new description preview

3. **Given** I switch to Preview mode
   **When** the description has markdown content
   **Then** the markdown renders as formatted HTML with proper typography
   **And** HTML tags embedded in markdown are escaped or stripped
   **And** scripts are not executable (sanitized output)

4. **Given** I view a card with a description on the board
   **When** the card renders
   **Then** a 2-line preview of the description appears below the title
   **And** the preview shows plain text (not rendered HTML) truncated with ellipsis
   **And** if the description is empty or null, no preview line is shown

5. **Given** the description contains unsafe HTML (e.g., `<script>`, `javascript:` URLs)
   **When** Preview mode renders the markdown
   **Then** the unsafe content is removed or neutralized
   **And** no XSS vulnerability is exposed

6. **Given** I am editing the description and click outside the detail panel or press Escape
   **When** the panel closes
   **Then** any unsaved changes are auto-saved on blur (same behavior as Story 4.1)

---

## Tasks / Subtasks

- [x] Install markdown rendering dependencies (`react-markdown`, `rehype-sanitize`) in `frontend/`
- [x] Add shadcn `Tabs` component to `frontend/src/components/ui/tabs.tsx`
- [x] Update `CardDetailPanel` description section:
  - [x] Replace plain `Textarea` with Edit/Preview toggle using shadcn `Tabs`
  - [x] Edit tab: keep `Textarea` for raw markdown input
  - [x] Preview tab: render markdown with `react-markdown` + `rehype-sanitize`
  - [x] Preserve auto-save on blur behavior (save raw markdown text)
  - [x] Preserve "Saving..." indicator and error handling
- [x] Update `Card` component face:
  - [x] Add 2-line description preview below title when `card.description` exists
  - [x] Preview is plain text, truncated with ellipsis, max 2 lines
  - [x] No preview rendered if description is null/empty/whitespace-only
  - [x] Ensure preview does not break card layout or drag-drop
- [x] Accessibility:
  - [x] Tabs have `aria-label="Description mode"`
  - [x] Preview content is readable by screen readers
- [x] Tests:
  - [x] Frontend tests for `CardDetailPanel` markdown toggle and rendering
  - [x] Frontend tests for `Card` description preview rendering
  - [x] Frontend tests for XSS sanitization in preview mode
  - [x] Verify all existing tests still pass

---

## Dev Notes

### Architecture Compliance

- **No backend changes required.** The `description` field (TEXT, nullable) on the `Card` entity was added in Story 4.1. The `PATCH /api/cards/:id` endpoint already accepts and persists `description`. DTOs already have `@IsOptional()` `@IsString()` `@MaxLength(10000)`.
- **Frontend feature-based structure:** All changes stay within `frontend/src/features/cards/`.
- **React Query:** Continue using `useUpdateCard()` hook. Auto-save on blur mutates `{ description: newValue || null }`. Invalidate `['cards', columnId]` on success.
- **shadcn/ui:** Use `Tabs` for the Edit/Preview toggle. Keep existing `Textarea` for edit mode. Keep `ScrollArea`, `Separator` for layout.
- **XSS Prevention (NFR9):** Use `rehype-sanitize` as a `react-markdown` plugin. This sanitizes at the AST level before HTML generation. Do NOT use raw `dangerouslySetInnerHTML`.
- **`rehype-sanitize` + `components` prop interaction:** `rehypeSanitize` runs during the rehype/unified AST phase, BEFORE the `components` prop mapping. This means `target="_blank"` and `rel="noopener noreferrer"` added via the `components` prop on the `<a>` element are safe and will NOT be stripped by `rehype-sanitize`. The `components` prop operates on React elements after sanitization is complete.

### Library Installation

```bash
cd frontend
npm install react-markdown rehype-sanitize
```

**Why these libraries:**
- `react-markdown` — renders markdown as React components. No `dangerouslySetInnerHTML`. Pluggable architecture.
- `rehype-sanitize` — sanitizes HTML in the rehype/unified AST. Prevents XSS by stripping unsafe tags/attributes before they become DOM nodes. Satisfies NFR9.

**Do NOT install:** `dompurify` (the architecture mentions it, but `rehype-sanitize` is the correct companion for `react-markdown` and works at the AST level without needing a DOM environment).

### shadcn Component to Add

```bash
cd frontend
npx shadcn@latest add tabs
```

This creates `frontend/src/components/ui/tabs.tsx`. Use `<Tabs>`, `<TabsList>`, `<TabsTrigger>`, `<TabsContent>`.

### CardDetailPanel Description Section — Target Structure

Replace the current Description `<div>` (lines 183–197 in current `card-detail-panel.tsx`) with:

```tsx
<div className="space-y-2">
  <div className="flex items-center justify-between">
    <h3 className="text-sm font-medium">Description</h3>
    {/* Edit/Preview Toggle */}
    <Tabs
      value={descriptionMode}
      onValueChange={(v) => setDescriptionMode(v as 'edit' | 'preview')}
    >
      <TabsList aria-label="Description mode">
        <TabsTrigger value="edit">Edit</TabsTrigger>
        <TabsTrigger value="preview">Preview</TabsTrigger>
      </TabsList>
    </Tabs>
  </div>

  <Tabs value={descriptionMode}>
    <TabsContent value="edit">
      <Textarea
        value={description}
        onChange={handleDescriptionChange}
        onBlur={handleDescriptionBlur}
        placeholder="Add a more detailed description..."
        aria-label="Card description"
        className="min-h-[200px] resize-y font-mono text-sm"
        disabled={isSavingDescription}
      />
    </TabsContent>
    <TabsContent value="preview">
      <div className="min-h-[200px] rounded-md border border-input bg-background p-3 prose prose-sm dark:prose-invert overflow-auto">
        {description.trim() ? (
          <ReactMarkdown rehypePlugins={[rehypeSanitize]}>
            {description}
          </ReactMarkdown>
        ) : (
          <p className="text-muted-foreground text-sm italic">
            Nothing to preview
          </p>
        )}
      </div>
    </TabsContent>
  </Tabs>

  {isSavingDescription && (
    <span className="text-xs text-muted-foreground">Saving...</span>
  )}
</div>
```

**Key implementation details:**
- `descriptionMode` state: `'edit' | 'preview'`, default `'edit'`
- Preserve `handleDescriptionChange`, `handleDescriptionBlur` from Story 4.1 exactly. The raw markdown text is what gets saved to the API.
- `min-h-[200px]` for both edit and preview panes so the panel doesn't jump height when toggling.
- `prose` and `prose-sm` classes (from Tailwind typography plugin, or custom styles) give reasonable markdown rendering defaults. If `@tailwindcss/typography` is not installed, use custom Tailwind classes for headings, lists, code blocks, etc.
- `font-mono` on the textarea gives a markdown-editor feel.
- `rehypeSanitize` with default settings strips `<script>`, `on*=` event handlers, `javascript:` URLs, and other unsafe constructs.

### Card Component — Description Preview on Face

Modify `frontend/src/features/cards/card.tsx`:

Below the title `<span>` (around line 115), add:

```tsx
{card.description?.trim() && (
  <p className="mt-1 text-xs text-muted-foreground line-clamp-2">
    {card.description}
  </p>
)}
```

**Important details:**
- `line-clamp-2` requires the `@tailwindcss/line-clamp` plugin or Tailwind v3.3+ native support. If `line-clamp-2` is unavailable, use:
  ```tsx
  <p className="mt-1 text-xs text-muted-foreground overflow-hidden text-ellipsis"
     style={{ display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>
    {card.description}
  </p>
  ```
- Show **plain text**, not rendered markdown. The raw markdown string is fine for a preview (e.g., "# Heading" shows as "# Heading"). Do NOT render markdown on the card face — it's visually noisy and breaks the kanban density.
- Only render if `card.description` is non-null and non-whitespace.
- Ensure the preview does not expand card height excessively. `line-clamp-2` guarantees max 2 lines.
- The card face must remain draggable. The preview `<p>` is inside the same click/drag container — no extra event handlers needed.

### Project Structure Notes

Backend files — **NO CHANGES**:
- `backend/src/cards/entities/card.entity.ts`
- `backend/src/cards/dto/create-card.dto.ts`
- `backend/src/cards/dto/update-card.dto.ts`
- `backend/src/cards/cards.controller.ts`
- `backend/src/cards/cards.service.ts`

Frontend files — **MODIFY**:
- `frontend/src/features/cards/card-detail-panel.tsx` — upgrade description to markdown editor
- `frontend/src/features/cards/card.tsx` — add 2-line description preview
- `frontend/src/features/cards/card-detail-panel.test.tsx` — update/add tests
- `frontend/src/features/cards/card.test.tsx` — add preview tests

Frontend files — **NEW**:
- `frontend/src/components/ui/tabs.tsx` — shadcn Tabs component (auto-generated by `npx shadcn add tabs`)

### CSS for Markdown Preview

If `@tailwindcss/typography` is installed, add `prose prose-sm dark:prose-invert` to the preview container.

If NOT installed, apply minimal custom styles within the preview container:

```tsx
<div className="min-h-[200px] rounded-md border border-input bg-background p-3 overflow-auto text-sm space-y-2">
  <ReactMarkdown rehypePlugins={[rehypeSanitize]} components={{
    h1: ({ children }) => <h1 className="text-lg font-bold">{children}</h1>,
    h2: ({ children }) => <h2 className="text-base font-semibold">{children}</h2>,
    p: ({ children }) => <p className="leading-relaxed">{children}</p>,
    ul: ({ children }) => <ul className="list-disc pl-5">{children}</ul>,
    ol: ({ children }) => <ol className="list-decimal pl-5">{children}</ol>,
    code: ({ children }) => <code className="rounded bg-muted px-1 py-0.5 font-mono text-xs">{children}</code>,
    pre: ({ children }) => <pre className="rounded bg-muted p-2 overflow-auto">{children}</pre>,
    a: ({ children, href }) => <a href={href} className="text-primary underline" target="_blank" rel="noopener noreferrer">{children}</a>,
  }}>
    {description}
  </ReactMarkdown>
</div>
```

The `components` prop lets you map markdown elements to styled components without needing the typography plugin.

### State Management Notes

- The `descriptionMode` state is LOCAL to `CardDetailPanel` — no need to sync with React Query or URL.
- When the panel opens, default to `'edit'` mode so the user can immediately type.
- When `card.description` updates from React Query cache (e.g., after a successful save), the preview content updates automatically because `description` state is synced from props.
- Do NOT reset `descriptionMode` when `card` prop changes — let the user stay in their chosen mode.

### Testing Strategy

**CardDetailPanel tests:**
- Render panel with description → verify Edit tab is active, textarea has raw markdown
- Click "Preview" tab → verify `react-markdown` renders the content
- Switch back to "Edit" → verify textarea is still present with same content
- Test XSS payload in description → switch to Preview → verify `<script>` tag is NOT in output
- Test auto-save on blur in Edit mode → verify `useUpdateCard().mutate` called with raw markdown
- Test empty description in Preview → verify "Nothing to preview" placeholder

**Card tests:**
- Render card with description → verify 2-line preview is shown below title
- Render card with null description → verify no preview element
- Render card with whitespace-only description → verify no preview element
- Test `line-clamp-2` behavior (or fallback CSS) truncates long descriptions

**E2E persistence test (Playwright):**
- Open card detail panel → type markdown description → switch to Preview tab → verify rendered markdown → close panel → reopen same card → verify description persisted and Preview tab still shows rendered content. This verifies DB persistence end-to-end per project test pattern (AGENTS.md: "For ALL data modification endpoints, tests MUST verify actual database state").

**Existing tests:**
- Run `npm run test` in `frontend/` — all existing tests must pass. Story 4.1 had 25/25 card module tests passing.

### References

- [Source: epics.md#Story 4.2: Markdown Description]
- [Source: architecture.md#Core Entities — Card entity has `description`]
- [Source: architecture.md#Authentication & Security — XSS prevention, NFR9]
- [Source: ux-design-specification.md#Component Strategy — MarkdownEditor with edit/preview toggle]
- [Source: ux-design-specification.md#Card component anatomy — title + description preview (2 lines)]
- [Source: 4-1-card-detail-panel.md — previous story that added description field and CardDetailPanel]

---

## Dev Agent Record

### Agent Model Used

(opencodelm)

### Completion Notes List

- [x] `react-markdown` and `rehype-sanitize` installed in `frontend/package.json`
- [x] shadcn `Tabs` component added (`frontend/src/components/ui/tabs.tsx`)
- [x] `CardDetailPanel` description section upgraded to Edit/Preview toggle
- [x] Edit mode preserves existing textarea behavior (auto-save on blur, "Saving...", error toast)
- [x] Preview mode renders markdown with `react-markdown` + `rehype-sanitize`
- [x] XSS payloads (script tags, event handlers, javascript URLs) are sanitized in preview
- [x] `Card` face shows 2-line plain-text preview of description when present
- [x] Preview hidden when description is null/empty/whitespace-only
- [x] `line-clamp-2` or equivalent CSS truncation applied
- [x] All existing frontend tests pass (backend tests unchanged — no backend work)
- [x] New frontend tests added for markdown toggle, preview rendering, XSS sanitization, card preview

### File List

Frontend — New:
- `frontend/src/components/ui/tabs.tsx` — shadcn Tabs component

Frontend — Modified:
- `frontend/src/features/cards/card-detail-panel.tsx` — markdown editor with Edit/Preview toggle
- `frontend/src/features/cards/card.tsx` — 2-line description preview on card face
- `frontend/src/features/cards/card-detail-panel.test.tsx` — updated/new tests
- `frontend/src/features/cards/card.test.tsx` — preview rendering tests
- `frontend/package.json` — added `react-markdown`, `rehype-sanitize` dependencies

Backend — No changes (description field already exists from Story 4.1)
