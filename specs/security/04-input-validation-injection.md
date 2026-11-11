# 04 — Input Validation & Injection

**Objective:** Rule out SQL injection, stored/reflected XSS (incl. markdown +
Mermaid), LIKE injection, and input abuse (oversized bodies, bad pagination,
unicode).

**Scope:** `backend-rs/src/features/*/store.rs` (SQL), `types.rs` (DTOs/garde),
frontend markdown rendering (`react-markdown`, `dompurify`, `mermaid`).

---

## 1. Test plan

### 1.1 SQL injection
- sqlx uses `?` bindings — verify no `format!`-built SQL in store.rs (CI gate
  already exists; re-run + manual grep).
- Fuzz every string input with: `' OR 1=1 --`, `"; DROP TABLE cards; --`,
  null bytes, unicode, overlong strings.
- Verify `notes` search `escape_like_pattern` escapes `%` and `_`.
- Verify no string concatenation in any query (grep `format!` in store.rs with
  the gate's SQL-keyword heuristic).

### 1.2 Stored XSS
- Inject into card title/description, note body, label name, project/board name:
  `<script>alert(1)</script>`, `<img src=x onerror=alert(1)>`,
  `[x](javascript:alert(1))`, `[x](data:text/html;base64,...)`.
- Render in frontend; verify DOMPurify strips scripts/event handlers; verify
  `react-markdown` does not render raw HTML by default (it doesn't, but check
  `rehype-raw` usage).
- Mermaid: test diagram injection (`%%{init:...}%%`, `click` directives, HTML
  in node labels). Verify `securityLevel: 'strict'` or `'sandbox'` (not 'loose').

### 1.3 Reflected XSS
- Error responses echo input? problem+json `detail` must not reflect raw input
  unsanitized. Test with `<script>` in path/query/body.

### 1.4 Input abuse
- Body > 2MB → 413/400 (axum DefaultBodyLimit).
- Deeply nested JSON → no stack overflow / reasonable error.
- Duplicate JSON keys → serde behavior (last wins) — document.
- Wrong Content-Type (text/plain) → 415 (MissingJsonContentType).
- Malformed JSON → 400 invalid-json.
- Pagination: `page=0`, `page=-1`, `limit=0`, `limit=101`, `limit=abc`,
  `project_id=abc` → 400 with errors[] (strict, no clamp, no 422).
- Oversized `limit` (e.g. 100000) → 400.

### 1.5 Unicode / normalization
- Email case folding (`Foo@Bar.com` vs `foo@bar.com`) — duplicate detection.
- Homoglyphs in names (visual spoofing) — document risk, optional normalization.
- RTL override chars in names — document.

### 1.6 LIKE / search injection
- Search term `%`, `_`, `\` → treated as literals, not wildcards.

## 2. Live findings so far
- None tested yet (requires code review + targeted requests). Backend uses
  parameterized queries by construction (sqlx `query!`).

## 3. Remediation guidance
- If any `format!` builds SQL: convert to `?` bindings + `push_bind`.
- If mermaid `securityLevel` is 'loose': set to 'strict' (or 'sandbox').
- If DOMPurify not applied or config too permissive: tighten (no `ADD_TAGS` for
  script/iframe/style; no `ALLOW_DATA_ATTR` unless needed).
- If pagination clamps silently: change to strict 400 (per spec amendment).
- Ensure `react-markdown` does not use `rehype-raw` unless sanitized.

## 4. Exit criteria
- All 1.1–1.6 pass on local compose stack.
- mermaid + DOMPurify config reviewed and documented.
- Regression tests for SQLi/XSS payloads in CI.

## 5. Task mapping
- `SEC-006` SQL injection audit (code review + fuzz).
- `SEC-007` XSS hardening (markdown + mermaid + DOMPurify).
- `SEC-008` input abuse / pagination strictness verification.