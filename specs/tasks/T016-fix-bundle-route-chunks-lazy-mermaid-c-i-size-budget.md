# T016: Fix bundle — route chunks + pele + CI size budget

> **STATUS: done (2026-11-18).** Wave 0. Closes **P1** (HIGH), **P2** (HIGH), CI half of **C2**.
>
> **Second scope amendment 2026-11-17 (user decision), folded in:** the first cut
> (83 lazy files) was judged too fragmented — lucide icons became one chunk each, and
> the two cold-landing routes paid an extra round trip. The user's directive, verbatim
> intent: *"human perceived performance first … make all routes static. bundle all
> vendor libs together. reduce lazy loading as much as possible. when the web
> application loads, everything should be snappy."* Scope now adds:
> 1. **All app routes become static imports** (BoardList, Login, Register, ProjectList,
>    ArchivedBoards, Notes pages, NoteDetail, NoteEdit, BoardView) — no route-level
>    Suspense flashes; only `markdown-renderer` and pele stay lazy (user's explicit
>    choice), with an idle prefetch of the markdown chunk.
> 2. **One `vendor` chunk** for all node_modules, excluding the markdown/pele stack
>    (that group stays lazy-only) — 1 request instead of ~5.
> 3. **Markdown consolidation**: the card-detail panel **Preview tab** switches from
>    `markdown-preview.tsx` to `LazyMarkdownRenderer` (GFM + highlight.js + mermaid,
>    feature parity with notes). `markdown-preview.tsx` is **kept** and rewired to
>    render the **board card face (`card-preview.tsx`) title** — basic markdown in
>    card title text (`inline` mode: no boxed min-height, `<p>` unwrapped, links
>    `stopPropagation` + `target=_blank`, same sanitize pipeline, memoized parse).
> 4. **Budget re-measure + re-set** under the new shape; spec/amendment figures updated
>    again; CI coordination unchanged (T017 untouched).
>
> **Third correction (review fix round, same day):** the code re-review found the
> card-title markdown wired through `react-markdown` was dragging micromark/unified
> into the *entry* on every page (~30–45 KB gzip — an exclusion set is only a
> promise about the graph, not the config). `MarkdownPreview` was rewritten as a
> **dependency-free inline renderer** (`**bold** *italic* `code` [text](https://…)`;
> everything else escaped as text; no raw HTML ever produced; links allowlisted to
> http/https/mailto with `rel="noopener"`). `dompurify` added to `LAZY_ONLY_PACKAGES`;
> four falsely-excluded packages restored to the lazy set (no out-of-family prod
> consumers exist); nested `node_modules/…/node_modules/…` attribution fixed to
> last-segment; inline-code chip styling restricted to non-block code. Initial gzip
> re-measured **257,986 B**; gate re-ratcheted **310,000 B / 225,000 B** (vendor
> 188,057). Red-proof held twice pre-fix; budget re-verified after. Follow-up review
> round (no blockers; LOWs resolved): no-language fenced code no longer gets the
> inline-code chip (multi-line heuristic); `{...props}` no longer spreads onto `<code>`;
> minimal hljs token palette added (var-referenced, light/dark); inline parser
> documents single-pass (no nested constructs).
>
> Non-goals (unchanged): pele stays lazy; highlight.js keeps the full common language
> set; typography ported from the panel's compact styles into the shared renderer so
> both surfaces look consistent; no commit without approval.
>
>
> **Amended 2026-11-17 (twice).** First amendment: scope now includes replacing the
> mermaid renderer with
> [**pele**](https://pele.run/) (`obsidianmd/pele`, MIT, by the Obsidian team) inside this
> task. The earlier "do not touch source lazy imports" rule no longer applies — the source
> change IS the renderer swap. The file slug (`-lazy-mermaid-`) is kept for ID stability.
> Budget numbers are **measure-then-set** (decision (a) from review), and theming is the
> **full pele token set mapped onto the KanPad Noir theme**, working in both light and dark
> mode (decision (b) from review). See §"Pele swap (2026-11-17 amendment)" below.
> Second amendment (2026-11-17, post-review, pre-implementation): the budget-gate
> definition now counts the entry `<script type="module" src>` in addition to
> `modulepreload` hrefs (entries are never modulepreloaded); frontmatter-vs-options wording
> clarified; `images: false` decision point added (recommended); pre-uninstall fidelity
> baseline, `@theme` teal/pink mirror, `npm audit` delta and non-`PeleError` test assertion
> added. See the "Review amendment" block in Notes.

## Goal

The source is correctly lazy (9 `React.lazy` routes, lazy diagram/markdown) but the
**production build defeats it**: `dist/` has 5 JS files, one dynamic import, and the
mermaid chunk (measured 2026-11-16: ~4.9 MB raw / **1,356 KB gzip**) is `modulepreload`ed
+ statically linked from the entry → **~1.84 MB gzip initial JS on every cold load,
including `/login`** (measured 2026-11-17: entry 69,824 B + runtime 658 B + vendor-react
138,217 B + vendor-mermaid 1,355,682 B + vendor 278,425 B ≈ 1.84 MB; fresh `npm run build`).

1. **Diagnose** the rolldown `codeSplitting.groups` capture (confirmed suspects: the
   catch-all `app` group at `vite.config.ts:52-56` merging entry modules, and the
   catch-all `vendor` group at `vite.config.ts:46-51` forcing every node_module into one
   shared initial chunk, statically linked from the merged entry). Red-proof the current
   state first.
2. **Replace mermaid with pele** — the mermaid chunk disappears entirely; notes diagrams
   render via `pele/lazy` (detected diagram type loads on first use, on demand).
3. **Fix** the chunk config so route-level splitting survives and diagram code is
   reachable only via dynamic import. Budget numbers **measure-then-set** (see Constraints).
4. **CI budget gate:** script fails the build if initial gzip > budget or any
   **initial-load chunk** (entry `<script type="module" src>` + every `modulepreload`
   href — entries are *not* modulepreloaded, verified 2026-11-17) > per-chunk limit;
   wire into the CI `web` job after `vite build`. Red-proof the gate.

## Pele swap (2026-11-17 amendment)

Why pele: ~**230–274 KB gzip** full bundle (vs 1.36 MB mermaid chunk; measured pele
1.13.0 `dist/pele.js` = 729,332 B raw / 274,134 B gzip), synchronous API, **zero runtime
dependencies**, published via GitHub Actions with **npm OIDC + SLSA provenance**, MIT,
developed by the Obsidian team (~4.6k stars; ~1.87M downloads last week vs mermaid
~26.8M; mermaid 12.3.0, MIT). Parsers tested against Mermaid's own spec suite +
differential fuzzing. Supports all built-in Mermaid 12.1.0 diagram types with its own
layout engine. **It is not the industry default yet** — treat as fast-moving 1.x:
pin exact version, keep sanitization, verify fidelity on real notes diagrams.

Measured from the `pele@1.13.0` tarball (2026-11-17):

| Entry | gzip |
|---|---|
| `dist/pele.js` (all diagram types) | 274,134 B (729,332 raw) |
| `dist/core.js` (API only) | ~212 B |
| `dist/lazy.js` | ~1,090 B |
| per-type chunks (`dist/diagrams/*`) | ~160–205 B gzip stubs + shared chunk (~15 KB) |
| `dist/esm/chunks/` total | ~968 KB raw |

Environment notes verified against the tarball (do not re-derive):

- **jsdom-safe:** the canvas text measurer (`wn()`) returns `null` when `getContext('2d')`
  is unavailable → pele falls back to a built-in width table. Unit tests will not crash.
- `render()` is synchronous and needs no `initialize()` call.
- `useId()` from React contains `:` (e.g. `:r1:`) — pele `idPrefix` allows only
  letters/digits/`-`/`_`. Sanitize before passing.
- Pele restricts nothing else relevant; our component passes no `options` from diagram
  source and registers no resolvers.

### Component plan (`mermaid-diagram.tsx`)

Component name `MermaidDiagram` and the ` ```mermaid ` code fence stay — the
**syntax users write is unchanged**; pele is the renderer. File name stays.

- Drop the `initialize()`/promise-cache dance (`mermaidInitialized`, `mermaidPromise`).
- `loadPele()` → `dynamic import('pele/lazy')` — keeps diagram code out of the notes
  route chunk entirely; only the detected type chunk loads on first diagram.
- `const { svg } = await renderAsync(code, { idPrefix, fontFamily, fontFamilyMono })`
  inside try/catch. `idPrefix = useId().replace(/[^a-zA-Z0-9_-]/g, '')`.
- **`fontFamily` must be the exact same string as `--pele-font`** (drawn font = measured
  font): `'Inter, system-ui, -apple-system, sans-serif'`. `fontFamilyMono`:
  `'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace'` (keep identical to
  `--pele-font-mono`).
- **Keep `DOMPurify.sanitize(svg)`** — pele output is safe by construction (escaped
  text, filtered links/images/styles), but sanitize is defense in depth and already a
  dependency. Pele never emits `<foreignObject>` here (no `math` resolver), so DOMPurify
  strips nothing legitimate.
- Errors: `PeleError` → existing static `<pre>Failed to render diagram</pre>` fallback.
  **Never** render the pele message (it can contain diagram source — pele security
  guidance). Non-`PeleError` rethrows are swallowed by the same fallback as today.
- Keep `role="img"` + `aria-label`, keep `LazyMarkdownRenderer` boundary upstream.

### Theming — full pele token set, KanPad Noir consistent (decision (b))

App already flips tokens via `.dark` on `<html>`, so **one `.pele {}` block of `var()`
references works for both modes** — no duplicate dark mapping. New block in
`frontend/src/index.css`, plus two new label tokens in `:root` **and** `.dark`
(teal `#14B8A6` / `#2DD4BF`, pink `#EC4899` / `#F472B6`).

| pele var | Maps to | Notes |
|---|---|---|
| `--pele-bg` | `var(--background)` | diagram bg = page bg |
| `--pele-fg` | `var(--foreground)` | text-primary |
| `--pele-fg-on-light` | fixed `#18181B` | source-fixed light fills — fixed by design |
| `--pele-fg-on-dark` | fixed `#FFFFFF` | source-fixed dark fills — fixed by design |
| `--pele-muted` | `var(--muted-foreground)` | |
| `--pele-line` | `var(--text-secondary)` | mid-gray; visible in both modes (unlike `--border`) |
| `--pele-surface` | `var(--surface)` | node fill |
| `--pele-surface-alt` | `var(--card)` | subgraph panel, lighter both modes |
| `--pele-border` | `var(--border)` | node/subgraph borders |
| `--pele-accent` | `var(--label-blue)` | monochrome brand has no accent; blue flips per theme |
| `--pele-critical` | `var(--error)` | also used for kanban high priorities |
| `--pele-error` | `var(--error)` | used by `mount()`; harmless for `render()` |
| `--pele-font` | `'Inter', system-ui, -apple-system, sans-serif` | **identical string to render option** |
| `--pele-font-mono` | `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace` | identical to render option |
| `--pele-title-weight` / `--pele-heading-weight` / `--pele-bold-weight` | `600` | Inter semi-bold, matches app headings |
| `--pele-radius` | `var(--radius)` | app 0.5rem |
| `--pele-series-1`…`8` | `var(--label-blue)`, `var(--label-orange)`, `var(--label-green)`, `var(--label-red)`, `var(--label-teal)`, `var(--label-yellow)`, `var(--label-purple)`, `var(--label-pink)` | charts/git-graph/pie stay inside the label palette family |

- **Render options are exclusively the app's.** The component never passes `options` from
  diagram source, never registers resolvers. Distinction to keep precise: diagram-*source*
  frontmatter (`---config: themeVariables: …---`) is read by pele by design — Mermaid
  compatibility, not a render option — and applies only as **fixed, theme-independent
  series-palette colors**; it inserts no markup and is not a security decision.
- **Decision to record at implementation time (`images: false` recommended).** Pele's
  default allows `http/https` images, so a user diagram can hotlink a remote image — a
  silent third-party fetch on every note view (IP/tracking-pixel vector; mermaid does not
  allow this by default). Recommended: pass `images: false` in the render options (one
  line, no expected KanPad UX impact; diagrams render text/shapes only). Alternative: keep
  the default and record the tradeoff. Either way the decision + rationale goes in Notes.
  `links` may stay at the default (`http/https/mailto/tel`, `rel="noopener"`): links carry
  no fetch-on-view cost.
- Do **not** pass `math`, `icons`, or `links`/`images` resolvers. Link defaults stay
  (`http/https/mailto/tel`; images `http/https` — unless `images: false` is adopted;
  `javascript:` / `data:` / `vbscript:` always blocked; `rel="noopener"`). Limit defaults
  stay (50k source chars, 4M SVG output, 5k edges).

## Depends On

(None — T016 now adds/removes deps itself.) T017 depends on this — both edit the CI
workflow; T018 owns shadcn/tailwind dep *moves* (its scope is unchanged, but the
lockfile will have shifted — note it in T018).

## Phase

0

## Critical

No — perf/UX, not correctness; but it's the biggest perceived-performance win available.

## Spec References

- `specs/arch-hardening-specs/20-performance.md` §Bundle (P1/P2 design: diagnose step,
  groups fix, budget gate, edge cases). **Amend with a dated note during implementation:
  mermaid → pele swap, same budget-gate mechanism.**
- `specs/arch-hardening-specs/00-overview.md` — findings P1, P2, C2; global DoD. Amend P1/P2
  line with the same dated note during implementation.
- `frontend/vite.config.ts:30-61` (the groups config), `frontend/dist/` (measured
  evidence, 2026-11-17), `frontend/src/App.tsx:11-37` (lazy routes),
  `frontend/src/features/notes/mermaid-diagram.tsx` (the component to swap),
  `frontend/src/index.css` (theme tokens).
- pele: <https://pele.run/api>, <https://pele.run/security>, <https://pele.run/theming>,
  <https://pele.run/compatibility>; repo `obsidianmd/pele` (MIT; OIDC + SLSA provenance;
  zero runtime dependencies). Verify render/lazy semantics against the published 1.13.0
  tarball or the docs at implementation time — do not guess.
- CI: `.github/workflows/backend-rs.yml` `web` job.

## Files to Create/Modify

| File | Change |
|---|---|
| `specs/tasks/T016-fix-bundle-route-chunks-lazy-mermaid-c-i-size-budget.md` | (this doc — done) |
| `specs/arch-hardening-specs/20-performance.md` + `00-overview.md` | Dated pele-swap amendment at implementation time |
| `frontend/src/features/notes/mermaid-diagram.tsx` | Renderer swap: mermaid → `pele/lazy` (`renderAsync`), keep DOMPurify, sanitized `idPrefix`, fonts passed explicitly |
| `frontend/src/features/notes/mermaid-diagram.test.tsx` | Rewrite: mock `pele/lazy` (success / `PeleError` / non-`PeleError`); add DOMPurify-strip assertion + `idPrefix` charset assertion |
| `frontend/package.json` + lockfile | `npm uninstall mermaid`; `npm install -E pele@1.13.0` (exact pin); add `budget` script |
| `frontend/src/index.css` | Full `.pele {}` mapping (one `.pele {}` block, var-referenced, works light+dark); add `--label-teal` / `--label-pink` to `:root` and `.dark`; mirror both into the `@theme` `--color-label-teal` / `--color-label-pink` mapping for parity with the six existing label colors |
| `frontend/vite.config.ts` | Remove `vendor-mermaid` group; **remove catch-all `vendor` group** (same bug class as the `app` group) and **remove catch-all `app` group**; keep `vendor-react` (priority 20); keep/preserve route-level and lazy boundaries per current rolldown-vite docs; lower `chunkSizeWarningLimit` post-fix |
| `frontend/scripts/check-bundle-budget.mjs` | **NEW** — parse `dist/index.html` → **entry `<script type="module" src>` + `modulepreload` hrefs** → gzip each (node zlib) → fail over budget (initial > budget or any initial-load chunk > per-chunk limit; both set measure-then-set). Entries are not modulepreloaded — a modulepreload-only gate silently waives the entry (~68 KB) from every check |
| `.github/workflows/backend-rs.yml` | Run budget script after `vite build` in `web` job |
| Tests | Budget-script unit test (fixture dist/ over/under budget); build-output assertions as pasteable evidence |
| `docs/lessons/frontend.md` | Dated, tagged lesson (2026-11-17, `frontend,bundle`) |

**Must NOT appear in the diff** (superseded by the second amendment, 2026-11-17):
`package.json` dependency *moves* (T018 owns shadcn/tailwind → devDependencies),
Playwright config (T017), React Compiler usage, `staleTime`/runtime behavior. The
mermaid→pele renderer change IS in scope by explicit owner decision (2026-11-17).
**Second-amendment supersessions:** `markdown-renderer.tsx` *was* modified (compact
typography port + GFM/table component styles — part of the renderer consolidation,
user-approved); `App.tsx` went all-static (user directive); `card-preview.tsx`,
`card-detail-panel.tsx`, `markdown-preview.tsx` all changed — the original
"notes markdown flow unchanged" clause no longer applies and is retained only as
first-amendment history.

## Implementation Steps

1. **Red-proof current state** (evidence, 2026-11-17 — paste into Notes): `dist/assets/`
   = 5 JS files; gzip: index 69,824 B · rolldown-runtime 658 B · vendor-react 138,217 B ·
   vendor 278,425 B · **vendor-mermaid 1,355,682 B**; `grep modulepreload dist/index.html`
   shows runtime, vendor-react, **vendor-mermaid**, vendor. P1/P2 confirmed.
2. **Amend spec refs** (`20-performance.md` §Bundle, `00-overview.md` P1/P2) with a dated
   note: renderer is pele; gate mechanism unchanged. Also refresh the stale P2 measured-date
   so the two docs agree (00-overview P2 cites 2026-11-13; 20-performance cites 2026-11-16) —
   fold the 2026-11-17 re-paste from step 1 into the dated note.
3. **Renderer swap:** *Before uninstalling,* capture the fidelity baseline: render 2–3
   representative diagrams (a flowchart, a pie chart, a sequence) with mermaid, screenshot
   them in light + dark, and keep the images out of the repo (or under `/tmp/opencode`) —
   step 10's "record pele-vs-mermaid fidelity observations" needs a reference, not recall.
   Then `npm uninstall mermaid && npm install -E pele@1.13.0`; rewrite
   `mermaid-diagram.tsx` per the component plan; verify `npm ls mermaid` → empty; run
   `npm audit` and note any new/changed advisories in Notes; rewrite
   `mermaid-diagram.test.tsx` (include a non-`PeleError` rejection landing in the same
   static fallback, not just `PeleError`).
4. **Theming:** add `--label-teal`/`--label-pink` to `:root` and `.dark` (plus the `@theme`
   `--color-label-teal`/`--color-label-pink` mirror); add the full `.pele {}` token block
   (single var-referenced block, both modes) per the table.
5. **Chunk config:** remove `vendor-mermaid`, `vendor`, `app` groups; keep `vendor-react`.
   Verify group semantics against current rolldown-vite docs (Context7) before writing —
   do not guess from rollup `manualChunks` lore.
6. **Build & measure (step (a)):** `npm run build`; first assert correctness (per-route
   chunks exist; entry does not statically import pele/mermaid; `modulepreload` excludes
   `vendor-mermaid` and any pele chunk; notes route stays lazy; pele per-type chunks
   emitted on demand). THEN compute initial gzip (**entry `<script type="module" src>` +
   modulepreload total** — the entry is not modulepreloaded) and set the budget at
   measured + 15–20 % headroom; set per-chunk limit from reality (e.g. measured largest
   initial-load chunk — entry or modulepreload — + headroom). **Do not gerrymander** (e.g. splitting one
   honest 278 KB chunk into two 139 KB chunks to pass a rule) — adjust the number with
   written justification in Notes instead; the task Constraint explicitly allows this.
   Re-lower `chunkSizeWarningLimit` to reality. Paste all numbers.
7. **Budget script + CI wiring:** `frontend/scripts/check-bundle-budget.mjs` → `budget`
   script → CI `web` job after `vite build`. Red-proof: temporarily make pele eager
   (`import { render } from 'pele'` at module top) → gate exits 1 → revert.
   Unit test the script (fixture dist/ over/under; include an entry-only-over-budget fixture — must fail — proving entries are counted).
8. **Verify budget options:** if measured initial is ~485 KB (entry 68 + runtime ~1 +
   vendor-react 138 + residual vendor), record it — removing the vendor catch-all should
   route most of that into lazy/route chunks, which is the point of step 5. If initial
   still exceeds the original 300 KB aspiration, the adjusted budget stands, documented.
9. **Full suite:** `vitest run` (unit + storybook) and `tsc --noEmit`.
10. **Visual smoke, light AND dark:** `/login` loads fast; `/notes/:id` with a flowchart,
    a **pie chart** (validates series palette against Noir theme), and a sequence
    diagram — all render with theme (no re-render toggle artifacts); verify diagram text
    contrast in both modes. Record pele-vs-mermaid layout fidelity observations in Notes
    (pele layout differs by design; mermaid syntax unchanged).
11. Lesson appended to `docs/lessons/frontend.md` (`frontend,bundle`, 2026-11-17).

## Constraints

- Diagram code must remain **dynamically imported** (behavior: notes diagrams render on
  demand; nothing diagram-related enters the initial path). Enforced by the gate.
- **Initial set = entry `<script type="module" src>` + `modulepreload` hrefs** (verified
  2026-11-17: the entry is never modulepreloaded). A modulepreload-only gate silently
  waives the entry (~68 KB) and the per-chunk limit never sees it — don't build it that way.
- **Budget is measure-then-set** (decision (a), 2026-11-17): measure after the chunk fix,
  set gate = measured + 15–20 % headroom, justify in Notes. Headroom rule exists so the
  gate doesn't become noise. Note the consequence: the gate guards **regression, not an
  absolute target** — the 300 KB aspiration in `20-performance.md` is retired; consider a
  follow-up task in a later wave to ratchet the budget down once the dust settles.
- **Pele pinned exactly** (`1.13.0`) — fast-moving 1.x (~2 releases/week). Do not widen
  to `^` carets or bump opportunistically in this task; a pele upgrade is a separate,
  deliberate change.
- No `math`/`icons` resolvers and no user-supplied `options`/`config` — pele then inserts
  no unfiltered markup. Keep DOMPurify. Error text displayed only via the static fallback.
  Diagram-source frontmatter (`themeVariables`) is pele's documented Mermaid-compatible
  behavior (fixed, theme-independent palette colors only) — it is not a render option.
  The `images: false` hardening (or a recorded exception) is part of this task — see the
  decision note in the options paragraph below the pele security guidance.
- Do not hand-tune away React Compiler or change `staleTime`/runtime behavior — this is
  a bundling + renderer-swap fix only.
- Rolldown/rolldown-vite group semantics: **verify against current docs** (Context7) —
  `codeSplitting.groups` is not rollup `manualChunks`.
- `useId()` must be sanitized to `[a-zA-Z0-9_-]` for `idPrefix`.
- Font strings must match between render options and `--pele-font`/`--pele-font-mono`
  (drawn = measured). Inter web-font load race is accepted; visual smoke covers it.
- Global DoD; no commit without approval; no `_sqlx_migrations` edits (n/a here); no
  changes to T017/T018 scope.

## Acceptance Criteria

- [x] Red-proof of the defect pasted (dist listing, gzip sizes, modulepreload lines).
- [x] Renderer swapped: `npm ls mermaid` empty; `pele@1.13.0` pinned exactly;
      `mermaid-diagram.test.tsx` rewritten and green (incl. a non-`PeleError` rejection →
      same static fallback); `markdown-renderer.tsx` modified only by the second
      amendment (typography port); fidelity baseline captured
      pre-uninstall (mermaid flowchart/pie/sequence, light + dark); `images:` decision
      recorded in Notes; `npm audit` delta noted.
- [x] Full Noir pele theme in place: single `.pele {}` var-referenced block, new
      `--label-teal`/`--label-pink` in both `:root` and `.dark` (+ `@theme` mirror);
      light and dark diagrams render with theme-correct colors (visual smoke evidence,
      incl. pie chart series contrast).
- [x] Post-fix `dist/`: route-level chunks exist (first pass 83 JS files; second
      amendment — all routes static — 58 JS files); entry does not
      statically import pele or mermaid (the one `mermaid` hit in the entry is the
      editor toolbar label string, not the renderer); `modulepreload` excludes diagram
      code; **initial gzip = 257,986 B (entry 69,500 + runtime 429 + single vendor
      chunk 188,057) vs 310,000 B budget — see Notes. Closes **P1**, **P2**.
- [x] Notes diagrams render (flowchart/pie/sequence, light + dark; live smoke against
      backend-rs + vite dev, note created/rendered/deleted).
- [x] Budget gate in CI `web` job, red-proofed (over-budget build fails with a clear
      message) (closes CI half of **C2**).
- [x] `chunkSizeWarningLimit` lowered (800 → 500 → 700 after the second amendment;
      vendor is 650 KB raw); full frontend suite + `tsc --noEmit` green;
      lesson appended; T016/T017 CI coordination kept intact (T017 not edited).

- [x] (Second amendment — done) All app routes static; single `vendor` chunk
      (markdown/pele family excluded, stays lazy); card-details panel Preview tab
      renders via `LazyMarkdownRenderer` (GFM/highlight/mermaid verified live);
      `CardPreview` title renders basic markdown via `MarkdownPreview inline`; initial
      gzip re-measured (257,986 B) and gate re-set (310,000/225,000); notes +
      card-details markdown render GFM/highlight/mermaid in smoke (light + dark);
      suite+tsc+lint green (879).

## Notes

**Second scope amendment 2026-11-17 (bundle shape, folded into T016):**
implementation resumed under the "perceived snappiness first" directive (see STATUS
block). All app routes become static; one catch-all `vendor` group replaces the
`vendor-react` group with the markdown/pele family excluded into its own lazy-only
group; card-details panel Preview tab uses `LazyMarkdownRenderer`; `MarkdownPreview`
stays and now renders the board card face title (`inline` mode — no boxed min-height,
`<p>` → fragment, anchors stopPropagation + `target=_blank`); shared renderer gains the
panel's compact typography; idle prefetch covers the lazy markdown chunk. Measured
figures + gate constants land below after the re-build.

**Review rounds (2026-11-17):** four parallel reviewers (code correctness, security,
build/CI, spec/scope) on the first implementation; all findings addressed and
re-verified — HIGH: `vendor-react` group regex over-captured the react-* ecosystem into
the initial chunk (react-markdown/react-router/day-picker were hoisted in, defeating the
lazy markdown boundary while the gate still passed) → boundary-anchored to
`(react|react-dom|scheduler)[\\/]`, vendor-react 451,942 → 218,921 B raw; budget
re-ratcheted (221,534 → **167,385 B** measured; 265,000/165,000 → **200,000/80,000 B**);
two independent re-reviewers confirmed the fix + all re-measured numbers. MEDs: rejected
`import('pele/lazy')` cached forever → rejection-reset in `loadPele`; no tests for
out-of-order resolution/unmount/empty-svg → added (`act()` barrier for the
stale-write case); DOMPurify default allows `<style>` → `FORBID_TAGS:['style']` + test.
Build review: zero-modulepreload and vendor-react-not-initial drift tripwires added to
the gate (fail closed), job renamed `web (gen:api --check -> tsc -> vite build -> bundle
budget)`. LOW: aria-label made static ("Diagram"); font-parity test pins
`--pele-font(-mono)` === JS render options with CSS comments stripped; gate `isMain`
uses `realpathSync`; entry/preload dedupe. **Test-strength honesty note:** deleting the
`seq !== renderSeqRef.current` half-guard alone keeps the out-of-order test green
because effect cleanup's `cancelled` flag already covers re-render stale writes — the
test bites when the whole guard is removed (verified by mutation testing; it fails).
`seq` is belt-and-braces, kept deliberately. The `loadPele` rejection-reset branch has
no test (needs a rejecting dynamic-import mock the `vi.mock('pele/lazy')` seam can't
express without `resetModules` gymnastics) — accepted and recorded. `style="…"`
attributes still survive DOMPurify's default (pele needs inline styles for its CSS-var
theming) — elements are locked down, attributes stay; noted in the lesson.

**Build-to-build noise:** gzip figures of record drift by ≤30 B per rebuild. All
figures of record in this file and the spec docs are from 2026-11-17 final builds
(after the review fix round: **257,986 B** — entry 69,500 + runtime 429 + vendor
188,057); ±30 B is noise, not a discrepancy.

**Illustrative headroom line (superseded by the second amendment):** gate was set to
initially **200,000 B** (measured 167,385 + ~19.5 %) / per-chunk **80,000 B** —
from review. **Second amendment (2026-11-17) re-set to 310,000 B / 225,000 B**,
measured after all app routes went static (257,986 B; entry 69,500 + runtime 429 +
vendor 188,057; 58 JS files). First fix's numbers are kept below for history.

**Measure-then-set budget (recorded 2026-11-17, implementation; re-ratcheted same day
after review):** post-fix initial gzip first measured at **221,534 B** (node zlib), and
the budget was set to 265,000 B / per-chunk 165,000 B. A review found the
`vendor-react` group regex (`react(?:-dom|-compiler)?`) had no trailing boundary and
was hoisting the whole react-* ecosystem (react-markdown, react-router, react-day-picker)
into the initial chunk — defeating the lazy markdown boundary. The group is now
boundary-anchored (`(react|react-dom|scheduler)[\\/]`); vendor-react dropped
451,942 → 218,921 B raw (**138,453 → 67,627 B** gzip); entry absorbed the eagerly-shared
rest. **Figure of record (re-ratcheted): initial gzip = 167,385 B** (entry 53,266 +
runtime 429 + vendor-react 67,627 + toast-helpers 12,676 + x- 19,853 +
dist-CRjgnXWp 10,141 + chevron-down 3,068 + dist-5YusBU5k 325). Budget re-set to
**200,000 B** (measured + ~19.5 % headroom, inside the 15–20 % rule), per-chunk limit
**80,000 B** (vendor-react 67,627 + ~18.3 %). The gate is a regression guard, not the
retired 300 KB absolute target; a ratchet task in a later wave is suggested.
`chunkSizeWarningLimit` 800 → 500 (largest raw chunk now: vendor-react 218,921 B,
markdown-renderer ~407 KB). All zlib-gzipSync numbers; `gzip -c` reads ≤150 B lower.

**Red-proofs (2026-11-17):**
- Pre-fix dist (fresh `npm run build`): 5 JS files; index 251,476 B raw / 69,824 B gzip ·
  rolldown-runtime 1,116 / 658 · vendor-react 451,942 / 138,217 · vendor 925,976 /
  278,425 · **vendor-mermaid 4,896,991 / 1,355,682**; `modulepreload` referenced runtime,
  vendor-react, **vendor-mermaid**, vendor → ≈1.84 MB gzip initial. P1/P2 confirmed.
- Budget gate red-proof (re-run after the re-ratchet): temporary eager
  `import { render } from 'pele'` in `src/App.tsx` (entry path) → entry ballooned to
  306,309 B gzip, total initial 420,428 B → `npm run budget` exits 1 with clear
  per-chunk and total failures → reverted, build green again (167,385 B, exit 0).
- **Gate boundary honesty note:** the same eager import inside `mermaid-diagram.tsx`
  (as literally suggested in Implementation Step 7) puts pele in the *lazy*
  markdown-renderer chunk (1,099 KB raw / 384 KB gzip) — the initial-load gate passes,
  correctly. The gate guards the initial path; lazy-chunk bloat is a different axis.
  Red-proving at the entry is the honest end-to-end check. Both observations recorded
  here for the reviewer.

**Pele swap decisions:**
- `images: false` **adopted** — pele's default allows `http/https` images, so a user
  diagram could hotlink a remote image (silent third-party fetch on every note view;
  IP/tracking-pixel vector; mermaid does not allow this by default). One line, no
  expected KanPad UX impact — diagrams render text/shapes only. `links` left at the
  default (`http/https/mailto/tel`, `rel="noopener"`): links carry no fetch-on-view
  cost. `math`/`icons` not passed; no resolvers; no user-supplied options/config.
  Diagram-source frontmatter (`themeVariables`) remains pele's documented
  Mermaid-compatible behavior (fixed, theme-independent palette colors only) — not a
  render option, no security decision.
- `npm ls mermaid` → empty. `pele@1.13.0` pinned exactly (`-E`).
- **fidelity baseline captured pre-uninstall** (mermaid flowchart/pie/sequence, light +
  dark, `vite preview`-equivalent standalone page with mermaid from node_modules,
  screenshots under `/tmp/opencode/t016-baseline/mermaid-{light,dark}.png`). These
  images are deliberately not in the repo.
- **pele-vs-mermaid layout fidelity observations:** pele's layout differs by design
  (own layout engine, auto node spacing) but is clean and readable for all three
  diagram types in both modes. Pie series palette maps exactly to the KanPad label
  palette (blue/orange/green/red/teal render in slot order; dark-mode slices keep the
  dark label tokens — contrast is good in both modes). Flowchart arrow/edge routing is
  more compact than mermaid's. Sequence diagram renders solid arrows for sync calls and
  dashed for responses, same as expected. Mermaid syntax users write is unchanged.
- **DOMPurify kept** (defense in depth; pele output is safe by construction — escaped
  text, filtered links/images/styles; no `<foreignObject>` emitted, so nothing
  legitimate is stripped). Test asserts `<script>`/`onload` are stripped.
- **idPrefix sanitized** (`useId()` contains `:`; pele allows `[a-zA-Z0-9_-]`);
  test asserts the charset. Render options use only the app's fonts, byte-identical to
  `--pele-font`/`--pele-font-mono` (`Inter, system-ui, -apple-system, sans-serif` /
  `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`).
- **`PeleError` and non-`PeleError` failures both land in the same static
  `<pre>Failed to render diagram</pre>` fallback** — pele messages can echo diagram
  source and are never rendered. Both paths pinned by tests.
- `markdown-renderer.tsx` untouched; ` ```mermaid ` fences unchanged.
- **`npm audit` delta:** post-swap audit shows **15 moderate** advisories, all in the
  dev-toolchain (storybook/`@storybook/react-vite` → glob/minimatch; `shadcn` →
  ts-morph/minimatch; `eslint`/`typescript-eslint` → minimatch; fix requires breaking
  upgrades). pele has **zero runtime dependencies** and contributes no advisories;
  no production-dependency advisories were introduced. Pre-swap baseline was not
  captured separately — flagged for honesty; all 15 trace to devDependencies.
- **Budget script:** `frontend/scripts/check-bundle-budget.mjs` — parses
  `dist/index.html` for the entry `<script type="module" src>` + every
  `<link rel="modulepreload">`, gzips each via node `zlib`, fails exit 1 on
  per-chunk > 225,000 B or initial total > 310,000 B, prints per-chunk + total gzip.
  Fails closed on drift: zero modulepreload hrefs in index.html, or a `vendor-react-*.js`
  chunk in dist/assets that is NOT in the initial set, both fail the gate (added after
  build review). Dedupes entry/preload overlap; missing preloaded asset fails;
  inline `<script type="module">` without `src` is not counted (vite always emits the
  entry with `src`); double-quoted attributes only.
  Unit tests in `scripts/check-bundle-budget.test.mjs` (13 tests): under-budget pass,
  modulepreload per-chunk failure, **entry-only per-chunk failure** (entries are never
  modulepreloaded), entry-only total-budget failure, summed-total failure, missing
  index.html, gzip measurement sanity, zero-modulepreload drift tripwire,
  vendor-react-present-but-not-initial tripwire, missing modulepreloaded asset,
  entry/preload dedupe. `budget` npm script added; CI `web` job runs it
  after `vite build`.
- **Storybook browser-mode flakiness:** first `test-storybook` run failed 7/54 with
  15 s timeouts (contention); quiet re-run passed 54/54. Noted in lesson.
- **Dev-server vs production timing:** `/login` in vite dev took ~13 s to form (dev
  transforms on the fly); production `vite preview` showed ~1.4 s to interactive form
  (77 requests, local, no CDN-font dependency).
- **Visual smoke (live, light + dark):** backend-rs (`:3001`, `kanpad_rs_dev`) +
  vite dev (`:5173`); registered `t016-smoke@test.local` in the dev DB, created notes
  with flowchart/pie/sequence, screenshotted `/notes/:id` in both modes (renders 3
  `.pele` SVGs; theme flips with no re-render artifacts; pie slices legible and
  on-palette in both modes), then **deleted both notes** (204). First smoke: note 17
  (pre-review build) and note 18 (final build after the vendor-react regex fix and
  DOMPurify `FORBID_TAGS` hardening) — both render green in light and dark. Screenshots kept under
  `/tmp/opencode/t016-baseline/{smoke,smoke2}-{light,dark}.png` (not in repo). The
  smoke user remains in the dev DB; `t016-smoke@test.local` is a dev-only account.
- **T018 coordination:** lockfile shifted (mermaid removed, pele added) — T018's
  shadcn/tailwind dep moves will rebase on top; no `package.json` dependency *moves*
  were made here (only `mermaid` out, `pele` in — both owned by this task). T017 not
  touched.
- **Spec amendments:** `20-performance.md` §Bundle and `00-overview.md` P1/P2/C2 carry
  dated 2026-11-17 pele-swap notes + refreshed measured dates; amendment log entry
  added. 300 KB figure retired there too.

**Review amendment 2026-11-17 (post-review, pre-implementation):** the budget-gate
definition was corrected — initial set = entry `<script type="module" src>` + modulepreload
hrefs (entries are never modulepreloaded; a modulepreload-only gate waives the entry
~68 KB); per-chunk limit applies to all initial-load chunks. Clarified that diagram-source
frontmatter `themeVariables` is not a render option (fixed, theme-independent palettes
only). Added: `images: false` decision point (recommended), pre-uninstall mermaid fidelity
baseline capture, `@theme` teal/pink mirror, `npm audit` delta, non-`PeleError` test
assertion, spec measured-date drift note. Budget remains a regression guard, not the old
300 KB absolute target — a ratchet task in a later wave is suggested.

## Lessons

Appended to `docs/lessons/frontend.md` (2026-11-17, `frontend,bundle`): catch-all
`codeSplitting.groups` without `test` are black holes (the `app` group merged entries;
the `vendor` group then statically linked everything); `modulepreload`-only gates waive
the entry; red-proof the gate at the real boundary (entry), not a proxy (lazy chunk);
measure-then-set budgets; replacing a heavy renderer beats lazy-loading it; storybook
browser-mode 15 s timeout flake under contention; production-artifact timings, not
dev-mode vibes.

